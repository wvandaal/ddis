# Execution Topologies — Exploration Index

> **Identity:** This exploration develops the theory and design for coordination topology
> management within DDIS/Braid — how multiple agents (human and AI) organize their
> communication, work distribution, scaling, and convergence, mediated through the
> datom store.
>
> **Status:** Design exploration (pre-specification). To be distilled into Braid spec
> namespaces when the framework is validated.
>
> **Date:** 2026-03-09
> **Session:** Deep first-principles exploration of agent/human work topologies

---

## Core Thesis (One Sentence)

**In a CRDT-based datom store, the coordination topology is not imposed externally — it
emerges from the data, is computed by the same query engine that validates specifications,
learns through the same harvest/seed lifecycle that preserves knowledge, and converges
through the same bilateral loop that aligns spec with implementation. Because the
specification is queryable data in the same store, the optimal topology can be *compiled*
from spec structure — not merely discovered from execution — making Braid the first
multi-agent framework where coordination is derived from structure rather than learned
from trial and error.**

---

## Reading Order

Read these documents in order. Each is self-contained at the paragraph level (an agent
can read just one document and understand that aspect completely), but they build on
each other conceptually.

| # | Document | Cognitive Mode | What It Covers |
|---|----------|---------------|----------------|
| 00 | `00-thesis.md` | Philosophical/motivational | The core insight: topology emerges from the datom store. The conversation/datom boundary. Human/AI isomorphism. Why this matters. |
| 01 | `01-algebraic-foundations.md` | Algebraic/mathematical | Sheaf structure, CALM stratification, lattice of topologies, harvest functor, composition with Braid algebra. |
| 02 | `02-coupling-model.md` | Analytical/empirical | Five coupling mechanisms, composite signal, learnable weights, staged introduction. |
| 03 | `03-topology-definition.md` | Definitional/ontological | T = (G, Phi, Sigma, Pi) formal definition, named patterns, hybrid topologies, datom schema, worked examples. |
| 04 | `04-transition-protocol.md` | Protocol/behavioral | CALM-stratified transition protocol, Tier M/NM, state machine, invariants. |
| 05 | `05-scaling-authority.md` | Policy/decisional | A(d) = R(1-C)T authority function, three-tier delegation, trust accumulation, Tier 2 protocol. |
| 06 | `06-cold-start.md` | Algorithmic/procedural | Monotonic relaxation from mesh, COLD_START algorithm, convergence trajectory, spectral partitioning. |
| 07 | `07-fitness-function.md` | Measurement/evaluative | F(T) seven dimensions, bilateral loop, diagnostic mapping, composition with F(S), quadrilateral convergence. |
| 08 | `08-open-questions.md` | Exploratory/uncertain | Signal system, capability modeling, cross-project topology, observability, verification strategy. |
| 09 | `09-invariants-catalog.md` | Reference/catalog | All topology invariants with IDs, falsification conditions, verification methods. |
| 10 | `10-design-decisions.md` | Decisional/archival | All ADRs from this exploration with problem/options/decision/consequences. |
| 11 | `11-topology-as-compilation.md` | Paradigmatic/capstone | **The key differentiator.** Spec dependency graph IS a program; topology IS the compiled execution plan. AOT compilation from spec structure, JIT fallback for uncompilable work, PGO via bilateral feedback. Eliminates cold-start, inverts learning loop, unifies task decomposition with topology selection. |

---

## Relationship to Braid Spec

This exploration extends multiple existing Braid spec namespaces:

| Existing Namespace | How Topology Extends It |
|--------------------|------------------------|
| `spec/07-merge.md` (MERGE) | Merge topology = coordination topology. G and Phi components determine merge patterns. |
| `spec/08-sync.md` (SYNC) | Sync barriers used for non-monotonic topology transitions (Tier NM). |
| `spec/09-signal.md` (SIGNAL) | Topology events (drift, conflict, scaling) are new signal types. |
| `spec/12-guidance.md` (GUIDANCE) | R(t) routing function extends to topology selection. M(t) tracks coordination methodology adherence. |
| `spec/05-harvest.md` (HARVEST) | Harvest from external coordination logs (Agent Mail, session JSONL). Coordination outcome harvesting. |
| `spec/06-seed.md` (SEED) | Seed includes coordination context (topology recommendations, coupling patterns, agent capabilities). |
| `spec/10-bilateral.md` (BILATERAL) | Bilateral loop extended to topology drift detection. F(T) parallels F(S). |
| `spec/18-trilateral.md` (TRILATERAL) | Extended to quadrilateral: Intent <-> Spec <-> Impl <-> Topology. |

---

## Relationship to Existing ACFS Coordination Stack

| Current Tool | Role | Topology Framework Relationship |
|-------------|------|--------------------------------|
| `br` (beads) | Issue tracking with dependency graph | Task state and dependencies become datoms. br is the ephemeral substrate; datoms are the durable extract. |
| `bv` (beads_viewer) | Graph intelligence (PageRank, betweenness, critical path) | Graph algorithms move into Braid's query engine. bv's triage = Braid's assignment policy Pi. |
| Agent Mail MCP | Message passing, file reservations, threading | Ephemeral conversation substrate (like session JSONL). Decisions harvested; messages are raw substrate. |
| `ntm` | Tmux orchestration, spawning, work assignment | Enactment layer. Braid recommends topology; ntm (or successor) enacts it. |

