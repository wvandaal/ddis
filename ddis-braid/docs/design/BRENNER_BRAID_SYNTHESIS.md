# The Brenner–Braid Synthesis: Cross-Examination and Autonomous Invariant Discovery

> **Purpose**: Comprehensive cross-examination of the Braid epistemological runtime
> with Sydney Brenner's scientific methodology (operationalized in brenner_bot).
> Identifies structural isomorphisms, asymmetric capabilities, and six concrete
> integration proposals culminating in the keystone: **Autonomous Invariant Discovery**.
>
> **Primary Sources**:
> - Braid: `SEED.md`, `docs/design/ADRS.md` (ADR-FOUNDATION-012..033),
>   `docs/design/STEERING_MANIFOLD.md`, `spec/` (21 namespaces),
>   `crates/braid-kernel/src/` (~3600 KLOC), `crates/braid/src/` (CLI)
> - Brenner: `https://github.com/Dicklesworthstone/brenner_bot` —
>   `brenner.ts` (~7500 LOC), `apps/web/src/lib/artifact-merge.ts`,
>   `specs/operator_library_v0.1.md`, `specs/artifact_schema_v0.1.md`,
>   `specs/evaluation_rubric_v0.1.md`, `specs/artifact_delta_spec_v0.1.md`
> - Session history: Sessions 033 (convergence engine), 034 (complete formalism),
>   038 (Y-combinator), 042 (steering manifold), 047 (perf+daemon), 048 (C9 epic),
>   049 (integration tests). IDs: `90df6190`, `c01bb082`, `0128002a`, `56ece7bd`,
>   `d9ac2e6b` (verified via `cass search`)
> - Memory files: `bedrock-vision.md`, `session-034-formalism.md`,
>   `session-038-y-combinator.md`
>
> **Session**: 050 (2026-03-28)
>
> **Traces to**: ADR-FOUNDATION-014 (Convergence Thesis), ADR-FOUNDATION-017
> (Hypothetico-Deductive Loop), ADR-FOUNDATION-020 (Falsification-First Principle),
> INV-FOUNDATION-006 (Kernel Methodology Agnosticism), C7 (Self-Bootstrap),
> C8 (Substrate Independence)

---

## Table of Contents

