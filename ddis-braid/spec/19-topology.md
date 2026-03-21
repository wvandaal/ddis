> **DEPRECATED**: This file is bootstrap scaffolding. The canonical source of truth is the braid datom store. Use `braid spec show` and `braid query` to access spec elements. See ADR-STORE-019.

---

> **Namespace**: TOPOLOGY | **Wave**: 4 (Integration) | **Stage**: 3
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §19. TOPOLOGY — Coordination Topology Framework

### §19.0 Overview

The coordination topology framework formalizes how multiple agents (human and AI)
organize their communication, work distribution, scaling, and convergence — all
mediated through the datom store. A topology T = (G, Φ, Σ, Π) is a 4-tuple
where G is the agent graph, Φ is the merge policy, Σ is the scaling policy,
and Π is the assignment policy. All four components are datoms, queryable via
Datalog, and evolvable through the bilateral learning loop.

**Core thesis**: In a CRDT-based datom store, coordination topology emerges from
data, is computed by the same query engine that validates specifications, learns
through harvest/seed, and converges through the bilateral loop. Because the
specification is queryable data in the same store, the optimal topology can be
*compiled* from spec structure — not merely discovered from execution.

**Traces to**: SEED.md §4 (CRDT merge), §5 (harvest/seed lifecycle), §6
(reconciliation taxonomy), §7 (self-improvement loop), §10 (Stage 3)
**Source**: `docs/history/exploration/execution-topologies/` (14 documents, ~5,900 lines)

**Extends**: MERGE (§7), SYNC (§8), SIGNAL (§9), GUIDANCE (§12), BILATERAL (§10),
TRILATERAL (§18 — quadrilateral extension)

---

### §19.1 Level 0: Formal Definition

#### T = (G, Φ, Σ, Π)

```
A coordination topology T = (G, Φ, Σ, Π) where:

  G = (V, E)                Agent graph
    V ⊂ Entity              Vertices = agent entities in the datom store
    E ⊂ V × V × ChannelType Edges = communication channels

  Φ : G × CouplingMatrix → MergeFrequency
    Merge policy: maps coupling between agent pairs to merge frequency

  Σ : SystemState → ScalingAction
    Scaling policy: decides when to add/remove agents and partition clusters

  Π : Agents × Tasks → AssignmentScore
    Assignment policy: maps agent-task pairs to assignment scores
```

All components are datoms in the store (C3: schema-as-data). Topology
decisions are append-only facts (C1). Topology history is fully queryable.

#### Named Topology Patterns

Six canonical patterns, expressible as Datalog predicates:

| Pattern | Channels | Diameter | Use When |
|---------|----------|----------|----------|
| Mesh | O(n²) | 1 | n ≤ 5, high coupling |
| Star(hub) | O(n) | 2 | n ≤ 12, hub agent |
| Pipeline | O(n) | n | Sequential dependencies |
| Ring | O(n) | n/2 | Pipeline with feedback |
| Hierarchy | O(n) | 2h | n > 12, tree authority |
| Hybrid | Varies | Varies | Multiple coupling clusters |

#### LIVE Topology Projection (LIVE_T)

Extending the trilateral model (§18) to a quadrilateral:

```
Given:
  TOPO_ATTRS ⊂ Keyword    — attribute namespace for topology facts
  TOPO_ATTRS ∩ INTENT_ATTRS = ∅
  TOPO_ATTRS ∩ SPEC_ATTRS = ∅
  TOPO_ATTRS ∩ IMPL_ATTRS = ∅

Fourth LIVE projection:
  LIVE_T(S) = project(S, {d ∈ S | d.a ∈ TOPO_ATTRS})

Quadrilateral divergence:
  Φ_total = Φ_IS + Φ_SP + Φ_PI + Φ_TI + Φ_TP
  where Φ_TI = |LIVE_T(S) ⊗ LIVE_I(S)| = topology-intent boundary
        Φ_TP = |LIVE_T(S) ⊗ LIVE_P(S)| = topology-implementation boundary
```

---

### §19.2 Invariants

#### INV-TOPOLOGY-001: Topology as Store Projection