**Design decision (TD-REPLACE-001):** Braid replaces the intelligence layer (bv's graph analysis,
ntm's assignment logic) with datom-native Datalog queries. The ephemeral communication layer
(Agent Mail) and the enactment layer (ntm/tmux) remain as separate substrates, with harvest
bridging coordination decisions into the datom store.

---

## Traceability to SEED.md

| SEED.md Section | Topology Relevance |
|-----------------|-------------------|
| S4 (Design Commitment #2: Append-only) | Coordination decisions are immutable datoms. Topology history is fully queryable. |
| S4 (Design Commitment #4: CRDT merge) | Merge topology IS coordination topology. Set union merge means topology determines information flow. |
| S5 (Harvest/Seed Lifecycle) | Coordination patterns harvested at session end. Next session seeded with topology recommendations. |
| S6 (Reconciliation Taxonomy) | Eight divergence types map to coordination concerns. Epistemic = agent knowledge gaps. Aleatory = concurrent assertions. Temporal = frontier inconsistency. |
| S7 (Self-Improvement Loop) | Topology learning through bilateral feedback loop. Coupling weights improve over sessions. |
| S8 (Interface Principles) | Dynamic CLAUDE.md includes topology context. Guidance footers include coordination state. |
| S10 (Stage 3: Multi-Agent) | Topology framework is the formal foundation for Stage 3 multi-agent coordination. |

---

## Prior Research Grounding

This exploration builds on prior research findings:

| Research Artifact | Key Finding | How It Informs This Work |
|-------------------|-------------|--------------------------|
| **INS-022** (Coordination Topology as Differentiator) | Coordination mechanism is strongest system differentiator across lineages. Four distinct topologies each optimal for different regime. | Validates that topology selection is worth formalizing. Informs named topology patterns. |
| **INS-005** (Task-Level Regime Routing) | Task-level regime routing outperforms fixed topology by >=10% OWF on mixed portfolios. | Motivates per-task coupling-based topology selection instead of static choice. |
| **Topology Comparison Framework** | Rigorous experimental design: 3 topologies x 6 regimes x 5 classification dimensions. | Provides methodology for validating topology framework empirically. |
| **Swarm Kernel Architecture** | Six convergent operators (K, D, M, C, IV, L). Three regime zones (Solo 1-3, Team 3-12, Fleet 12+). | Informs scaling policy thresholds and regime classification. |
| **cm rule b-mm42aqga** | Architect-to-Worker pattern is trajectory-optimal. Pass curated artifacts, not transcripts. | Supports the harvest functor model: extract decisions, not raw conversation. |

---

## Key Invariants (Summary)

Full catalog in `09-invariants-catalog.md`. Key invariants for quick reference:

- **INV-TOPO-TRANS-001**: Monotonic transitions never decrease channel count
- **INV-TOPO-TRANS-002**: Grace period ensures no in-flight merge data loss
- **INV-TOPO-TRANS-003**: Agent graph connected after every enacted transition
- **INV-TOPO-COLD-001**: Coordination intensity monotonically relaxes (never tightens without evidence)
- **INV-TOPO-COLD-002**: Cold-start topology >= coupling-optimal intensity (safe upper bound)
- **INV-TOPO-FIT-001**: F(T) monotonically improves under bilateral loop (within noise margin)
- **INV-TOPO-FIT-002**: F(T) and F(S) are independent (no confounding)
- **INV-TOPO-COMPILE-001**: Compiled topology never under-coordinates vs spec structure
- **INV-TOPO-COMPILE-002**: Compiled topology weakly dominates cold-start for spec-bearing projects
- **INV-TOPO-COMPILE-003**: PGO prediction error decreases monotonically after warm-up

---

## Key Design Decisions (Summary)

Full catalog in `10-design-decisions.md`. Key decisions for quick reference:

- **TD-SUBSTRATE-001**: Coordination messages are ephemeral substrate; decisions are datoms
- **TD-ISOMORPHISM-001**: Human/AI and AI/AI coordination are structurally identical
- **TD-COUPLING-001**: Composite coupling signal with five mechanisms and learnable weights
- **TD-CALM-001**: CALM compliance stratifies topology transitions (monotonic = no barrier)
- **TD-AUTHORITY-001**: Scaling authority A(d) = R(1-C)T with earned trust
- **TD-COLDSTART-001**: Monotonic relaxation from mesh (minimax strategy)
- **TD-FITNESS-001**: Seven-dimensional F(T) with bilateral convergence loop
- **TD-REPLACE-001**: Braid replaces intelligence layer; ephemeral substrate remains separate
- **TD-COMPILE-001**: Hybrid AOT+JIT topology compilation with PGO from bilateral feedback

---

*This exploration is itself an instance of the DDIS methodology: formalize before building,
separate exploration from execution, and ensure every claim has traceability and falsification
conditions. The documents below are the exploration artifact. The future specification work
will distill them into formal invariants, ADRs, and negative cases within the Braid spec.*