1. [Research Reports](#1-research-reports)
   - [1.1 Braid Kernel Architecture](#11-braid-kernel-architecture)
   - [1.2 Braid CLI Architecture](#12-braid-cli-architecture)
   - [1.3 DDIS Specification Deep Dive](#13-ddis-specification-deep-dive)
   - [1.4 Session History Analysis](#14-session-history-analysis)
   - [1.5 Brenner Bot Deep Dive](#15-brenner-bot-deep-dive)
2. [Structural Isomorphism](#2-structural-isomorphism)
3. [Asymmetric Capabilities: Brenner Has, Braid Lacks](#3-asymmetric-capabilities-brenner-has-braid-lacks)
4. [Asymmetric Capabilities: Braid Has, Brenner Lacks](#4-asymmetric-capabilities-braid-has-brenner-lacks)
5. [Six Integration Proposals](#5-six-integration-proposals)
6. [The Keystone: Autonomous Invariant Discovery](#6-the-keystone-autonomous-invariant-discovery)
7. [Critical Assessment](#7-critical-assessment)
8. [Implementation Roadmap](#8-implementation-roadmap)

---

## 1. Research Reports

### 1.1 Braid Kernel Architecture

**Source**: `/data/projects/ddis/ddis-braid/crates/braid-kernel/` (~3600 KLOC)
**Methodology**: Thorough exploration of all major modules with 200+ line reads per file,
tracing imports and exports between modules.

#### Overview

The `braid-kernel` crate is a **pure computation library** implementing a formal knowledge
management system. It has **no IO, no async, no filesystem access, no network** — every
function is deterministic. The crate is designed for property-based testing and bounded
model checking (Kani proofs, StateRight models).

The fundamental unit of information is the **datom** — a five-tuple
`[entity, attribute, value, transaction, operation]`. The entire system is built around
an append-only datom store that forms a **G-Set CvRDT** (grow-only set, convergent
replicated data type) under set union.

#### 1.1.1 Datom Store Architecture (`src/store.rs`, `src/datom.rs`)

**File**: `crates/braid-kernel/src/datom.rs` (25 KB)
**File**: `crates/braid-kernel/src/store.rs` (241 KB)

**Core Types:**

- **`EntityId`** — 32-byte BLAKE3 hash of content. Content-addressed: `EntityId = BLAKE3(content)`.
  The inner field is private, enforcing INV-STORE-003 by construction. There is a `ZERO`
  sentinel for placeholder use.

- **`Attribute`** — Namespaced keyword string `:namespace/name`. Validated on construction:
  must start with `:`, contain exactly one `/`, be ASCII-only.

- **`Value`** — 9-variant enum: String, Keyword, Boolean, Long(i64), Double(OrderedFloat<f64>),
  Instant(u64), Uuid([u8;16]), Ref(EntityId), Bytes(Vec<u8>).

- **`TxId`** — Hybrid Logical Clock with three fields: `wall_time: u64` (millis since epoch),
  `logical: u32` (same-millisecond ordering), `agent: AgentId`. Total order:
  `(wall_time, logical, agent)` lexicographic. The `tick()` method handles clock regression
  by maintaining monotonicity. The `merge()` method takes the pointwise max of two HLCs.

- **`AgentId`** — 16-byte BLAKE3 truncation of agent name.

- **`ProvenanceType`** — Total-order lattice:
  `Hypothesized(0.2) < Inferred(0.5) < Derived(0.8) < Observed(1.0)`.

- **`Op`** — `Assert | Retract`. Retractions are themselves datoms; the store never deletes.

**Algebraic Structure:**

The `Store` is `(P(D), ∪)` — a join-semilattice satisfying:
- **L1** Commutativity: `S1 ∪ S2 = S2 ∪ S1`
- **L2** Associativity: `(S1 ∪ S2) ∪ S3 = S1 ∪ (S2 ∪ S3)`
- **L3** Idempotency: `S ∪ S = S`
- **L4** Monotonicity: `S ⊆ (S ∪ T)`
- **L5** Bottom: `∅ ∪ S = S`

**Storage:**

The canonical state is a `BTreeSet<Datom>` (ordered by all 5 fields = EAVT index). Six
secondary indexes are maintained incrementally:
1. **entity_index**: `BTreeMap<EntityId, Vec<Datom>>` — O(1) entity lookups
2. **attribute_index**: `BTreeMap<Attribute, Vec<Datom>>` — O(1) attribute lookups
3. **vaet_index**: `BTreeMap<EntityId, Vec<Datom>>` — reverse reference traversal
   (target → referencing datoms)
4. **avet_index**: `BTreeMap<(Attribute, Value), Vec<Datom>>` — unique lookups and range scans
5. **live_view**: `BTreeMap<(EntityId, Attribute), (Value, TxId)>` — LIVE view, current
   resolved value via LWW
6. **MaterializedViews** — incremental F(S) fitness accumulators (O(1) fitness reads)

All indexes are updated by the single `index_datom()` method on every insertion.

**Transaction Typestate Pattern:**

Transactions use Rust's typestate pattern via sealed traits:
- `Transaction<Building>` — accepts assertions/retractions
- `Transaction<Committed>` — validated, sealed with TxId
- `Transaction<Applied>` — applied to store, receipt available

Transitions are enforced at compile time: `Building → commit(store) → Committed →
transact(store) → Applied`.

**Merge:**

`store.merge(other)` performs pure set union of `BTreeSet<Datom>`. The frontier is updated
via pointwise-max per agent (vector clock semantics). After merge, ALL indexes and
materialized views are rebuilt from scratch. A five-step cascade follows: conflict detection,
schema rebuild, resolution recompute, LIVE invalidation, and trilateral metrics update.

**Frontier:**

`Frontier = HashMap<AgentId, TxId>` — equivalent to a vector clock. Supports time-travel
queries via `Frontier::at(store, cutoff)`.

#### 1.1.2 Harvest/Seed Lifecycle (`src/harvest.rs`, `src/seed.rs`)

**File**: `crates/braid-kernel/src/harvest.rs` (170 KB)
**File**: `crates/braid-kernel/src/seed.rs` (210 KB)

**Harvest** is end-of-session epistemic gap detection. **Seed** is start-of-session context
assembly. They are complementary:

**Harvest Pipeline (INV-HARVEST-005):**

`DETECT → CLASSIFY → SCORE → PROPOSE → REVIEW → COMMIT → RECORD`

The v2 pipeline (`harvest_pipeline`) works in three phases:
1. **Tx-log extraction** — scans the store for datoms with `tx > session_start_tx`, builds
   `EntityProfile` for each entity touched during the session. Profiles capture attributes,
   namespace classification counts, ident/doc values, ref count, etc.
2. **Classification + Scoring + Gap detection** — classifies each profile by dominant
   attribute namespace (Intent → Decision/Observation, Spec → Dependency/Uncertainty,
   Impl → Dependency, Meta → inferred). Scores by information density. Detects completeness
   gaps (e.g., a spec entity missing `:spec/falsification`).
3. **Session knowledge integration** — merges v1 session_knowledge items not already profiled.

Key types:
- `HarvestCandidate` — entity + assertions + category + confidence + weight + reconciliation
  type + status
- `HarvestCategory` — domain-neutral: Observation, Decision, Dependency, Uncertainty
- `CandidateStatus` — total order lattice: Proposed < UnderReview < Committed | Rejected
- Surprisal score: geometric mean of keyword novelty, entity novelty, and confidence delta

**Seed Pipeline (INV-SEED-001):**

`ASSOCIATE → QUERY → ASSEMBLE → EMIT`

- **ASSOCIATE** — discover relevant entities via semantic search or explicit entity lists,
  with bounded traversal (depth × breadth)
- **QUERY** — fetch datoms for discovered entities
- **ASSEMBLE** — organize into five sections: Orientation, Constraints, State, Warnings, Directive
- **EMIT** — rate-distortion compressed via `ProjectionLevel`:
  Full(π₀) > Summary(π₁) > TypeLevel(π₂) > Pointer(π₃)

The seed uses PageRank for entity relevance scoring and truncates intelligently at
sentence/clause boundaries.

#### 1.1.3 Bilateral System (`src/bilateral.rs`)

**File**: `crates/braid-kernel/src/bilateral.rs` (176 KB)

The bilateral coherence verification loop is a **discrete dynamical system** on the lattice
of store states. Its key mathematical contribution is the fitness function F(S), a
**Lyapunov function** satisfying monotonic convergence: `F(S(t+1)) ≥ F(S(t))` (Law L1).

**7-Component Fitness Function F(S) ∈ [0,1]:**

```
F(S) = 0.18×V + 0.18×C + 0.18×D + 0.13×H + 0.13×K + 0.08×I + 0.12×U
```

- **V** (Validation): fraction of spec elements with witness evidence, depth-weighted
- **C** (Coverage): spec-impl coverage ratio from forward scan, depth-weighted
- **D** (Drift): `1 - Φ/Φ_max` (normalized divergence complement)
- **H** (Harvest quality): methodology score M(t)
- **K** (Contradiction): `1 - (conflict_count / total_multi_valued_attrs)`
- **I** (Incompleteness): `1 - (incomplete_specs / total_specs)`, 4-tier partial credit
- **U** (Uncertainty): mean confidence across exploration entities

Weights are resolvable from a `PolicyConfig` via `FitnessWeights::from_policy()`.

**Five Coherence Conditions (CC-1..CC-5):**

- CC-1: No contradiction in spec (machine-evaluable)
- CC-2: Impl satisfies Spec (machine-evaluable)
- CC-3: Spec approximates Intent (human-gated)
- CC-4: Agent agreement via store union (machine-evaluable)
- CC-5: Methodology adherence > 0.5 threshold

**Spectral Certificate:**

The bilateral loop produces a spectral certificate combining:
- **Fiedler value λ₂**: algebraic connectivity of the entity graph
- **Cheeger constant h(G)**: isoperimetric ratio, bounded by Cheeger inequality
  `λ₂/2 ≤ h(G) ≤ √(2λ₂)`
- **Persistent homology**: topological stability via transaction barcode
- **Rényi entropy spectrum**: S₀ (Hartley), S₁ (von Neumann), S₂ (collision), S_∞ (min-entropy)
- **Entropy decomposition**: `S₃ = S₁ + ΔS_boundary + ΔS_ISP`
- **Ollivier-Ricci curvature**: mean and minimum across edges (positive = clustered,
  negative = bottleneck)

Convergence rate is bounded by `1 - exp(-spectral_gap)`, and mixing time is
`O(log(n)/spectral_gap)`.

**BoundaryCheck Trait:**

An extensible object-safe trait allowing new divergence types to participate in F(S) without
modifying the core. The `BoundaryRegistry` normalizes weights and computes
`F(S)_boundaries = Σ((w_i / W_total) × coverage(b_i))`.

**Comonadic Depth (DC-1):**

Entities have a depth level stored as `:comonad/depth`: 0=OPINION, 1=syntactic, 2=structural,
3=property, 4=formal (KNOWLEDGE). The depth_weight mapping:
`[0→0.0, 1→0.15, 2→0.4, 3→0.7, 4→1.0]`.

**FitnessDelta Gradient:**

`MaterializedViews::project_delta()` computes the exact 7-dimensional `ΔF(S)` vector for
hypothetical datoms WITHOUT mutating state. This is gradient computation on the coherence
manifold, used for routing tasks by projected fitness improvement.

#### 1.1.4 Trilateral System (`src/trilateral.rs`)

**File**: `crates/braid-kernel/src/trilateral.rs` (85 KB)

The trilateral model extends bilateral coherence to the full **ISP (Intent-Specification-
Implementation) triangle**. It partitions all store datoms into three LIVE projections by
attribute namespace.

**Attribute Namespace Partition (INV-TRILATERAL-005):**

- **Intent**: `:intent/decision`, `:intent/rationale`, `:intent/source`, `:intent/goal`, etc.
- **Specification**: `:spec/id`, `:spec/element-type`, `:spec/falsification`,
  `:spec/traces-to`, etc.
- **Implementation**: `:impl/signature`, `:impl/implements`, `:impl/file`,
  `:impl/test-result`, etc.
- **Meta**: everything else (`:db/*`, `:tx/*`, etc.)

Configurable via `NamespaceConfig` loaded from policy datoms.

**LIVE Projections (INV-TRILATERAL-001):**

Three monotone functions from store to filtered datom set. `live_projections(store)` returns
`(LiveView_intent, LiveView_spec, LiveView_impl)` using materialized ISP entity sets from
`MaterializedViews`.

**Divergence Metric Φ (INV-TRILATERAL-002):**

```
Φ = w_IS × D_IS + w_SP × D_SP
```

where `D_IS = |Intent \ Spec|` (intent entities not covered by spec),
`D_SP = |Spec \ Impl|` (spec entities not covered by impl). Default weights:
`w_IS=0.4, w_SP=0.6`.

**First Betti Number β₁:**

Computed from the entity reference graph using edge Laplacian eigendecomposition.
`β₁ = 0` means no structural cycles (forest); `β₁ > 0` counts independent cycles
indicating contradictions or circular dependencies.

**Coherence Quadrant (Φ, β₁) Duality (INV-TRILATERAL-009):**

- Coherent: `Φ=0, β₁=0`
- GapsOnly: `Φ>0, β₁=0`
- CyclesOnly: `Φ=0, β₁>0`
- GapsAndCycles: `Φ>0, β₁>0`

**Von Neumann Entropy (INV-COHERENCE-001):**

`S(ρ) = -Tr(ρ log₂ ρ) = -Σ(λᵢ × log₂(λᵢ))` where `ρ = A/Tr(A)` is the density matrix
from the adjacency matrix. Low entropy = coherent; high entropy = dispersed.

#### 1.1.5 Policy System (`src/policy.rs`)

**File**: `crates/braid-kernel/src/policy.rs` (79 KB)

The policy system implements **declarative epistemological policy** (ADR-FOUNDATION-013).
It is explicitly domain-neutral: no attribute or type assumes DDIS or any specific
methodology (NEG-FOUNDATION-003).

**PolicyConfig** is the single source of truth, parsed from store datoms via
`PolicyConfig::from_store()`:

- **BoundaryDef**: source/target entity patterns + weight + report template.
  `F(S) = Σ(weight_i × coverage(boundary_i))`
- **AnomalyDef**: trigger attribute + count threshold + alert message
- **CalibrationConfig**: window size (default 20) + MAE threshold (default 0.05) for the
  self-improving loop
- **NamespaceConfig**: attribute partition overrides for Intent/Spec/Impl
- **Element types**: configurable spec element prefixes (default: INV, ADR, NEG)
- **ISP prefixes**: configurable attribute prefix→counter mappings for non-DDIS projects
- **Harvest overrides**: category weight multipliers and expected attributes per entity type

Policy datoms use `:policy/*` namespace attributes. When no policy exists, callers fall
back to hardcoded DDIS defaults. The system feeds into a closed loop: every policy element
participates in calibration.

#### 1.1.6 Routing/Guidance System (`src/routing.rs`)

**File**: `crates/braid-kernel/src/routing.rs` (105 KB)

The routing system implements **R(t) graph-based work prioritization**.

**Session Working Set (SWS):**

Tracks temporal locality for routing: active tasks (in-progress since session boundary),
session-created tasks, and EPIC siblings. Session boundary =
max(session.started_at, last_harvest_wall_time, now - 3600).

**R(t) Impact Score:**

The composite impact score for each task is a weighted sum of 6 features:
- g₁: PageRank (dependency authority) — weight 0.25
- g₂: betweenness proxy (degree product) — weight 0.25
- g₃: critical path position — weight 0.20
- g₄: blocker ratio (fraction of all tasks this unblocks) — weight 0.15
- g₅: staleness (age/max_age) — weight 0.10
- g₆: priority boost (from metadata) — weight 0.05

Post-factors applied multiplicatively:
- **type_multiplier**: impl/bug=1.0, feature=0.9, test=0.8, epic=0.0, docs=0.3, question=0.2
- **urgency_decay**: `1.0 + ln(age_days + 1) × 0.1`
- **spec_anchor**: 0.3/0.7/1.0 based on task-to-spec resolution
- **session_boost**: 3.0 (active), 2.0 (EPIC sibling), 1.5 (session-created), 1.0 (default)
- **gradient_delta**: projected F(S) change if task completed
- **observation_dampening**: `1/(1 + 0.3×N)` for N observations, or 1.2 for 0 observations (CE-OAR)

**Learned Routing Weights (RFL-4):**

Weights are learned from action-outcome history via **ridge regression**:
`w = (XᵀX + λI)⁻¹Xᵀy` where `λ=0.01`. Requires minimum 50 data points, weights clamped
to [0.01, 0.5], normalized to sum to 1.0.

**Hypothesis Ledger:**

Records predicted actions with outcomes for calibration. The system tracks hypothesis
completion rates and uses them for weight refitting.

#### 1.1.7 Spec ID System (`src/spec_id.rs`)

**File**: `crates/braid-kernel/src/spec_id.rs` (11 KB)

Implements a **provable bijection** between human-readable and machine-readable specification
element identifiers.

- Human form: `INV-GUIDANCE-022`, `ADR-STORE-012`, `NEG-MERGE-001`
- Store ident: `:spec/inv-guidance-022`, `:spec/adr-store-012`

The `SpecId` type stores normalized components `(element_type, namespace, number)`. The
bijection property `denormalize(normalize(h)) = h` is verified by test. Case-insensitive
parsing.

Configurable element types via `parse_with_types()` for non-DDIS domains
(e.g., `["REQ", "CTRL"]`).

#### 1.1.8 Topology Module (`src/topology.rs`)

**File**: `crates/braid-kernel/src/topology.rs` (108 KB)

Implements **ADR-TOPOLOGY-004 (Topology as Compilation)** for multi-agent coordination.

**Pipeline:**

1. **Front-end**: Extract file coupling via Jaccard similarity:
   `J(A,B) = |A ∩ B| / |A ∪ B|`
2. **Middle-end**: Partition tasks into disjoint groups via connected components (BFS)
3. **Back-end**: Assign groups to agents via greedy bin-packing
   (longest-processing-time-first heuristic for makespan minimization)
4. **Emit**: Produce a `TopologyPlan` with per-agent assignments, coupling entropy,
   parallelizability

**Density Matrix Formalism:**

The normalized coupling matrix `ρ_C = C / Tr(C)` is a **density matrix** (PSD, unit-trace,
symmetric) whose:
- **Von Neumann entropy** `S(ρ_C) = -Tr(ρ_C log ρ_C)` equals the irreducible coordination
  complexity
- **Effective rank** `r_eff = exp(S)` = optimal number of parallel agent groups
- **Parallelizability** `p = r_eff / n` (topology-level Amdahl's Law)

**Spectral Partition:**

`spectral_partition()` recursively bisects groups using the **Fiedler vector** (eigenvector
of the 2nd smallest eigenvalue of the graph Laplacian). `fiedler_bisect()` builds the
induced subgraph Laplacian, computes eigendecomposition, and splits on the sign of the
Fiedler vector. Falls back to half-split when algebraic connectivity is near zero.

**Partition quality** = `1 - (inter_cluster_coupling / total_coupling)`.

**Invariant Coupling:**

Beyond file coupling, `compute_invariant_coupling()` measures semantic overlap: tasks
implementing specs that transitively depend on each other (via `:spec/traces-to` BFS
reachability + Jaccard).

**CALM Tier Classification (ADR-TOPOLOGY-002):**

`CalmTier::MonotonicParallel` (can execute without coordination) vs
`CalmTier::NonMonotonicBarrier` (requires sync). Based on the CALM theorem
(Consistency As Logical Monotonicity).

**Topology Patterns:** Mesh (all independent), Star (one dominant group), Hybrid (multiple
medium groups), Solo (single agent), Pipeline (linear chain).

#### 1.1.9 Resolution System (`src/resolution.rs`, `src/merge.rs`)

**File**: `crates/braid-kernel/src/resolution.rs` (64 KB)
**File**: `crates/braid-kernel/src/merge.rs` (55 KB)

**Three-mode join-semilattice:**

Each attribute declares its resolution mode:

1. **LWW** (Last-Writer-Wins): Total order by `(TxId, BLAKE3_tiebreaker)`. Meet = max.
   Commutative, associative, idempotent. Ties broken by BLAKE3 hash of value.

2. **Lattice**: User-defined partial order with lub. Diamond patterns produce error signals.
   At Stage 0, falls back to LWW (lattice definitions deferred to Stage 1).

3. **MultiValue**: Set union. Meet = union. Multi-value mode never conflicts.

**Six-condition conflict predicate (INV-RESOLUTION-004):**

A conflict exists when ALL hold: (1) same entity, (2) same attribute, (3) different values,
(4) both assertions, (5) cardinality `:one`, (6) causally independent. Conservative: may
have false positives, never false negatives.

**Resolution Pipeline:**

`detect_conflicts()` → `resolve_with_trail()` → `conflict_to_datoms()` (audit trail).

`ConflictEntity` captures full provenance: conflicting values, transaction IDs, detection
timestamp. `ResolutionRecord` captures the winning value, mode, and resolution tx.
Resolution decisions become first-class queryable facts via `:resolution/*` datoms.

`verify_convergence()` checks determinism by resolving all (entity, attribute) pairs twice
plus with reversed and rotated assertion orders, confirming all produce identical results.

**Merge Cascade (INV-MERGE-009):**

Five steps after set-union merge:
1. Conflict detection (fully implemented)
2. Schema rebuild
3. Resolution recompute for conflicting pairs
4. LIVE index invalidation
5. Trilateral metrics update

Steps 2-5 produce stub datoms at Stage 0. `CascadeReceipt` captures conflicts, schema
changes, stale LIVE views.

#### 1.1.10 Query Engine (`src/query/`)

**File**: `crates/braid-kernel/src/query/mod.rs` and submodules

A Datalog-like query engine with naive bottom-up fixpoint evaluation (Knaster-Tarski),
CALM compliance for Stage 0-1 (monotonic queries only). Six-stratum classification.

The `graph.rs` submodule implements a comprehensive graph algorithm library:
- Topological sort (Kahn's algorithm), SCC (Tarjan's), PageRank, HITS, betweenness
  centrality, k-core decomposition
- Graph/edge Laplacian, Fiedler vector, Cheeger constant
- Spectral decomposition (Jacobi eigendecomposition), Lanczos method
- Persistent homology (birth-death diagrams), Ollivier-Ricci curvature
- Kirchhoff index, heat kernel trace
- Sheaf cohomology (cellular sheaf, conflict sheaf)

All graph algorithms are deterministic (INV-QUERY-017).

#### 1.1.11 Key Invariant Summary

The system maintains several fundamental mathematical invariants:

- **G-Set CvRDT**: Store is a join-semilattice under set union (commutative, associative,
  idempotent, monotonic)
- **Content-addressability**: EntityId = BLAKE3(content), never mutable
- **HLC monotonicity**: TxId ordering respects causality
- **Lyapunov convergence**: F(S) is monotonically non-decreasing under bilateral operations
- **(Φ, β₁) duality**: Coherent iff both divergence and cycle count are zero
- **Frontier computability**: Frontier derivable from datom set alone
- **Resolution commutativity**: Same inputs produce same resolved value regardless of order
- **Seed-harvest complement**: Every seed datum traces to a datom; every session terminates
  with harvest
- **Disjointness invariant**: In topology plans, no file appears in more than one agent's
  assignment

---

### 1.2 Braid CLI Architecture

**Source**: `/data/projects/ddis/ddis-braid/crates/braid/`
**Methodology**: Thorough exploration of daemon, commands, output, and integration tests.

#### 1.2.1 Main CLI Structure and Request Routing (`main.rs`)

The `Cli` struct uses clap derive to parse arguments. Global flags include `--budget`,
`--context-used`, `--format`, `--orientation`, and `--quiet`. The `command` field is
`Option<commands::Command>` — when absent, it defaults to `Command::Status` (bare `braid`
shows a terse dashboard).

**Execution flow in `main()`:**

1. **Parse CLI** via `Cli::parse()`
2. **Resolve output mode** using `output::resolve_mode()` — priority:
   `--format` flag > `BRAID_OUTPUT` env > TTY detection > default (Agent)
3. **Handle `--orientation`** — prints agent onboarding prompt, returns
4. **Default to Status** — if no subcommand, creates `Command::Status`
5. **Budget context** — `BudgetCtx::from_flags()` computes `k*_eff` attention quality
6. **Extract metadata** — `command_name_for()`, `is_harvest_command()`,
   `is_budget_exempt()` before cmd is consumed
7. **Resolve store path** — `resolve_store_path()` walks up directory tree
   (like git finds `.git/`)
8. **DAEMON ROUTING (INV-DAEMON-007)** — `daemon::try_route_through_daemon()` is called
   FIRST, before opening LiveStore. If daemon is running, CLI marshals command to JSON-RPC
   `tools/call` request over Unix socket.
9. **Direct mode fallback** — if daemon unavailable, opens `LiveStore::open()` once
   (L1-SINGLE invariant)
10. **Session auto-detect** — `detect_session_start()` checks/creates session
11. **Command dispatch** — `commands::run(cmd, budget_ctx, mode, quiet, live)`
12. **Post-command hooks**: RFL-2 (action recording), AR-2 (reconciliation trace),
    NEG-HARVEST-001 (unharvested work warning), divergence detection
13. **Budget gate** — `apply_budget_gate()` enforces per-command token ceiling
14. **Render** — `cmd_output.render(mode)` selects JSON/TSV/Agent/Human rendering

#### 1.2.2 Daemon Architecture (`daemon.rs`)

**File**: `crates/braid/src/daemon.rs` (~2000 lines)

The daemon is a Unix-socket server holding a single `LiveStore` in memory, serving JSON-RPC
requests using the same protocol as the MCP server.

**Architecture (DS2/DS4 concurrency model):**

- **Single shared store**: `Arc<RwLock<LiveStore>>` — one in-memory store shared across
  all connections
- **Thread-per-connection**: Accept loop spawns one OS thread per connection
  (INV-DAEMON-012: accept loop never blocks on dispatch)
- **Write lock per-request**: Each `tools/call` acquires write lock, releases it, then
  submits runtime observation TxFile via commit channel
- **Group commit thread (DS2)**: Dedicated `braid-group-commit` thread owns `WalWriter`.
  Connection threads submit `CommitRequest`s via `CommitHandle` (mpsc). Commit thread
  batches writes (adaptive: 50ms default, 5ms under load), single WAL fsync, then
  acquires write lock to apply datoms.
- **Checkpoint thread (DS3)**: Background WAL-to-EDN at configurable intervals (default 60s)

**Startup sequence (`serve_daemon()`):**

1. Acquire lock file (`daemon.lock`) — atomic `O_CREAT|O_EXCL` (INV-DAEMON-001)
2. Open LiveStore with WAL recovery (DS5: three-level recovery)
3. Install runtime schema (`:runtime/command`, `:runtime/latency-us`, etc.)
4. Run capability census (PERF-4a) — records subsystems as `:capability/*` datoms
5. Run reflexive FEGH (PERF-4b) — bridge hypothesis generation
6. Read config for idle timeout (300s) and checkpoint interval (60s)
7. Wrap LiveStore in `Arc<RwLock>`
8. Spawn group commit thread
9. Spawn checkpoint thread
10. Remove stale socket, bind new Unix socket
11. Install SIGTERM/SIGINT handlers
12. Enter accept loop

**Runtime observation (INV-DAEMON-003):**

`build_observation_tx()` wraps every `tools/call` with timing. Records `:runtime/command`,
`:runtime/request-id`, `:runtime/latency-us`, `:runtime/outcome`, `:runtime/datom-count`,
`:runtime/cache-hit` as datoms. The daemon's operational behavior is visible to its own
analytical tools.

**Auto-routing (`try_route_through_daemon()`):**

When CLI detects no daemon, auto-starts one: forks detached child running
`braid daemon start`, polls for socket (50ms intervals, 3s max), falls back to direct
mode on timeout. `marshal_command()` maps 11 CLI commands to MCP tool names + JSON args.

#### 1.2.3 Command Definitions (`commands/mod.rs`)

The `Command` enum defines all subcommands, organized by workflow phase:
- **SETUP**: `Init`
- **CAPTURE**: `Observe`, `Write`, `Note`, `Wrap`
- **QUERY**: `Query`, `Status`, `Log`, `Schema`
- **COHERENCE**: `Bilateral`, `Trace`, `Verify`, `Challenge`, `Witness`
- **LIFECYCLE**: `Harvest`, `Seed`, `Session`
- **TASK MANAGEMENT**: `Task`, `Next`, `Done`, `Go`
- **ADMIN**: `Shell`, `Mcp`, `Daemon`, `Model`, `Config`
- **COORDINATION**: `Topology`, `Spec`
- **SHORTCUTS**: `Transact`, `Extract`

#### 1.2.4 Status Dashboard (`commands/status.rs`)

The status command is the primary orientation tool — bare `braid` resolves to it.

**Modes:**
- **Terse** (default, <150 tokens): store metrics + F(S) + M(t) + harvest status + next action
- **Verbose** (`--verbose`): full methodology breakdown + all actions
- **Deep** (`--deep`): bilateral F(S) scan + graph analytics + convergence trajectory
- **Verify** (`--verify`): on-disk integrity check

**`StatusSnapshot`:** Pre-computes all expensive values once: fitness (O(1) from
materialized views), coherence (Φ, β₁, quadrant), telemetry, methodology_score with
trend, task_counts, ready_set, trace_status, adjusted_gaps.

**Side-effect writes during status:**
- **UAQ-6**: Records which context blocks were "presented" (attention tracking)
- **ATT-2-IMPL**: When `--verbose` requested, applies Hebbian boosts to blocks,
  causing frequently-requested sections to auto-promote

#### 1.2.5 Output System (`output.rs`)

**Four modes** (INV-OUTPUT-001):
- `Json` — complete structured data via `serde_json::to_string_pretty()`
- `Tsv` — tab-separated values rendered from JSON
- `Agent` — three-part structure (context/content/footer, ~300 tokens)
- `Human` — full formatted text

`CommandOutput` carries all three representations. Every command handler builds one.
`render_projected()` supports ACP budget-constrained rendering.

#### 1.2.6 LiveStore (`live_store.rs`)

Write-through persistent store unifying `DiskLayout` and `Store`. Key properties:
- `write_tx()` atomically updates in-memory store AND transaction log
- `store.bin` serialization on `flush()` or `Drop` only (dirty-flag batching)
- `refresh_if_needed()` uses O(1) mtime-based detection of external writes
- `open_with_wal()` provides three-level recovery

#### 1.2.7 Integration Tests (`tests/daemon_integration.rs`)

Eight test categories exercising daemon through actual interfaces:
1. Daemon lifecycle (start/stop, lock files, graceful shutdown)
2. Socket communication (MCP handshake, tools/list, error codes)
3. Tool dispatch (observe/query round-trip, task lifecycle)
4. Runtime observation (datom generation verification)
5. Semantic equivalence (daemon vs direct mode produce same state)
6. Multi-connection (concurrent socket access, no deadlock)
7. Checkpoint (WAL→EDN conversion)
8. Cross-process (write through daemon, verify via CLI direct)

---

### 1.3 DDIS Specification Deep Dive

**Source**: `SEED.md`, `spec/`, `docs/design/ADRS.md`, `docs/design/STEERING_MANIFOLD.md`,
`docs/design/FAILURE_MODES.md`
**Methodology**: Deep read of all foundational documents, tracing ADRs and invariants
through related spec components.

#### 1.3.1 The Central Thesis

DDIS is a **formal epistemology runtime**. It is not a database, not a memory system, not
a documentation tool. It is a substrate for maintaining **verifiable coherence** between
intent, specification, implementation, and observed behavior. From SEED.md §1:

> "to be able to say, at any point in a project of any scale, with formal justification
> rather than subjective confidence: I know what I want. It is logically coherent and
> internally consistent."

The memory problem (AI agents losing knowledge across session boundaries) is the
*presenting symptom*; **divergence** is the deeper disease (CO-001). Divergence occurs at
every boundary in the chain from intent to reality: Intent to Specification (axiological),
Specification to Specification (logical), Specification to Implementation (structural), and
Implementation to Observed Behavior (behavioral).

#### 1.3.2 The 8+1 Divergence Taxonomy (CO-003, SEED.md §6)

| # | Type | Boundary | Detection | Resolution |
|---|------|----------|-----------|------------|
| 1 | **Epistemic (EP)** | Store vs. agent knowledge | Harvest gap detection | Harvest |
| 2 | **Structural (ST)** | Implementation vs. spec | Bilateral scan / drift | Reimplementation |
| 3 | **Consequential (CO)** | Current state vs. future risk | Uncertainty tensor | Guidance |
| 4 | **Aleatory (AL)** | Agent vs. agent | Merge conflict detection | Deliberation |
| 5 | **Logical (LO)** | Invariant vs. invariant | Contradiction detection | New ADR |
| 6 | **Axiological (AX)** | Implementation vs. goals | Fitness, goal-drift | Human review |
| 7 | **Temporal (TE)** | Frontier vs. frontier | Frontier comparison | Sync barrier |
| 8 | **Procedural (PR)** | Behavior vs. methodology | Drift detection | Dynamic AGENTS.md |
| 9 | **Reflexive (RX)** | System vs. system's model | Capability census | Self-test |

The **ninth type — Reflexive** — was added in `spec/22-reflexive.md` (ADR-REFLEXIVE-001),
discovered empirically through 5 dog-food friction points in Session 025.

#### 1.3.3 The Three Learning Loops

From `STEERING_MANIFOLD.md` §11 and the OBSERVER formalism:

| Loop | Name | What Improves | Mechanism | Scientific Analogy |
|------|------|---------------|-----------|-------------------|
| **OBSERVER-4** | Calibration | Better hypotheses | Predicted vs actual ΔF(S) → gradient descent | Instrument reliability |
| **OBSERVER-5** | Structure | Better reconciliation | Temporal coupling → proposed boundaries | Discovering phenomena |
| **OBSERVER-6** | Ontology | Better morphisms | Observation clustering → concept entities | Paradigm formation |

Together: the system learns what coherence means (weights), what should be aligned
(structure), and what kinds of knowledge exist (ontology).

#### 1.3.4 The Observation-Projection Duality (ADR-FOUNDATION-016)

From `STEERING_MANIFOLD.md` §5:

| Projection | Source | Target |
|------------|--------|--------|
| Code | Spec | Source files |
| Tests | Invariants | Test suite |
| Seeds | Store through relevance filter | Agent context |
| AGENTS.md | Methodology through behavioral filter | Operating instructions |
| Status | Fitness + tasks + boundaries | CLI output |

Every boundary has two morphisms: observation (in) and projection (out). Convergence:
`observe(project(store)) ≅ store` — "when knowing IS doing."

The **Universal Action Queue** (ADR-FOUNDATION-019) scores both directions:
`α(action | store) = E[ΔF(S)] / cost(action)`, for all action types.

#### 1.3.5 The Y-Combinator for LLMs (Session 038)

**Source**: `STEERING_MANIFOLD.md` §4, `session-038-y-combinator.md`

**Braid is the fixed-point combinator that gives stateless LLMs self-reference.**

```
Y = λf.(λx.f(x x))(λx.f(x x))
```

An LLM is `context → continuation` — stateless. Braid wraps it:

```
seed → agent session → harvest → updated store → new seed → ...
```

When this converges, the seed contains exactly the information to produce the session
that produces the harvest that produces the seed. **The function finds itself in its
own output.**

| Level | Y applied to | Mechanism |
|---|---|---|
| L0 | Memory | Store + seed/harvest |
| L1 | Self-observation | Daemon (introspective observer) |
| L2 | Learning | Calibration (hypothesis ledger) |
| L3 | Inquiry | FEGH bridge hypotheses |
| L4 | Ontology | Ontological surprise detection |
| L5 | Meta-learning | Load-bearing failure escalation |

The hierarchy closes into a loop. The loop IS Y. F(S) → 1.0 IS the fixed point.

#### 1.3.6 The Epistemological Triangle (ADR-FOUNDATION-020/021)

Three irreducible operations underlie all knowledge:

```
ASSERTION (monadic)       — Reality → Store(Datom)     — how knowledge enters
FALSIFICATION (comonadic) — Store(H) → (H, {¬Hᵢ})     — how knowledge is tested
WITNESS (constructive)    — ¬¬H → H                    — how survival becomes knowledge
```

Knowledge progresses through: OPINION (0.0) → HYPOTHESIS (0.15) → TESTED (0.4) →
SURVIVED (0.7) → KNOWLEDGE (1.0).

**Anti-Goodhart Architecture** (ADR-FOUNDATION-021): F(S) rewards survived falsification,
not accumulated confirmation. Three mechanisms: dialectical depth, pre-registered
falsification, SAW-TOOTH invariant (healthy F(S) oscillates).

#### 1.3.7 Substrate Independence (C8)

The kernel must not hardcode DDIS or any methodology (INV-FOUNDATION-006). Evidence:
- Five innate schemas are substrate-independent (Components, Dependencies, Invariants,
  Patterns, Anomalies)
- Policy manifests configure domain-specific behavior
- Configurable element type prefixes (INV/ADR/NEG or REQ/CTRL/etc.)
- Provider-neutral naming (`agent_md`, not `claude_md`)
- Go CLI gap analysis proves concepts work at scale on different substrate

---

### 1.4 Session History Analysis

**Source**: `cass search` and `cm context` queries across all ddis-braid sessions
**Methodology**: 10 targeted searches across key theoretical concepts

#### 1.4.1 Brenner Prior Discussion

8 hits across ddis-braid (3), research (4), and home-ubuntu (1) projects. Prior research
sessions (`b0490209`, `7a0f7c4f`) discussed brenner extensively. One hit in ddis parent
(`9c633064`). This is not the first cross-examination — prior work exists in the research
project context.

#### 1.4.2 Theoretical Epicenter Sessions

**Session c01bb082** (Session 034 in MEMORY.md): The theoretical epicenter. Accounts for:
- 6/11 hits on "formal epistemology"
- 4/6 hits on "hypothetico-deductive"
- 2/6 hits on "observation projection duality"
- ALL 5/5 hits on "divergence taxonomy"

This was the "Complete Formalism + 27-Task Execution Sprint" — Observer Formalism,
Observation-Projection Duality, Hypothetico-Deductive Loop, Hypothesis Ledger, Universal
Action Queue, Three Primitives, Epistemological Triangle, Anti-Goodhart Architecture.

**Session 0128002a** (Session 038): ALL 5 Y-combinator/fixed-point hits. Single focused
subagent deep-dive.

**Session 90df6190** (Session 033): ALL 5 convergence engine hits. Three subagents involved.
"Convergence Engine + Epistemology Breakthrough."

#### 1.4.3 Procedural Memory (cm)

10 rules returned, top by score:
1. `b-mm9dygkx` (2.46): Cleanroom verification methodology for transcript-to-ADRS
2. `b-mm9dyga5` (1.23): Systematic enumeration for design decision coverage
3. `b-mm8qjjcn` (1.22): IMPLEMENTED/STUB/MISSING classification for code-spec audit
4. `b-mm844lpd` (1.22): Quantitative metrics from ddis validate/coverage/drift

No rules specifically about Brenner. No anti-patterns or deprecated warnings relevant
to this synthesis.

---

### 1.5 Brenner Bot Deep Dive

**Source**: `https://github.com/Dicklesworthstone/brenner_bot`
**Methodology**: btca source-code research + WebFetch of README and key source files.
Read `brenner.ts` (~7500 LOC), `artifact-merge.ts`, `operator-library.ts`,
`session-kickoff.ts`, `delta-parser.ts`, all spec files in `specs/`.

#### 1.5.1 Who Is Sydney Brenner

Sydney Brenner (1927-2019): Nobel Prize-winning molecular biologist, co-discoverer of
messenger RNA, established *C. elegans* as a model organism, 2002 Nobel Prize in
Physiology or Medicine. The project draws from **236 video transcripts** of Brenner's
interviews from "Web of Stories."

What makes Brenner's methodology special is not domain knowledge but a **transferable
cognitive operating system for scientific inquiry**. Two foundational axioms:

1. **"Reality has a generative grammar"** — the world operates by discoverable causal rules
2. **"Understanding requires reconstruction"** — you cannot claim understanding without
   specifying how to build the phenomenon from primitives

#### 1.5.2 What Brenner Bot Does

A **research orchestration system** that operationalizes Brenner's methodology through
multi-agent collaboration. NOT a chatbot or knowledge base — a structured protocol for
conducting multi-agent "research lab" sessions that produce **lab-grade artifacts**.

Key components:
- **Corpus**: 236 curated transcript sections with stable `section-n` anchors
- **CLI**: `brenner.ts` (~7500 LOC TypeScript, compiled via Bun)
- **Web app**: Next.js 16 at brennerbot.org
- **Agent Mail integration**: Multi-agent coordination via message passing
- **Artifact compiler**: Deterministic merge of agent contributions

**Critical design constraint**: No vendor AI API calls. All inference via subscription-tier
CLI tools (Claude Code, Codex CLI, Gemini CLI) running in operator-controlled terminals.

#### 1.5.3 Core Data Structures

**Artifact** — The central output, structured research document with 7 mandatory sections:
- **Research Thread (RT)**: One falsifiable bite point
- **Hypothesis Slate (H1-H6)**: 2-5 hypotheses + mandatory "third alternative"
- **Predictions Table (P)**: Discriminative predictions per hypothesis
- **Discriminative Tests (T)**: Ranked decision experiments with potency checks
- **Assumption Ledger (A)**: Load-bearing assumptions + mandatory scale/physics check
- **Anomaly Register (X)**: Quarantined exceptions
- **Adversarial Critique (C)**: Attacks on the artifact's own framing

**Delta** — The atomic unit of collaboration:
```json
{
  "operation": "ADD" | "EDIT" | "KILL",
  "section": "<artifact section>",
  "target_id": "<item ID>" | null,
  "payload": { ... },
  "rationale": "why this change"
}
```
KILL preserves items with strikethrough (never deletes) — paralleling braid's C1.

**Hypothesis State Machine**:
```
proposed → active → confirmed | refuted | superseded
active → deferred → active (reactivation loop)
```
Full transition log with audit trail.

**EvidencePack** — External evidence management: records (EV-001 format) with type,
source, key findings, supports/refutes linkage. Access methods: url, doi, file, session,
manual.

#### 1.5.4 The Operator Algebra

17 operators encode Brenner's cognitive moves:

| Symbol | Name | Action |
|--------|------|--------|
| `level-split` | Level-Split | Separate program/interpreter, message/machine |
| `recode` | Recode | Change representation; reduce dimensionality |
| `invariant-extract` | Invariant-Extract | Find what survives transformations |
| `exclusion-test` | Exclusion-Test | Derive forbidden patterns; lethal tests |
| `materialize` | Materialize | Theory to "what would I see?" |
| `object-transpose` | Object-Transpose | Change substrate until test is easy |
| `quickie` | Quickie | Cheap pilot to de-risk |
| `hal` | HAL | Have A Look (direct observation) |
| `chastity-check` | Chastity-vs-Impotence | Won't vs can't control |
| `scale-check` | Scale-Check | Calculate; stay physically imprisoned |
| `exception-quarantine` | Exception-Quarantine | Isolate anomalies; track Occam's broom |
| `theory-kill` | Theory-Kill | Discard when ugly |
| `paradox-hunt` | Paradox-Hunt | Find contradictions to reveal missing rules |
| `cross-domain` | Cross-Domain | Import unrelated-field patterns |
| `dephase` | Dephase | Move out of phase with fashion |
| `amplify` | Amplify | Use biological amplification |
| `diy` | DIY | Build what you need; don't wait |

**Objective function**: Maximize
`(expected mind-change × downstream option value) / (time × cost × ambiguity)` —
evidence per week.

**Default operator compositions** (chains):
- Standard diagnostic: level-split → recode → extract invariants → derive forbidden patterns
- Theory-to-test: recode → materialize → quickie de-risking
- Hygiene layer: scale-check → quarantine anomalies → theory-kill
- System optimization: transpose organism → amplify signal → build tools

#### 1.5.5 Core Algorithms

**Deterministic Artifact Merge** (`mergeArtifactWithTimestamps`):
- Deep-clone base artifact
- Sort all deltas by timestamp (oldest first)
- Apply each: ADD (generate next ID, validate capacity), EDIT (field-level merge,
  array union for anchors), KILL (strikethrough, check post-conditions)
- Deterministic: same inputs → same output
- Idempotent KILLs, conflict-free by design

**Lint System** (`lintArtifact`):
- ~50 checks across all 7 sections
- Validates: minimum counts, third alternative, scale check, anchor validity (1-236),
  reference integrity, test scoring, potency checks
- Returns violation IDs mapped to operators via `LINT_OPERATOR_GUIDANCE`
- **Key innovation**: lint violations automatically suggest which cognitive operator to apply

**Session Robot Mode** (fully automated multi-agent):
- Each round: 3 role-specific prompts → 3 subprocesses → parse deltas → merge → lint →
  check convergence
- **Convergence criterion**: kill_rate > add_rate (hypotheses eliminated faster than added)
- Step mode: human-in-the-loop between rounds

#### 1.5.6 Multi-Agent Orchestration

Three primary roles mapped to AI models:
- **Hypothesis Generator** (GPT/Codex): Level-Split, Cross-Domain, Paradox-Hunt
- **Test Designer** (Claude/Opus): Exclusion-Test, Materialize, Object-Transpose, Potency-Check
- **Adversarial Critic** (Gemini): Exception-Quarantine, Theory-Kill, Scale-Check

Each agent receives role-specific prompt: triangulated Brenner kernel (shared invariant),
role-specific operators, citation rules, delta format, pre-submission checklist.

**Test Scoring** (4 dimensions, 0-3 each, max 12):
- Likelihood ratio: <2:1 (0) to >100:1 (3)
- Cost: >$100K (0) to <$1K (3)
- Speed: >1 year (0) to <1 week (3)
- Ambiguity: Many confounds (0) to digital readout (3)

#### 1.5.7 12 Epistemic Guardrails

1. Always include third alternative ("both could be wrong")
2. Always include potency/validity checks
3. Use scale as hard prior (calculate actual numbers)
4. Prefer exclusion to accumulation (kill hypotheses, don't collect data)
5. Label but don't panic over missing mechanisms
6. Quarantine exceptions honestly (appendix treatment)
7. Kill theories when ugly, don't protect them
8. Monitor "Occam's broom" carpet height (what's being swept away?)
9. Try inversion when stuck
10. Guard imagination with experiment
11. Reject logically elegant but physically implausible theories
12. Suspect easy metaphorical analogies

#### 1.5.8 Key Design Decisions

1. **No API calls**: All inference through CLI tools — humans always in the loop
2. **Delta-centric evolution**: Structured operations, not prose — deterministic compilation
3. **Append-only kills**: KILL = strikethrough, never delete — full audit trail
4. **Lint-to-operator mapping**: Structural gaps → cognitive interventions
5. **Triangulated kernel**: One shared methodology reference in all prompts
6. **Evidence packs**: External evidence formally tracked with supports/refutes
7. **Experiment capture**: Raw results stored immutably; interpretation via deltas

---

## 2. Structural Isomorphism

These projects solve **the same problem** from opposite ends of the abstraction spectrum.
The isomorphism is precise:

| Brenner_bot | Braid | Shared Structure |
|---|---|---|
| Artifact (7 sections) | Store (datom set, G-Set CvRDT) | Monotonically growing knowledge container |
| Delta (ADD/EDIT/KILL) | Transaction (assert/retract) | Atomic, auditable knowledge mutation |
| KILL = strikethrough | Retraction = new datom `op=retract` | **Append-only epistemology** (C1) |
| Discriminative test scoring | R(t) impact + project_delta | Acquisition function: what to do next |
| Adversarial Critic role | `braid challenge` (comonadic duplicate) | Falsification as structure |
| Lint → Operator guidance | Divergence → Guidance footer | Gap → cognitive intervention |
| Hypothesis state machine | Hypothesis Ledger (`:hypothesis/*`) | Testable claims with lifecycle |
| Operator compositions | Hypothetico-deductive loop | Staged cognitive pipeline |
| Three agent roles | Topology pipeline (spectral partition) | Multi-agent coordination |
| Potency check (positive controls) | Witness system (FBW triple-hash) | Verification of verification |
| "Evidence per week" objective | α = E[ΔF(S)] / cost | Information-per-unit-cost maximization |
| 12 epistemic guardrails | 10 hard constraints (C1-C10) | Structural bounds on reasoning |
| Convergence: kill_rate > add_rate | Convergence: F(S) → 1.0 | Explicit termination condition |
| Third alternative requirement | Uncertainty markers (0.0-1.0) | Epistemic humility as structure |

The parallelism is not coincidental. Both systems formalize the **hypothetico-deductive
method** — the only known general-purpose epistemological engine. Brenner formalized it
from 60 years of bench science. Braid formalized it from first principles via category
theory and information geometry. They converge because **there is only one correct
epistemology** and both found it.

**Verification of convergence claim**: The three shared primitives map exactly:
1. **Morphisms**: Brenner "observations/experiments" = braid "morphisms between reality
   and store" (ADR-FOUNDATION-015, `bedrock-vision.md` line 103)
2. **Reconciliation**: Brenner "adversarial critique" = braid "8-type divergence detection"
   (CO-003, SEED.md §6)
3. **Acquisition function**: Brenner "maximize expected mind-change per cost" = braid
   "α = E[ΔF(S)] / cost" (ADR-FOUNDATION-025/028, `session-034-formalism.md` line 169)

---

## 3. Asymmetric Capabilities: Brenner Has, Braid Lacks

### 3.A The Operator Algebra — A Formal Grammar of Cognitive Moves

**Source**: `specs/operator_library_v0.1.md` (~32 KB), `apps/web/src/lib/operator-library.ts`

This is the most significant gap. Braid tells agents **what** to do (R(t) routing) and
**where** divergence exists (8+1 taxonomy), but never **how to think** about closing it.

Brenner's 17 operators encode specific cognitive transformations. Each has:
- **Triggers**: when this operator should fire
- **Failure modes**: how this operator goes wrong
- **Composition rules**: how operators chain into pipelines
- **Quote-bank anchors**: Brenner's actual reasoning (transcript section references)

**Why braid needs this**: The guidance footer currently says: "CC-2 failing: 12 specs lack
traces → `braid trace`". This tells the agent WHAT is wrong, not HOW to think about fixing
it. With operators: "CC-2 failing → Apply **Invariant-Extract**: what property survives
across all implementations of INV-STORE-003? That's your trace anchor."

**Traces to**: The guidance system (`src/routing.rs`, `maybe_inject_footer()` in
`commands/mod.rs`) already has the wiring point. The operator recommendation would be an
additional field in `GuidanceBlock`.

### 3.B Discriminative Power as First-Class Metric

**Source**: `specs/evaluation_rubric_v0.1.md`, test scoring in `brenner.ts`

Brenner's test scoring: likelihood ratio × cost × speed × ambiguity. The **likelihood
ratio** is the key: how much does this test discriminate between hypotheses?

Braid's `project_delta` (MaterializedViews, `src/bilateral.rs`) measures expected fitness
improvement — **exploitation only**. It asks "how much closer to coherence?" not "how
much does this change our beliefs?"

The difference matters in **metastable states** (F(S) ≈ 0.62 with many small improvements
available). `project_delta` ranks by size. But the most **informative** observation might
have moderate ΔF(S) but enormous **discriminative power** — it reveals whether an entire
boundary model is correct or wrong.

**Traces to**: ADR-FOUNDATION-025/028 (acquisition function), ADR-FOUNDATION-030 (FEGH).
The existing `project_delta` in `MaterializedViews` is the implementation point.

### 3.C The Potency/Chastity Distinction (Control Divergence)

**Source**: Brenner transcript section-50, `specs/operator_library_v0.1.md`
(chastity-check operator)

Brenner's operationally most valuable insight: when a test fails, distinguish
**"won't work" (chastity)** from **"can't detect" (impotence)**. Every experiment must
include positive controls.

Braid has no analog. When an invariant's witness fails, the system records the failure
as evidence against the invariant. But what if the witness itself is broken? The system
can't distinguish "invariant violated" from "test wrong."

**Traces to**: Witness system (`spec/21-witness.md`), FBW in `src/bilateral.rs`.
The existing `:witness/*` schema would gain `:witness/positive-control` Ref.

### 3.D The Anomaly Register (Exception-Quarantine)

**Source**: `specs/artifact_schema_v0.1.md` (Anomaly Register section),
exception-quarantine operator

Brenner treats anomalies as **first-class epistemic objects** — neither ignored nor given
veto power. Quarantined in explicit register, tracked for accumulation, periodically
re-examined. "Occam's broom" — what's swept under the carpet — is itself monitored.

Braid has the 8-type divergence taxonomy but no mechanism for "things that don't fit any
type." The system classifies everything; resistant anomalies get force-fit or silently
dropped.

**Traces to**: The divergence taxonomy (CO-003), harvest pipeline (`src/harvest.rs`).
Could be a new `HarvestCategory::Anomaly` or a dedicated `:anomaly/*` namespace.

### 3.E The Level-Split Operator (Meta-Cognitive Primitive)

**Source**: `specs/operator_library_v0.1.md` (level-split, the first and most powerful
operator)

Brenner's deepest cognitive move: separate program from interpreter, message from machine.
The meta-operation enabling all others.

Braid does this IMPLICITLY (kernel/CLI, policy/substrate, methodology/algebra) but doesn't
have it as an EXPLICIT operator agents can invoke. Making "level-split" first-class would
help agents recognize confusion between abstraction levels — a common failure mode in
agentic coding (documented in `FAILURE_MODES.md` FM-005: Phantom Alignment).

### 3.F Scale-Check as Hard Prior

**Source**: Brenner guardrail #3, scale-check operator in
`specs/operator_library_v0.1.md`

"Stay imprisoned within the physical context." Calculate actual numbers before theorizing.
Absent from braid — F(S) has no mechanism for sanity-checking quantitative claims against
physical/logical constraints.

---

## 4. Asymmetric Capabilities: Braid Has, Brenner Lacks

### 4.A CRDT Merge with Proven Convergence

Brenner_bot: "sort deltas by timestamp, apply sequentially, LWW on collision." No formal
convergence properties. Braid: G-Set CvRDT with proven lattice axioms (L1-L5, Theorems
PO-1 through PO-3 in `spec/01-store.md`). Difference between "usually works" and
"provably always works."

### 4.B The Complete Divergence Taxonomy

Brenner: "hypothesis right/wrong" (binary). Braid: 9 types, each with boundary, detection,
resolution (CO-003, SEED.md §6). Far richer ontology of epistemic failure.

### 4.C The Information-Geometric Substrate

Von Neumann entropy, Fiedler vectors, Ollivier-Ricci curvature, persistent homology,
Cheeger constants, Kirchhoff index, heat kernel traces, sheaf cohomology — all in
`src/query/graph.rs`. Brenner_bot has no geometric model of its knowledge space.

### 4.D Schema-as-Data and Meta-Learning

Braid's C3 (schema as datoms, evolution as transactions) enables self-evolving ontology.
Brenner_bot's schema is hardcoded TypeScript.

### 4.E The Steering Manifold

Every CLI token as steering vector on the LLM's knowledge manifold
(`STEERING_MANIFOLD.md`). Brenner_bot treats prompts as instructions; braid treats them
as **navigation**.

### 4.F Self-Calibration via Hypothesis Ledger

Braid's hypothesis ledger (`:hypothesis/*`, ADR-FOUNDATION-018) provides automated
predicted-vs-actual calibration. Brenner_bot's evaluation is manual rubric-based scoring.

---

## 5. Six Integration Proposals

### 5.1 Cognitive Operator Lattice (COL)

**Import Brenner's operator algebra into braid as policy-layer datoms.**

**Schema** (`:operator/*` namespace):
```
:operator/symbol       — Keyword (e.g., :operator.symbol/level-split)
:operator/trigger      — String (when to fire)
:operator/action       — String (cognitive move)
:operator/failure-mode — String (how it goes wrong)
:operator/composition  — Ref(s) to successor operators
:operator/domain       — Keyword (:universal | :scientific | :engineering | :compliance)
```

**Wiring**: `compute_operator_recommendation(divergence_type, store) →
Option<OperatorRecommendation>` maps each divergence type to default operator sequence:

| Divergence | Primary Operator | Chain |
|---|---|---|
| Epistemic | HAL (Have A Look) | HAL → Invariant-Extract → Recode |
| Structural | Level-Split | Level-Split → Recode → Exclusion-Test |
| Consequential | Materialize | Materialize → Scale-Check → Quickie |
| Aleatory | Cross-Domain | Cross-Domain → Paradox-Hunt → Level-Split |
| Logical | Paradox-Hunt | Paradox-Hunt → Exclusion-Test → Theory-Kill |
| Axiological | Dephase | Dephase → Level-Split → Exception-Quarantine |
| Temporal | Quickie | Quickie → HAL → Invariant-Extract |
| Procedural | DIY | DIY → Object-Transpose → Scale-Check |
| Reflexive | Level-Split | Level-Split → Scale-Check → Invariant-Extract |

**Calibration**: Operator recommendations participate in hypothesis ledger. System LEARNS
which cognitive strategies work for which problems.

**C8 compliance**: Operators enter through policy manifest, not kernel. Kernel provides
`query_operators(store, divergence_type) → Vec<OperatorCard>`. Different domains define
different operator sets.

**Second gradient**: Beyond ∇F(S) (knowledge coherence direction), adds ∇O(S) (optimal
cognitive strategy). System learns not just WHAT to know but HOW to learn.

**Estimated LOC**: ~200 kernel + ~100 CLI wiring
**Traces to**: ADR-FOUNDATION-013 (Policy Manifest), guidance system in `src/routing.rs`

### 5.2 Discriminative Information Gain (DIG)

**Replace project_delta with discriminative scoring measuring expected belief change.**

For each candidate action *a*:

```
DIG(a) = D_KL(P(θ|a,S) ‖ P(θ|S)) × V_downstream(a) / (C_token(a) × A(a))
```

Where:
- `D_KL` = KL divergence between posterior and prior = **expected mind-change**
- `V_downstream` = blocked tasks/hypotheses that become evaluable = **option value**
- `C_token` = estimated token cost
- `A` = ambiguity: competing explanations for expected outcome

**Tractable proxy**: `DIG(a) ≈ |project_delta(a)| × uncertainty(boundary(a)) ×
V_downstream(a) / cost(a)`, where `uncertainty(boundary)` from hypothesis ledger
calibration error.

**Fisher information connection**: `D_KL ≈ ½ δθᵀ F δθ` where F is the Fisher information
matrix. The Fisher information IS the Hessian of the log-likelihood — already implicitly
present in MaterializedViews.

**Solves metastability**: When F(S) = 0.62 with hundreds of small improvements, DIG ranks
by **information value** not ΔF(S). Small-ΔF(S) observations at uncertain boundaries worth
more than large-ΔF(S) at well-calibrated boundaries.

**Estimated LOC**: ~100 in `src/routing.rs`
**Traces to**: ADR-FOUNDATION-025/028 (acquisition function), ADR-FOUNDATION-030 (FEGH)

### 5.3 Structural Adversarial Critique (SAC)

**Formalize adversarial critic as mandatory harvest pipeline phase.**

```
Current:  DETECT → CLASSIFY → SCORE → PROPOSE → REVIEW → COMMIT → RECORD
New:      DETECT → CLASSIFY → SCORE → PROPOSE → RED-TEAM → REVIEW → COMMIT → RECORD
```

RED-TEAM applies three Brenner operators to each harvest candidate:

1. **Scale-Check**: Are quantitative claims physically plausible?
2. **Exception-Quarantine**: Does observation conflict with existing store? Quarantine
   rather than overwrite.
3. **Theory-Kill**: Is observation genuine information or restatement of known knowledge?
   (content-hash dedup + semantic similarity)

**Comonadic depth integration**: Observations surviving red-team start at depth 1
(HYPOTHESIS). Failures stay at depth 0 (OPINION). Directly feeds depth-weighted F(S).

**SAW-TOOTH connection**: Red-team produces the "surprise dips" the SAW-TOOTH invariant
requires. 0% flag rate = echo chamber signal → strengthen red-team. Target: 10-20%
flag rate.

**Estimated LOC**: ~200 in `src/harvest.rs`
**Traces to**: ADR-FOUNDATION-020 (falsification-first), ADR-FOUNDATION-021 (anti-Goodhart),
SAW-TOOTH invariant in `STEERING_MANIFOLD.md` §8

### 5.4 Control Divergence (Type 10)

**Add tenth divergence type capturing potency/chastity distinction.**

```
Type 10: CONTROL — verification system vs. system being verified
Boundary: Test harness ↔ system under test
Detection: Positive control witnesses
Resolution: Fix test before trusting results
```

**Implementation**: Every FBW witness gets optional `:witness/positive-control` Ref to a
witness of known-true property using same verification path. If positive control fails,
witness marked `IMPOTENT` not `FALSIFIED`.

**Why it matters**: False negatives in verification are silent. Test framework bug causing
all witnesses to pass inflates F(S) without genuine coherence. Positive controls detect
this.

**Estimated LOC**: ~50 schema + ~50 bilateral logic
**Traces to**: `spec/21-witness.md`, FBW in `src/bilateral.rs`

### 5.5 Fisher Information Geometry — Unifying Framework

**Reveal that braid's existing math IS Fisher information in disguise.**

The Fisher information matrix: `F_ij = E[∂log p(x|θ)/∂θ_i × ∂log p(x|θ)/∂θ_j]`

What this gives us:

1. **F(S) as scalar summary**: `F(S) ≈ 1 - Tr(F⁻¹)/Tr(F₀⁻¹)` — normalized inverse
   Fisher information
2. **project_delta as finite-difference gradient**: `ΔF(S) ≈ ∇_θ F(S) · δθ`
3. **Brenner's test scoring as expected Fisher gain**: High likelihood ratio = high
   expected Fisher information gain
4. **Cramér-Rao bound**: `Var(θ̂) ≥ F⁻¹` — theoretical lower bound on knowledge needed
5. **Natural gradient**: `F⁻¹ ∇F(S)` — coordinate-invariant gradient following geodesics
   on the statistical manifold

**Connection to existing math**: The Bures metric on density matrices IS the Fisher
information metric for quantum states. Von Neumann entropy IS the dual of Fisher
information. Spectral gap IS the minimum eigenvalue of F. **All existing machinery is
already Fisher information.** This unification reveals, not adds.

**Eigendecomposition of F** reveals:
- **Principal uncertainty directions** (small eigenvalues): where we're most ignorant
- **Optimal observations** (eigenvectors of F⁻¹): what would most reduce uncertainty
- **Effective dimensionality** (eigenvalues above threshold): independent uncertain aspects

**Estimated LOC**: ~300 (uses existing Jacobi/Lanczos in `src/query/graph.rs`)
**Traces to**: `spec/20-coherence.md`, density matrices in `src/trilateral.rs`,
Bures metric, ADR-COHERENCE-003

### 5.6 Profunctor Calculus of Epistemic Instruments

**The deepest formalization: experiments as first-class mathematical objects.**

A profunctor `P : C^op × D → Set` assigns to each pair `(c ∈ C, d ∈ D)` a set of
"probes" — ways to connect knowledge of type C to type D.

| Category | Objects | Example |
|---|---|---|
| **Intent** | Goals, requirements | "security" refines to "auth + authz + encryption" |
| **Spec** | Invariants, ADRs, NEGs | INV-STORE-001 traces to SEED.md §4 |
| **Impl** | Code, tests, configs | store.rs depends on datom.rs |
| **Obs** | Observations, hypotheses | Temporal ordering of observations |
| **Operator** | Cognitive moves | Level-Split → Recode → Exclusion-Test |

The profunctors (epistemic instruments):

| Profunctor | Source → Target | What it does |
|---|---|---|
| **Bilateral** | Spec^op × Impl → Set | Probes for spec-impl alignment |
| **Witness** | Spec^op × Obs → Set | Probes for invariant verification |
| **Harvest** | Obs^op × Spec → Set | Promotes observations to spec |
| **Seed** | Spec^op × Intent → Set | Reconstructs intent from spec |
| **Guidance** | Operator^op × Obs → Set | Recommends cognitive moves |
| **Challenge** | Spec^op × Spec → Set | Self-falsification probes |

Composition via **coend**: `(Q ⊗ P)(a,c) = ∫^b P(a,b) × Q(b,c)` gives compound
instruments. Example: `Bilateral ⊗ Witness` = "verify spec-impl alignment through
observed evidence."

**Yoneda lemma** guarantees every instrument is representable. **Colimit** = CRDT merge.
**Initial algebra** = Y-combinator convergence. **Terminal coalgebra** = greatest fixed
point.

**Status**: ADR-level design document. Informs architecture without requiring full
implementation. Phase C territory.

**Traces to**: Sheaf cohomology already in `src/query/graph.rs`, observation-projection
adjunction (ADR-FOUNDATION-016), Y-combinator structure (`STEERING_MANIFOLD.md` §4)

---

## 6. The Keystone: Autonomous Invariant Discovery

### 6.1 The Gap

Braid has complete loops for:
- **Verifying** invariants (bilateral, witness, challenge, F(S))
- **Calibrating** routing (hypothesis ledger, OBSERVER-4)
- **Organizing** observations (concept clustering, OBSERVER-6)

But invariants themselves — the atomic units of the specification — can only be authored
by a human or agent running `braid spec create`. The spec is **hand-crafted input**, never
**emergent output**. This is a structural asymmetry: the most important type of knowledge
is the one type the system can't discover on its own.

### 6.2 The Insight

**Brenner's Invariant-Extract operator applied at the meta-level**: "Find what survives
transformations." The system transforms its store through thousands of transactions across
dozens of sessions. What survives ALL of them? Those are the invariants. The system
discovers them the same way Brenner discovers biological invariants — by looking at what
NEVER changes across conditions.

This is **inductive inference** formalized as a store operation — the **fourth learning
loop** (after calibration, structure, ontology): **axiomatics** (learn invariants). With
this loop, the system has no remaining structural epistemic blind spots.

### 6.3 The Mechanism

```rust
pub fn discover_invariants(store: &Store, min_support: usize) -> Vec<ProposedInvariant>
```

Scan the store's history for attribute patterns that hold with perfect (or near-perfect)
reliability:

**Pattern Type 1: Temporal co-occurrence**
When attribute A is asserted on an entity, attribute B is always asserted in the same or
next transaction. (N = 47, exceptions = 0)
→ Propose: "INV-AUTO-001: Every entity with `:task/status = :closed` has non-empty
`:task/acceptance-criteria`."

**Pattern Type 2: Absence patterns**
When attribute A has value V, attribute B is NEVER present. (N = 31, exceptions = 0)
→ Propose: "NEG-AUTO-001: Entities with `:spec/element-type = :invariant` never have
`:exploration/maturity = :sketch`."

**Pattern Type 3: Ordering invariants**
Attribute A's value always precedes attribute B's value in lattice ordering.
(N = 23, exceptions = 0)
→ Propose: "INV-AUTO-002: `:task/status` transitions are monotonically non-decreasing
(open ≤ in-progress ≤ closed)."

**Pattern Type 4: Cardinality invariants**
Attribute A always appears exactly K times per entity.
→ Propose as schema constraint.

**Pattern Type 5: Referential invariants**
When entity E has Ref to entity F, F always has attribute A.
→ Propose as foreign-key-like constraint.

### 6.4 Output Structure

```rust
pub struct ProposedInvariant {
    pub pattern: InvariantPattern,
    pub support: usize,          // how many instances observed
    pub exceptions: usize,       // how many violations (should be 0 or near-0)
    pub confidence: f64,         // beta distribution: (s+1)/(s+e+2)
    pub falsification: String,   // auto-generated: "violated if..."
    pub suggested_id: String,    // e.g., "INV-AUTO-001"
    pub traces_to: Vec<String>,  // spec elements that informed the pattern
}

pub enum InvariantPattern {
    CoOccurrence { trigger_attr: Attribute, trigger_value: Option<Value>,
                   required_attr: Attribute, required_value: Option<Value> },
    Absence { when_attr: Attribute, when_value: Value,
              forbidden_attr: Attribute },
    Ordering { attr: Attribute, required_order: Vec<Value> },
    Cardinality { attr: Attribute, exact_count: usize },
    Referential { ref_attr: Attribute, target_required_attr: Attribute },
}
```

### 6.5 Properties

**Each proposed invariant comes with:**
- The **pattern** (what was observed) — machine-readable and human-readable
- The **support** (how many instances) — statistical weight
- The **falsification condition** (auto-generated) — C6 compliance by construction
- The **confidence** (beta distribution posterior) — proper Bayesian credible interval
- **Traceability** — which observations and transactions informed the discovery

**Lifecycle**: Proposed → Endorsed (agent says "yes") → Witnessed (tests added) →
Established (survived challenges). This mirrors the comonadic depth hierarchy:
OPINION → HYPOTHESIS → TESTED → SURVIVED → KNOWLEDGE.

**MDL filter**: Propose invariant only when its description length is shorter than the
description length of the instances it subsumes. Prevents overfitting — no invariant for
patterns observed only twice.

### 6.6 Why This Is The Keystone

**1. It IS Brenner's Invariant-Extract at the meta-level.**
"Find what survives transformations" applied to the system's own behavior. The same
operator that Brenner uses to discover biological laws, the system uses to discover its
own specification laws.

**2. It closes the last remaining open loop.**
Three learning loops (calibration, structure, ontology) + axiomatics = complete epistemic
coverage. No structural blind spots remain.

**3. F(S) becomes self-accelerating.**
Every discovered invariant adds a new dimension to F(S). More dimensions = finer coherence
measurement = better routing = faster convergence = more observations = more discoverable
invariants. Specification GROWS from use, and growth makes the system better at growing.

**4. It connects ALL six proposals.**
- COL (operators): The discovery mechanism ENACTS Invariant-Extract — the most powerful
  operator — continuously
- DIG (discriminative gain): Each proposed invariant is a contrastive prediction — a
  falsification target
- SAC (adversarial critique): Auto-invariants are immediately challengeable — the red-team
  has concrete targets
- Type 10 (control divergence): Positive controls can be auto-generated: "this invariant
  held in all N cases; test case K is the positive control"
- Fisher (information geometry): Invariant discovery = compression of the Fisher information
  matrix (same information, fewer parameters)
- Profunctors: The discovery function IS a profunctor from Obs^op × Spec → Set — promoting
  observations to spec elements

**5. It makes F(S) trustworthy.**
Auto-invariants discovered from data have empirical backing that hand-authored invariants
lack. An F(S) computed from auto-discovered invariants with 100% support rate is more
credible than one from aspirational spec elements.

**6. It's the deepest expression of C7 (self-bootstrap).**
C7 says the system's first data is its own spec. With invariant discovery, the system
doesn't just CHECK its spec against itself — it GENERATES spec from itself. The
specification is alive.

**7. It IS inductive compression.**
Before discovery: N separate datoms. After discovery: one invariant + N instances
EXPLAINED by the invariant. Kolmogorov complexity drops. Information preserved.
This is LEARNING in the most fundamental sense — finding shorter descriptions.

### 6.7 The Deep Math: Invariant Discovery as Kolmogorov Compression

The connection to the Fisher information framework is direct: a discovered invariant
**compresses** the store's parameter space.

Before invariant discovery, each pattern instance is an independent parameter. The Fisher
information matrix has N diagonal entries, each providing independent information.

After discovery, the N instances are explained by ONE invariant with ONE parameter
(its truth value). The Fisher information per datom INCREASES — the same total information
is concentrated into fewer parameters. The effective dimensionality (number of significant
eigenvalues of F) drops.

This is why invariant discovery accelerates convergence: it reduces the dimensionality of
the coherence optimization problem. Fewer parameters to calibrate = faster convergence
to the fixed point.

**Rate of discovery** is bounded by the **minimum description length principle**:

```
Propose invariant I iff: L(I) + L(data|I) < L(data)
```

where L(·) is description length. The invariant must compress — its description plus the
residual must be shorter than describing the raw data.

**Connection to Brenner's objective function**: Invariant discovery maximizes
(information compression × downstream impact) / (computation cost) — the same form as
Brenner's "evidence per week."

---

## 7. Critical Assessment

### 7.1 What Works (Immediate Viability)

| Proposal | Kernel changes | LOC estimate | C8 compliant | Risk |
|---|---|---|---|---|
| 5.1 COL (operators) | None — policy datoms | ~300 | Yes (policy layer) | Low |
| 5.2 DIG (scoring) | Augment `compute_routing` | ~100 | Yes (scoring math) | Low |
| 5.3 SAC (adversarial harvest) | Extend harvest pipeline | ~200 | Yes (pipeline phase) | Low |
| 5.4 Type 10 (control) | Schema attrs + bilateral | ~100 | Yes (divergence type) | Low |
| 6 Keystone (auto-invariants) | New pure function | ~400 | Yes (query, no policy) | Medium |
| 5.5 Fisher (geometry) | Extend MaterializedViews | ~300 | Yes (math layer) | Medium |
| 5.6 Profunctors (theory) | ADR only (Phase C) | ~50 (doc) | N/A | None |

### 7.2 What Needs Care

**Complexity budget**: F(S) = 0.62. Adding features before consolidation violates "reduce
scope before reducing quality." Counter-argument: these features IMPROVE consolidation —
they make existing features more effective, not more numerous.

**Practicality of Fisher information**: Full Fisher matrix is O(n²) in parameters.
Tractable for small stores; needs Lanczos/random projection for large ones. The existing
Jacobi eigendecomposition in `src/query/graph.rs` handles matrices up to ~1000×1000.
Beyond that, use the approximate spectral methods already planned (SPECTRAL-1/2).

**Auto-invariant false positives**: Patterns that hold by accident (small N) or by
construction (schema constraints that trivially enforce them). The MDL filter and
minimum-support threshold mitigate this. Set min_support = 20 initially; calibrate from
false positive rate.

**Testing surface**: Every new subsystem needs tests. Current: ~1665 tests. Proposals
add ~200 tests total. Manageable if phased.

### 7.3 Validation via Substrate Test (C8)

"Would this make sense for a React project? A research lab? A compliance team?"

- **COL**: Yes — cognitive operators are domain-neutral. A React developer can Level-Split
  (separate rendering from state management). A compliance team can Exclusion-Test
  (what violations are impossible given current controls?).
- **DIG**: Yes — discriminative scoring applies to any boundary system.
- **SAC**: Yes — red-teaming applies to any knowledge pipeline.
- **Type 10**: Yes — positive controls are universal verification hygiene.
- **Auto-invariants**: Yes — pattern discovery works on any datom store.
- **Fisher**: Yes — information geometry is domain-neutral.
- **Profunctors**: Yes — category theory is the most domain-neutral framework that exists.

All proposals pass C8.

---

## 8. Implementation Roadmap

### Phase A: Close Current Loops (F(S) 0.62 → 0.75)

**Priority order by impact/effort ratio:**

1. **Keystone: Autonomous Invariant Discovery** (~400 LOC)
   - New file: `crates/braid-kernel/src/invariant_discovery.rs`
   - Wire into: `braid status` ("Proposed Invariants" section),
     `braid bilateral` (auto-invariant coverage), harvest pipeline
   - Tests: empty store (no proposals), minimal store (no proposals below threshold),
     task-closure patterns (co-occurrence proposal), status transitions (ordering proposal),
     schema constraints (referential proposal)
   - Acceptance: ≥3 genuine invariants discovered from braid's own 176K-datom store

2. **COL: Cognitive Operator Lattice** (~300 LOC)
   - Schema: `:operator/*` namespace in policy manifest
   - Implementation: `query_operators()` in kernel, guidance footer integration in CLI
   - Tests: operator lookup by divergence type, composition chain traversal,
     unknown divergence type returns empty
   - Acceptance: guidance footer includes operator recommendation for each divergence

3. **DIG: Discriminative Information Gain** (~100 LOC)
   - Augment `compute_routing()` in `src/routing.rs`
   - Add `uncertainty(boundary)` from hypothesis ledger calibration data
   - Add `V_downstream()` from dependency graph blocked-task count
   - Tests: uncertain boundary beats certain boundary at equal ΔF(S),
     high-V_downstream beats low at equal uncertainty
   - Acceptance: task ordering changes measurably vs current R(t) on braid's own store

4. **SAC: Structural Adversarial Critique** (~200 LOC)
   - New harvest pipeline phase between PROPOSE and REVIEW
   - Three sub-checks: Scale-Check, Exception-Quarantine, Theory-Kill
   - Tests: duplicate observation flagged, conflicting observation quarantined,
     genuine observation passes
   - Acceptance: red-team flags 10-20% of harvest candidates on real sessions

### Phase B: Deepen Formalism (F(S) 0.75 → 0.85)

5. **Type 10: Control Divergence** (~100 LOC)
   - Schema: `:witness/positive-control` Ref attribute
   - Logic: bilateral checks positive control before trusting witness result
   - Tests: broken positive control marks witness IMPOTENT, working positive control
     allows VERIFIED
   - Acceptance: at least 3 witness chains include positive controls

6. **Fisher Information Unification** (~300 LOC)
   - Natural gradient in routing: `F⁻¹ ∇F(S)` replacing raw gradient
   - Eigendecomposition of F reveals principal uncertainty directions
   - Uses existing Jacobi/Lanczos in `src/query/graph.rs`
   - Tests: natural gradient produces different (better) ordering than raw gradient,
     principal uncertainty directions match known weak boundaries
   - Acceptance: routing calibration error decreases ≥10%

7. **Operator Calibration** (~100 LOC)
   - Track operator recommendations in hypothesis ledger
   - `operator_effectiveness(store, operator_symbol) → f64`
   - Tests: effective operators recommended more, ineffective less
   - Acceptance: operator-divergence mapping adapts from initial defaults

### Phase C: Full Synthesis (F(S) 0.85 → 0.95)

8. **Profunctor Architecture** (~50 LOC documentation)
   - ADR-FOUNDATION-035: Profunctor Calculus of Epistemic Instruments
   - Selective implementation where formalism reveals optimization opportunities
   - No kernel changes unless formalism suggests concrete improvements

9. **Anomaly Register** (~200 LOC)
   - `:anomaly/*` namespace, quarantine lifecycle, accumulation monitoring
   - "Occam's broom height" metric in `braid status`
   - Tests: quarantined anomaly does not affect F(S), accumulated anomalies trigger review

---

## Appendix A: Convergence Proof for Invariant Discovery

**Claim**: Autonomous invariant discovery is monotonically convergent — each discovered
invariant either increases F(S) or has no effect.

**Proof sketch**:
1. A discovered invariant I has support N ≥ min_support and 0 exceptions.
2. I is proposed at depth 0 (OPINION), contributing 0.0 to F(S) via depth-weighting.
3. If endorsed → depth 1, contributing 0.15 × coverage(I) to F(S).
4. coverage(I) ≥ 0 by construction (I was observed to hold in N cases).
5. Therefore F(S) with I ≥ F(S) without I. □

**Falsification of convergence claim**: If a proposed invariant is WRONG (an exception
exists but was missed), endorsing it inflates F(S) incorrectly. This is detected by
the challenge system (comonadic duplicate) and corrected by retracting the invariant.
The SAW-TOOTH invariant ensures this correction produces a healthy oscillation, not a
permanent error.

## Appendix B: Connection to Prior Sessions

| Session | Contribution to This Synthesis |
|---|---|
| 033 (`90df6190`) | Convergence thesis, three learning loops, bedrock vision |
| 034 (`c01bb082`) | Complete formalism, observer formalism, hypothetico-deductive loop |
| 035 | Comonadic depth, challenge system, external readiness |
| 038 (`0128002a`) | Y-combinator, seven-level reflexive hierarchy, Markov blanket |
| 042 (`56ece7bd`) | Steering manifold, Piagetian foundation, concept crystallization |
| 047 | Performance, daemon, external validation (6.75/10 on rr-cli) |
| 048 | C9 second-order epistemic closure, observe subcommands |
| 049 | Integration tests (78 tests, 3757 LOC, 100% catalog) |
| **050 (this)** | Brenner cross-examination, operator algebra, autonomous invariant discovery |

## Appendix C: Spec Elements to Crystallize

From this document, the following spec elements should be created:

1. **ADR-FOUNDATION-035**: Profunctor Calculus of Epistemic Instruments
2. **ADR-FOUNDATION-036**: Autonomous Invariant Discovery (Inductive Compression)
3. **ADR-FOUNDATION-037**: Cognitive Operator Lattice (Brenner Integration)
4. **INV-FOUNDATION-016**: Discovered invariants have auto-generated falsification (C6)
5. **INV-FOUNDATION-017**: Invariant discovery is monotonically convergent
6. **INV-FOUNDATION-018**: MDL filter prevents overfitting (min description length)
7. **NEG-FOUNDATION-006**: Auto-invariants must not bypass depth hierarchy
8. **INV-DIVERGENCE-010**: Type 10 Control Divergence positive control requirement
9. **INV-CALIBRATION-001**: Operator effectiveness learned from hypothesis outcomes
10. **INV-HARVEST-010**: SAC red-team phase flags 10-20% of candidates

## Appendix D: Brenner Operator Quick Reference

For implementation of Proposal 5.1 (COL), the complete operator library:

```yaml
operators:
  - symbol: level-split
    trigger: "Confusing two levels of abstraction"
    action: "Separate program from interpreter, message from machine"
    failure_mode: "Splitting too finely (losing emergent properties)"
    domain: universal

  - symbol: recode
    trigger: "Current representation obscures pattern"
    action: "Change representation to reduce dimensionality"
    failure_mode: "Lossy recoding that discards signal"
    domain: universal

  - symbol: invariant-extract
    trigger: "Multiple observations of same phenomenon"
    action: "Find what survives across all transformations"
    failure_mode: "Confusing correlation with invariance"
    domain: universal

  - symbol: exclusion-test
    trigger: "Multiple competing hypotheses"
    action: "Derive forbidden patterns; design lethal experiments"
    failure_mode: "Testing only easy-to-falsify hypotheses"
    domain: universal

  - symbol: materialize
    trigger: "Theory lacks concrete predictions"
    action: "Theory to 'what would I see?'"
    failure_mode: "Materializing at wrong scale"
    domain: universal

  - symbol: object-transpose
    trigger: "Test is expensive or slow in current substrate"
    action: "Change substrate until test is easy"
    failure_mode: "Transposition changes essential property"
    domain: scientific

  - symbol: quickie
    trigger: "Large investment planned without pilot data"
    action: "Cheap pilot to de-risk"
    failure_mode: "Quickie too small to be informative"
    domain: universal

  - symbol: hal
    trigger: "Theorizing without observation"
    action: "Have A Look (direct observation)"
    failure_mode: "Seeing what you expect instead of what's there"
    domain: universal

  - symbol: chastity-check
    trigger: "Negative result obtained"
    action: "Distinguish won't (chastity) from can't (impotence)"
    failure_mode: "Assuming impotence without positive control"
    domain: universal

  - symbol: scale-check
    trigger: "Quantitative claim without calculation"
    action: "Calculate actual numbers; stay physically imprisoned"
    failure_mode: "Using wrong units or order of magnitude"
    domain: universal

  - symbol: exception-quarantine
    trigger: "Observation contradicts current framework"
    action: "Isolate anomaly; track Occam's broom carpet height"
    failure_mode: "Quarantine becomes permanent ignore"
    domain: universal

  - symbol: theory-kill
    trigger: "Theory survives but is ugly/overcomplicated"
    action: "Discard; pursue elegance"
    failure_mode: "Killing theory that was actually correct"
    domain: universal

  - symbol: paradox-hunt
    trigger: "Framework feels complete but progress stalled"
    action: "Find contradictions to reveal missing rules"
    failure_mode: "Manufacturing paradoxes that aren't real"
    domain: universal

  - symbol: cross-domain
    trigger: "Stuck within current field's paradigms"
    action: "Import unrelated-field patterns"
    failure_mode: "False analogy (Brenner guardrail #12)"
    domain: universal

  - symbol: dephase
    trigger: "Following fashionable approach without questioning"
    action: "Move out of phase with fashion; ask unfashionable questions"
    failure_mode: "Contrarianism without substance"
    domain: universal

  - symbol: amplify
    trigger: "Signal too weak to detect"
    action: "Use amplification (biological, computational, statistical)"
    failure_mode: "Amplifying noise along with signal"
    domain: scientific

  - symbol: diy
    trigger: "Waiting for tool/resource that doesn't exist"
    action: "Build what you need; don't wait"
    failure_mode: "Building when buying would suffice"
    domain: engineering
```

Default composition chains:
```yaml
chains:
  diagnostic: [level-split, recode, invariant-extract, exclusion-test]
  theory-to-test: [recode, materialize, quickie]
  hygiene: [scale-check, exception-quarantine, theory-kill]
  optimization: [object-transpose, amplify, diy]
  meta-cognitive: [dephase, cross-domain, paradox-hunt]
```

Divergence → Operator mapping:
```yaml
divergence_mapping:
  epistemic: {primary: hal, chain: [hal, invariant-extract, recode]}
  structural: {primary: level-split, chain: [level-split, recode, exclusion-test]}
  consequential: {primary: materialize, chain: [materialize, scale-check, quickie]}
  aleatory: {primary: cross-domain, chain: [cross-domain, paradox-hunt, level-split]}
  logical: {primary: paradox-hunt, chain: [paradox-hunt, exclusion-test, theory-kill]}
  axiological: {primary: dephase, chain: [dephase, level-split, exception-quarantine]}
  temporal: {primary: quickie, chain: [quickie, hal, invariant-extract]}
  procedural: {primary: diy, chain: [diy, object-transpose, scale-check]}
  reflexive: {primary: level-split, chain: [level-split, scale-check, invariant-extract]}
  control: {primary: chastity-check, chain: [chastity-check, scale-check, hal]}
```