**Traces to**: SEED.md §4 (Design Commitment #2), INV-STORE-012 (LIVE index)
**Type**: Structural invariant
**Statement**: The coordination topology T is a LIVE materialized view over the
datom store. T = LIVE_T(S) is computed deterministically from store state S.
Two stores with the same datom sets produce the same topology.
**Falsification**: Two stores with identical datom sets produce different topology
projections.
**Verification**: V:PROP — construct identical stores via different transaction
orderings, assert LIVE_T produces identical topology. V:KANI — determinism proof
for small n.

---

#### INV-TOPOLOGY-002: Topology Independence of Ordering

**Traces to**: spec/01-store.md L1–L5 (CRDT laws), INV-MERGE-001
**Type**: Safety invariant
**Statement**: For any two topologies T₁, T₂ over the same agent set, if all
datoms eventually propagate (liveness), then the LIVE index converges to the same
state regardless of which topology was used during propagation.
**Proof sketch**: LIVE index is computed from datom set S via per-attribute
resolution modes. Resolution modes are deterministic functions of S. Since MERGE
is set union (L1–L3), the final S is the same regardless of merge order or
channel topology. Therefore LIVE(S) is the same. QED
**Falsification**: Two different merge topologies over the same datom set produce
different LIVE indices after convergence.
**Verification**: V:PROP — simulate n agents under mesh vs star vs pipeline
topology, same datom set, assert LIVE indices converge identically. V:MODEL —
model check with StateRight for n ≤ 5.

---

#### INV-TOPOLOGY-003: Topology Decision Immutability

**Traces to**: C1 (append-only store)
**Type**: Structural invariant
**Statement**: Every topology decision (transition proposal, enactment, rollback,
scaling action, coupling measurement, fitness measurement) is an immutable datom.
Topology history is fully queryable and never lost.
**Falsification**: A topology decision entity is deleted or mutated in place.
**Verification**: V:TYPE — topology decision types only expose assert operations.
V:PROP — after any sequence of topology operations, all prior decision datoms
remain in the store.

---

#### INV-TOPOLOGY-004: Composite Coupling Signal

**Traces to**: SEED.md §7 (self-improvement loop), INS-005 (task-level routing)
**Type**: Measurement invariant
**Statement**: Task coupling is a composite signal of five mechanisms (file paths,
invariants, schema, causal dependencies, historical patterns) with learnable
weights stored as datoms. The composite preserves semilattice structure: coupling
scores only grow through observation, explicit reduction requires retraction.
**Formal**:
```
coupling(T₁, T₂) = w_f·file(T₁,T₂) + w_i·inv(T₁,T₂) + w_s·schema(T₁,T₂)
                  + w_c·causal(T₁,T₂) + w_h·historical(T₁,T₂)

w_f + w_i + w_s + w_c + w_h = 1.0, all w ≥ 0
```
**Stage introduction**: Stage 0: [1,0,0,0,0]. Stage 0b: [0.50,0.35,0,0,0.15].
Stage 1: [0.40,0.25,0.10,0,0.25]. Stage 3: [0.25,0.20,0.10,0.15,0.30].
**Falsification**: The composite coupling function violates commutativity
(merge-order dependence) or allows coupling to decrease without explicit
retraction.
**Verification**: V:PROP — compute coupling under different merge orderings,
assert identical results. Assert monotonicity: adding observations never
decreases coupling.

---

#### INV-TOPOLOGY-005: Coupling-to-Topology Determinism

**Traces to**: ADR-TOPOLOGY-001
**Type**: Functional invariant
**Statement**: Given a coupling matrix C and agent count n, the topology
selection function is deterministic: same inputs always produce same topology.
**Falsification**: Two invocations with identical C and n produce different
topology structures.
**Verification**: V:PROP — generate random coupling matrices, assert deterministic
topology selection.

---

#### INV-TOPOLOGY-006: Monotonic Transition Safety (CALM Tier M)

**Traces to**: spec/01-store.md L4 (monotonicity), CALM theorem, INV-SYNC-001
**Type**: Safety invariant
**Statement**: A Tier M (monotonic) transition never decreases the channel count.
For any Tier M transition T_old → T_new: channels(T_new) ⊇ channels(T_old).
**Falsification**: |channels(T_new)| < |channels(T_old)| and the transition was
classified as Tier M.
**Verification**: V:TYPE — `MonotonicTransition` type constructor only accepts
channel additions (AddChannel, IncreaseFrequency, AddAgent). V:PROP — generate
random Tier M transitions, assert channel set only grows.

---

#### INV-TOPOLOGY-007: Grace Period Completeness (CALM Tier NM)

**Traces to**: spec/08-sync.md INV-SYNC-001, C1 (append-only, no data loss)
**Type**: Safety invariant
**Statement**: No in-flight merge data is lost during a Tier NM transition.
For any datom d in-transit at grace period start, d is received by the target
agent before grace period end.
**Falsification**: A datom was in-transit on a channel deactivated before the
datom was received.
**Verification**: V:PROP — inject datom on channel C immediately before grace
period start, assert datom appears in target agent's store before grace period
end. Parameterize over channel latency, datom size, grace period duration.

---

#### INV-TOPOLOGY-008: Connectivity Preservation

**Traces to**: Graph theory (connected graph), coordination liveness
**Type**: Safety invariant
**Statement**: The agent graph is connected after every enacted topology
transition. For all pairs of active agents (α, β), there exists a path through
active channels in T_new.
**Falsification**: After an enacted transition, there exist active agents α, β
with no path between them.
**Verification**: V:PROP — after every simulated transition, run BFS/DFS
connectivity check. Assert all active agents are in the same connected component.
V:MODEL — model check with StateRight for n ≤ 5.

---

#### INV-TOPOLOGY-009: Rollback Safety

**Traces to**: spec/01-store.md L4, INV-TOPOLOGY-006
**Type**: Safety invariant
**Statement**: A rollback is always a monotonic transition (re-adding removed
channels). channels(rollback_result) ⊇ channels(T_old).
**Falsification**: A rollback removes channels that existed in T_old.
**Verification**: V:TYPE — rollback function produces topology where channels are
superset of T_old. V:PROP — generate random Tier NM transitions, simulate
rollback, assert channel set includes all of T_old's channels.

---

#### INV-TOPOLOGY-010: Cold-Start Monotonic Relaxation

**Traces to**: Minimax strategy, 06-cold-start.md theorem
**Type**: Progress invariant
**Statement**: The coordination intensity never increases without evidence of
under-coordination. If conflict_rate = 0 and quality ≥ quality_prev − ε:
intensity(T_next) ≤ intensity(T_current).
**Formal**: Let intensity(T) = Σ merge_frequency_rank(channel) over all channels.
**Falsification**: Merge frequency increases or topology tightens when
conflict_rate = 0 and quality is stable or improving.
**Verification**: V:PROP — track coordination_intensity across simulated sessions,
assert monotonically non-increasing except after conflict events (D₂ > 0) or
quality drops.

---

#### INV-TOPOLOGY-011: Cold-Start Safety Bound

**Traces to**: INV-TOPOLOGY-010, minimax dominance theorem
**Type**: Safety invariant
**Statement**: The cold-start topology provides coordination intensity ≥ the
coupling-optimal topology for the actual task coupling.
intensity(COLD_START(C)) ≥ intensity(OPTIMAL(C)) for any coupling matrix C.
**Falsification**: There exists a coupling matrix C where cold-start intensity <
optimal intensity.
**Verification**: V:PROP — generate random coupling matrices, compute COLD_START
and OPTIMAL (brute-force for small n), assert COLD_START ≥ OPTIMAL.

---

#### INV-TOPOLOGY-012: F(T) Monotonic Improvement

**Traces to**: spec/10-bilateral.md INV-BILATERAL-001 (convergence), SEED.md §7
**Type**: Progress invariant
**Statement**: When the bilateral loop recommends a topology change and the change
is enacted, F(T') ≥ F(T) − ε (noise margin ε = 0.02). Three consecutive
decreases beyond ε trigger Signal::TopologyDrift.
**Falsification**: An enacted bilateral-recommended change decreases F(T) by more
than ε across the measurement window.
**Verification**: V:PROP — track F(T) before and after each enacted change, assert
improvement within margin. Statistical test at 95% confidence.

---

#### INV-TOPOLOGY-013: F(T)/F(S) Independence

**Traces to**: spec/10-bilateral.md, orthogonality of spec quality and coordination
**Type**: Structural invariant
**Statement**: Topology fitness F(T) and specification fitness F(S) are
independent. Changes to topology do not affect specification convergence, and
vice versa. |corr(F(T), F(S))| < 0.3 (weak or no correlation).
**Falsification**: A topology change causes F(S) to decrease by > 0.05, or a spec
change causes F(T) to decrease by > 0.05.
**Verification**: V:PROP — measure both across topology and spec changes, compute
Pearson correlation, assert |r| < 0.3.

---

#### INV-TOPOLOGY-014: F_total Quadrilateral Convergence

**Traces to**: spec/18-trilateral.md INV-TRILATERAL-001, ADR-TRILATERAL-004
**Type**: Progress invariant
**Statement**: Total system fitness F_total = λ·F(S) + (1−λ)·F(T) converges
monotonically toward 1.0 under the quadrilateral bilateral loop. λ varies by
phase: 0.80 (spec production), 0.50 (multi-agent impl), 0.20 (coordination).
**Formal**: The quadrilateral adds topology vertex and two boundaries (T↔I, T↔P)
to the trilateral model. F_total = 1.0 is the fixpoint where all four vertices
are coherent.
**Falsification**: F_total diverges or oscillates without convergence under
bilateral loop recommendations.
**Verification**: V:PROP — simulate multi-session scenarios, track F_total trend.

---

### §19.3 Architectural Decision Records

#### ADR-TOPOLOGY-001: Composite Coupling with Learnable Weights

**Traces to**: SEED.md §7, INS-005 (task-level routing outperforms fixed topology)
**Problem**: How should task coupling be measured to drive topology selection?
**Options**:
1. Single signal (file paths only) — always available but misses semantic coupling
2. Fixed multi-signal — more complete but hardcoded weights may be wrong
3. Learnable multi-signal — adapts but cold-start requires defaults

**Decision**: Option 3 — five mechanisms with weights stored as datoms, updated
through bilateral feedback. File-path-dominated cold-start defaults.

**Rationale**: INS-005 establishes task-level routing outperforms fixed topology.
By analogy, task-level coupling measurement should outperform fixed signals. The
bilateral loop provides the update mechanism. Cold-start defaults (file-path at
w=1.0) provide reasonable initial behavior.

**Consequences**: Five coupling mechanisms defined. Weights are datoms. Staged
introduction across Stages 0–3. Composite preserves semilattice structure
(composes with CRDT merge).

---

#### ADR-TOPOLOGY-002: CALM-Stratified Topology Transitions

**Traces to**: spec/03-query.md (CALM compliance), spec/08-sync.md
**Problem**: How should topology transitions be handled safely?
**Options**:
1. All transitions require barrier — maximum safety, unnecessary overhead for
   monotonic changes
2. No barriers — maximum speed, risks data loss on non-monotonic changes
3. CALM-stratified — monotonic = no barrier, non-monotonic = barrier

**Decision**: Option 3 — CALM stratification.

**Rationale**: CALM is already the foundation of Braid's query classification.
Applying it to topology transitions is a natural extension. The classification
is mechanical (compare channel sets before/after).

**Consequences**: Two-tier protocol: Tier M (no barrier) and Tier NM (barrier +
grace period + connectivity verification). Rollback is always Tier M (re-adding
channels). Automatic tier classification from algebraic properties.

---

#### ADR-TOPOLOGY-003: Scaling Authority A(d) = R(1−C)T

**Traces to**: SEED.md §6 (reconciliation), spec/04-resolution.md (three-tier routing)
**Problem**: Who decides scaling actions, and how is authority determined?
**Options**:
1. Always human — safe but bottleneck
2. Always autonomous — fast but risky
3. Authority function with earned trust — starts conservative, earns autonomy

**Decision**: Option 3 — A(d) = R(1−C)T where R = reversibility, C = commitment
weight, T = trust score earned from outcomes.

**Three-tier mapping**:
- A > 0.5: Act autonomously (Tier 1)
- A ∈ [0.2, 0.5]: Recommend and act unless vetoed (Tier 2)
- A < 0.2: Recommend only, human approval required (Tier 3)

**Rationale**: Static "always human" or "always autonomous" is a process
obligation that doesn't adapt. The authority function is a structural guarantee:
the system CAN'T act on high-commitment decisions until it earns trust.

**Consequences**: T_initial = 0.3 (conservative). Harmful outcomes decrease trust
4× faster than good outcomes increase it. Some decisions (remove active agent,
scale down by half) may never reach autonomous tier — by design.

---

#### ADR-TOPOLOGY-004: Topology as Compilation

**Traces to**: C7 (self-bootstrap), SEED.md §4 (spec as queryable data)
**Problem**: How should optimal topology be determined?
**Options**:
1. Reactive only — execute, observe, adjust. Standard in all existing frameworks.
2. Compile from spec structure + reactive fallback — derive coupling from spec
   dependency graph (AOT), fall back to reactive for uncompilable work (JIT),
   refine via bilateral feedback (PGO).

**Decision**: Option 2 — Hybrid AOT+JIT with PGO.

**Rationale**: Braid uniquely has specifications as queryable data in the same
store. The spec dependency graph encodes coupling structure *before* any agent
begins working. This enables compile-time topology optimization that no other
framework can achieve. The compilation pipeline:
```
Spec → Front-end (dependency graph → coupling IR) →
Middle-end (partition, CALM classify) →
Back-end (emit T=(G,Φ,Σ,Π)) →
Execute → Profile (harvest outcomes) → PGO (refine compiler)
```

**Consequences**: Eliminates cold-start penalty for spec-bearing projects.
Compiled topology weakly dominates cold-start (INV-TOPOLOGY-015). PGO prediction
error decreases monotonically after warm-up. Unifies task decomposition with
topology selection. This is the payoff of C7 that SEED.md did not anticipate.

---

#### ADR-TOPOLOGY-005: Two-Tier Substrate Model

**Traces to**: SEED.md §5 (harvest/seed), ADR-HARVEST-001
**Problem**: Should coordination messages be datoms?
**Options**:
1. All coordination as datoms — full history but massive store bloat (C1 means
   they can never be cleaned up)
2. No coordination as datoms — loses learning capability
3. Two-tier: ephemeral substrate + harvested datoms

**Decision**: Option 3 — messages live in external systems (Agent Mail, session
JSONL). The harvest functor extracts significant decisions, outcomes, and
patterns into datoms.

**Rationale**: The harvest/seed lifecycle already solves this for human/AI
conversations. Same architecture applies to AI/AI coordination.

**Consequences**: Agent Mail remains as real-time substrate. Harvest pipeline
extended to ingest coordination logs. Braid replaces intelligence layer (bv graph
analysis, ntm assignment logic) with Datalog queries; ephemeral communication
and enactment layers remain separate.

---

#### ADR-TOPOLOGY-006: Human/AI Coordination Isomorphism

**Traces to**: spec/01-store.md ADR-STORE-008 (provenance lattice)
**Problem**: Should human/AI and AI/AI coordination use different mechanisms?
**Options**:
1. Separate mechanisms — can optimize each but doubles design surface
2. Unified mechanism — one algebra for both, differences handled by provenance

**Decision**: Option 2 — unified. Both are instances of bounded-context
frontier-bearing agents exchanging assertions through a shared medium.

**Rationale**: Differences (bandwidth, latency, authority) are quantitative, not
qualitative. The provenance lattice (Observed > Derived > Inferred > Hypothesized)
and three-tier conflict routing handle authority differences. No new mechanism
needed.

---

#### ADR-TOPOLOGY-007: Seven-Dimensional F(T) with Bilateral Loop

**Traces to**: spec/10-bilateral.md (F(S)), spec/13-budget.md (rate-distortion)
**Problem**: How should topology quality be measured?
**Options**:
1. Single metric (throughput) — simple but misses conflicts, balance, knowledge loss
2. Seven-dimensional with weighted composition — comprehensive, enables diagnostics
3. No formal metric — flexible but not mechanically evaluable

**Decision**: Option 2 — seven dimensions.

**Formula**:
```
F(T) = w₁·D₁ + w₂·(1-D₂) + w₃·(1-D₃) + w₄·(1-D₄) + w₅·D₅ + w₆·(1-D₆) + w₇·(1-D₇)
```

| # | Dimension | Measures | Optimal | Default Weight |
|---|-----------|----------|---------|---------------|
| 1 | Throughput | Tasks/time | 1 | 0.25 |
| 2 | Conflict rate | Merge conflict fraction | 0 | 0.20 |
| 3 | Staleness | Mean stale fact age | 0 | 0.10 |
| 4 | Merge overhead | Time merging / total time | 0 | 0.15 |
| 5 | Balance | Agent utilization uniformity | 1 | 0.10 |
| 6 | Blocking time | Time blocked / total time | 0 | 0.10 |
| 7 | Knowledge loss | Epistemic gap at session end | 0 | 0.10 |

**Rationale**: F(S) uses multi-dimensional composition. Extending to topology
provides consistency and enables the bilateral convergence loop. The diagnostic
mapping (dimension → corrective action) turns topology optimization from
guesswork into a systematic process.

**Consequences**: Each dimension queryable via Datalog. Weights are datoms. The
D₂ vs D₄ tension (conflict rate vs merge overhead) is the fundamental tradeoff;
optimal point biased toward more merging (conflict cost >> merge cost).

---

### §19.4 Negative Cases

#### NEG-TOPOLOGY-001: No Topology Without Store Grounding

**Traces to**: INV-TOPOLOGY-001
**Type**: Negative case
**Statement**: The topology must be a store projection, never an external
configuration imposed on the system. If T is not derivable from store state S,
then T is invalid.
**Violation condition**: A topology exists that cannot be reconstructed from the
datom store alone (e.g., topology state stored in external config file, in-memory
only, or agent-local state not replicated to store).
**Recovery**: Retransact the topology decisions into the store.

---

#### NEG-TOPOLOGY-002: No Autonomous High-Commitment Scaling

**Traces to**: ADR-TOPOLOGY-003
**Type**: Negative case
**Statement**: The system must not autonomously execute scaling decisions with
commitment weight C > 0.5, regardless of trust score T. Such decisions always
require human approval (A(d) < 0.2 for high C, even at T_max).
**Formal**: For any decision d where C(d) > 0.5: A(d) = R·(1−C)·T ≤ 0.5·R·T.
Even at R=1.0, T=1.0: A(d) ≤ 0.5 → never reaches autonomous tier threshold
of 0.5 (equality case is boundary, not interior).
**Violation condition**: The system removes an active agent, scales down by half,
or performs other high-commitment action without human approval.

---

#### NEG-TOPOLOGY-003: No Non-Monotonic Transition Without Barrier

**Traces to**: INV-TOPOLOGY-006, INV-TOPOLOGY-007, ADR-TOPOLOGY-002
**Type**: Negative case
**Statement**: A topology transition that removes channels must go through the
Tier NM protocol (barrier + grace period + connectivity verification). Bypassing
the barrier risks in-flight data loss.
**Violation condition**: A channel is deactivated without completing the grace
period, or the agent graph is not verified connected post-transition.
**Recovery**: Automatic rollback (re-assert old topology — always Tier M).

---

#### NEG-TOPOLOGY-004: No Orphaned Agents

**Traces to**: INV-TOPOLOGY-008
**Type**: Negative case
**Statement**: No active agent may be disconnected from all other active agents.
The agent graph must remain connected after every enacted transition.
**Violation condition**: After a transition, there exist agents α, β with no path
between them through active channels.
**Recovery**: Automatic rollback + Signal::TopologyTransitionFailed at High severity.

---

#### NEG-TOPOLOGY-005: No Coupling Score Fabrication

**Traces to**: NEG-SEED-001 (no fabricated context), INV-TOPOLOGY-004
**Type**: Negative case
**Statement**: Coupling scores must be derived from observable data (file paths,
invariant dependencies, schema relationships, causal transactions, historical
outcomes). Fabricated or hardcoded coupling scores violate the learning loop.
**Violation condition**: A coupling score is asserted without a computable
derivation from store data.

---

### §19.5 Algorithms and Protocols

#### COLD_START Algorithm

```
COLD_START(agents: Set<Agent>, tasks: Set<Task>) → Topology:

  n ← |agents|
  C ← n×n zero matrix                     // coupling matrix

  // Phase 1: Compute coupling from available signals
  for each pair (Tᵢ, Tⱼ) in tasks:
    C[assigned(Tᵢ)][assigned(Tⱼ)] += file_coupling(Tᵢ, Tⱼ)
  if store.has_schema(:inv/depends-on):
    for each pair (Tᵢ, Tⱼ):
      C[assigned(Tᵢ)][assigned(Tⱼ)] += 0.6 · invariant_coupling(Tᵢ, Tⱼ)

  // Phase 2: Select topology
  if n ≤ 3: return Mesh(agents)            // O(6) overhead is trivial
  clusters ← spectral_partition(C, θ=0.3)
  if |clusters| = 1:
    if n ≤ 5: return Mesh(agents)
    hub ← argmax_a Σ C[a][*]
    return Star(hub, agents)
  else:
    sub ← {}
    for cluster k in clusters:
      if |agents_k| ≤ 3: sub[k] ← Mesh(agents_k)
      else: sub[k] ← Star(argmax coupling within k, agents_k)
    bridge ← agent maximizing cross-cluster coupling
    return Hybrid(sub, bridge, inter_freq=:periodic)
```

**Convergence**: 3–10 sessions depending on project variance.

#### Tier NM Transition Protocol

```
1. PROPOSE: Assert topology change with status :proposed
2. PROPAGATE: Change flows through existing topology (Tier M datom assertion)
3. ACKNOWLEDGE: Each affected agent asserts acknowledgment
4. BARRIER: Query for complete acknowledgment (non-monotonic query, requires sync)
5. GRACE PERIOD: Old and new channels both active for duration δ
   δ = base_grace · commitment_weight(transition)
6. ENACT: Status → :enacted, agents switch within one merge cycle
7. VERIFY: Post-transition BFS/DFS connectivity check
8. On TIMEOUT: Status → :timed_out, old topology remains,
   Signal::TopologyTransitionFailed
9. On FAILED verification: ROLLBACK (re-assert old channels, always Tier M)
```

#### F(T) Bilateral Loop

```
F(T) measured → detect drift (which Dᵢ dropped?) →
  D₂↑: tighten Φ (more merging)
  D₄↑: relax Φ (less merging)
  D₅↓: rebalance Π (reassign tasks)
  D₁↓: scale up Σ or unblock critical path
  D₇↑: trigger earlier harvest
→ propose change → transition protocol → measure F(T') → harvest outcome
```

---

### §19.6 Datom Schema

#### Agent Entity

| Attribute | Type | Resolution | Stage |
|-----------|------|-----------|-------|
| :agent/program | Keyword | LWW | 0 |
| :agent/model | Keyword | LWW | 0 |
| :agent/capabilities | Multi-value set | Multi | 0 |
| :agent/status | Keyword | LWW | 0 |
| :agent/cluster | Ref | LWW | 3 |
| :agent/frontier | Ref | LWW | 3 |

#### Channel Entity

| Attribute | Type | Resolution | Stage |
|-----------|------|-----------|-------|
| :channel/from | Ref | LWW | 3 |
| :channel/to | Ref | LWW | 3 |
| :channel/type | Keyword | LWW | 3 |
| :channel/merge-frequency | Keyword | Lattice(max) | 3 |
| :channel/active | Boolean | LWW | 3 |
| :channel/coupling-score | Double | Lattice(max) | 3 |

#### Merge Frequency Lattice

```
:realtime > :high > :medium > :low > :session > :trunk-only

Resolution mode Lattice(max): concurrent proposals resolve to higher frequency
(conservative direction — more coordination is always safe).
```

---

### §19.7 Open Questions

#### OQ-TOPOLOGY-001: Signal Composition for Topology Events (0.55)

New signal types (TopologyDrift, CouplingSpike, MergeConflict, ScalingRecommendation,
TopologyTransitionFailed, AgentIdle, CriticalPathBlocked, BalanceDrift) need
interaction rules and priority ordering. Resolution: design as Stage 3 extension
to spec/09-signal.md.

#### OQ-TOPOLOGY-002: Agent Capability Discovery (0.50)

How the system learns agent capabilities — self-declaration, observed task
completions, or both. Capability granularity (:store vs :store/transact) and
model-specific capabilities unresolved. Resolution: empirical observation across
5–10 multi-agent sessions.

#### OQ-TOPOLOGY-003: Cross-Project Topology (0.40)

Whether separate stores with federated merge (Option C from exploration) or
shared coordination namespace is needed. Resolution: implement federated stores,
observe cross-project dependency frequency.

#### OQ-TOPOLOGY-004: Observability and Debugging (0.55)

Visualization medium for topology graphs and F(T) dashboards. How much topology
context fits in the k* attention budget for guidance footers. Resolution: start
with CLI text + guidance footer, add Mermaid export, defer TUI to Stage 4.

#### OQ-TOPOLOGY-005: Formal Verification Feasibility (0.45)

Whether Kani can verify topology properties involving graph algorithms. Whether
StateRight model checking is feasible for transition protocol with n > 5.
Resolution: implement V:PROP first, attempt V:KANI with conservative bounds.

---

### §19.8 Compilation Pipeline (ADR-TOPOLOGY-004 Detail)

The topology-as-compilation paradigm uniquely differentiates Braid from all
existing multi-agent frameworks.

#### Front-End: Spec Graph → Coupling IR

```
CouplingIR = {
  nodes: Set<TaskId>,
  edges: Map<(TaskId, TaskId), CouplingWeight>,
  annotations: Map<(TaskId, TaskId), CouplingAnnotation>
}

CouplingWeight = {
  structural: f64,      // From spec dependency graph (exact)
  historical: f64,      // From PGO data (learned, initially 0)
  combined: f64,        // Weighted combination
  confidence: f64,      // Prediction confidence
}
```

Spec dependencies (:inv/depends-on, :adr/affects, :inv/constrains, :neg/tests)
form a directed graph. Tasks bound to overlapping spec elements are structurally
coupled.

#### Middle-End: Optimization Passes

1. **Partition**: Spectral partition of coupling IR into clusters
2. **Critical path**: Longest-path through dependency DAG
3. **CALM classify**: Monotonic tasks (parallelizable) vs non-monotonic (sequential)
4. **Merge elimination**: Clusters with coupling < θ need no direct channel

#### Back-End: Emit T = (G, Φ, Σ, Π)

Emit complete topology configuration as datom transaction.

#### PGO: Profile-Guided Optimization

```
prediction_delta(session) = |predicted_coupling − observed_coupling|

After each harvest:
  Update coupling weights toward observed values
  Adjust partition thresholds based on conflict/overhead data
  Refine CALM classifications based on actual sync requirements
```

**INV-TOPOLOGY-015**: Compiled topology weakly dominates cold-start for
spec-bearing projects. Compiled uses structural coupling (exact) while cold-start
uses only file coupling (heuristic).

**INV-TOPOLOGY-016**: PGO prediction error decreases monotonically after warm-up
period (≥ 3 sessions with outcome data).

---

### §19.9 Cross-Reference Summary

| Element | Depends On | Nature |
|---------|-----------|--------|
| INV-TOPOLOGY-001 | INV-STORE-012 (LIVE index) | Structural |
| INV-TOPOLOGY-002 | INV-STORE-001, L1–L5 (CRDT) | Safety (convergence) |
| INV-TOPOLOGY-004 | INV-STORE-001 (semilattice) | Measurement |
| INV-TOPOLOGY-006 | L4 (monotonicity), CALM | Safety (transition) |
| INV-TOPOLOGY-007 | INV-SYNC-001, C1 | Safety (data loss) |
| INV-TOPOLOGY-008 | INV-QUERY-* (BFS/DFS) | Safety (connectivity) |
| INV-TOPOLOGY-012 | INV-BILATERAL-001 | Progress (convergence) |
| INV-TOPOLOGY-014 | INV-TRILATERAL-001 | Progress (quadrilateral) |
| ADR-TOPOLOGY-002 | spec/03-query.md, 08-sync.md | Extension |
| ADR-TOPOLOGY-003 | spec/04-resolution.md | Extension |
| ADR-TOPOLOGY-004 | C7 (self-bootstrap) | Novel |

**Two independent invariant chains**:
1. **Convergence**: Cold-start safety (011) → Monotonic relaxation (010) →
   F(T) improvement (012) → Independence (013) → F_total (014)
2. **Transition**: Monotonic safety (006) → Rollback safety (009) →
   Connectivity (008) → Grace period (007)

---

*This specification distills ~5,900 lines of exploration (14 documents) into
formal invariants, ADRs, and negative cases. All claims are falsifiable, all
decisions have alternatives and rationale, all open questions have confidence
levels. The topology framework is Stage 3 (multi-agent coordination) but its
schema foundations (agent entities, coupling weights) can be introduced
incrementally from Stage 0.*
