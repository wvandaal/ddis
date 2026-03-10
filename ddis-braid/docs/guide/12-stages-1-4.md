# §12. Future Stages (1–4) — Roadmap & Extension Points

> **Spec reference**: [spec/17-crossref.md](../spec/17-crossref.md) §17.3
> **SEED.md**: §10 (The Bootstrap)
> **Purpose**: What each stage adds, what Stage 0 code must leave room for,
> and what this guide explicitly does NOT cover.

---

## §12.1 Stage 1 — Budget-Aware Output + Guidance Injection

**26 additional INVs** | Builds on Stage 0

### New Capabilities

- **Q(t) measurement**: Continuous attention budget tracking (INV-BUDGET-001–006)
- **Output precedence**: Prioritize high-value content when budget is low (INV-BUDGET-003)
- **Guidance compression**: Adapt guidance footers to remaining budget (INV-GUIDANCE-003–004)
- **Harvest warnings**: Proactive Q(t)-based harvest prompts (INV-INTERFACE-004/007)
- **Statusline bridge**: Real-time metrics to IDE statusbar (INV-INTERFACE-004)
- **Significance tracking**: Hebbian access log for datom relevance (INV-QUERY-003/008–009)
- **Frontier-scoped queries**: Stratified Datalog with frontier parameter (INV-QUERY-003)
- **Basic bilateral loop**: F(S) computation, convergence monitoring (INV-BILATERAL-001–002/004–005)
- **Confusion signal**: First signal type — agent confusion detection (INV-SIGNAL-002)
- **FP/FN calibration**: Harvest detection quality tuning (INV-HARVEST-004/006)
- **CLAUDE.md relevance/improvement**: Dynamic CLAUDE.md quality tracking (INV-SEED-007–008)
- **Betweenness centrality**: Bottleneck detection in dependency graphs (INV-QUERY-015)
- **HITS hub/authority**: Dual scoring for dependency structure (INV-QUERY-016)
- **k-Core decomposition**: Tightly coupled component identification (INV-QUERY-018)

### Stage 0 Extension Points

Code that Stage 0 must structure to accommodate Stage 1 (but NOT prematurely implement):

| Extension Point | Stage 0 Design | Stage 1 Addition |
|----------------|---------------|-----------------|
| Output formatting | `OutputMode` enum with `json/agent/human` | Add budget-aware truncation to `agent` mode |
| Guidance footer | Static footer selection | Budget-parameterized footer compression |
| Query engine | Monotonic evaluation only | Add `Stratified(Frontier)` mode to evaluator |
| Harvest trigger | Manual `braid harvest` only | Auto-detect Q(t) < threshold, emit warning |
| Store statistics | Basic counts in `braid status` | Add Q(t), F(S), significance scores |

**Design rule**: Leave the `match` arms. If an enum has `QueryMode::Monotonic`, adding
`QueryMode::Stratified(Frontier)` later requires only the new match arm and the evaluator
code — no structural refactoring.

---

## §12.2 Stage 2 — Branching + Deliberation

**23 additional INVs** | Builds on Stage 1

### New Capabilities

- **W_α working set**: Per-agent isolated workspace (INV-STORE-013, INV-MERGE-003–007)
- **Patch branches**: Create/switch/merge/compare/prune/lock (INV-MERGE-003–007)
- **Branch comparison**: Structured diff between competing branches (INV-MERGE-006)
- **Deliberation lifecycle**: Propose → Discuss → Stabilize → Crystallize (INV-DELIBERATION-001–006)
- **Precedent queries**: Query past deliberation outcomes for guidance (INV-DELIBERATION-003)
- **Stability guard**: Block premature crystallization (INV-DELIBERATION-002)
- **Bilateral symmetry**: Forward and backward flow use identical branch mechanics (INV-BILATERAL-003)
- **Diamond lattice signals**: Schema lattice signal generation (INV-SIGNAL-005, INV-SCHEMA-008)
- **Guidance expansion**: Drift detection triggers branch-level guidance (INV-GUIDANCE-006)
- **Delegation topology**: Multi-agent harvest delegation (INV-HARVEST-008)
- **Query extensions**: Branch-scoped queries and projection reification (INV-QUERY-004/011)
- **Eigenvector centrality**: Refined recursive influence scoring (INV-QUERY-019)
- **Articulation points**: Single-point-of-failure detection (INV-QUERY-020)
- **Topology fitness**: Phase-topology optimization T(t) (INV-GUIDANCE-011)

### Stage 0 Extension Points

| Extension Point | Stage 0 Design | Stage 2 Addition |
|----------------|---------------|-----------------|
| Store structure | Single flat datom set | Branching G-Set: trunk + branch sets |
| Merge | Pure set union of two stores | Branch merge with ancestry tracking |
| EntityId | Content-addressed from content | Same — branch metadata uses new attributes |
| Frontier | Per-agent TxId | Per-agent-per-branch TxId |

---

## §12.3 Stage 3 — Multi-Agent Coordination

**11 additional INVs** | Builds on Stage 2

### New Capabilities

- **Full sync barriers**: Consistent cut across agent frontiers (INV-SYNC-001–005)
- **Eight signal types**: Complete signal taxonomy with three-tier routing (INV-SIGNAL-001/003–004/006)
- **Subscription completeness**: Every signal type has at least one subscriber (INV-SIGNAL-006)
- **Topology-agnostic queries**: Query results independent of network topology (INV-QUERY-010)
- **Human signal injection**: External signal input for human-in-the-loop (INV-INTERFACE-006)

### Stage 0 Extension Points

| Extension Point | Stage 0 Design | Stage 3 Addition |
|----------------|---------------|-----------------|
| AgentId | Single agent, hardcoded | Multi-agent registry, per-agent frontiers |
| Signal system | No signals | Eight signal types, dispatch, subscription |
| Merge | Two-store merge | N-store merge with barrier coordination |

---

## §12.4 Stage 4 — Advanced Intelligence

**2 additional INVs** | Builds on Stage 3

### New Capabilities

- **Learned guidance**: Track guidance effectiveness, adjust recommendations (INV-GUIDANCE-005)
- **TUI**: Interactive terminal interface with subscription liveness (INV-INTERFACE-005)
- **Spectral authority**: Eigenvector centrality for datom importance (deferred)
- **Significance-weighted retrieval**: Access-log-based relevance ranking (deferred)

---

## §12.5 What This Guide Does NOT Cover

1. **Stage 1–4 internal design**: The guide specifies extension points, not implementation
   details. Those stages will get their own guide sections when their spec elements activate.

2. **Performance optimization**: Correctness first (NEG-003). Stage 0 uses `BTreeSet<Datom>`
   in memory (see guide/01-store.md for store data structure details). Optimization to
   persistent B-tree indexes (redb) is a binary-crate concern, not a kernel concern.

3. **Deployment infrastructure**: Docker, systemd, monitoring. Stage 0 is a CLI tool
   invoked by agents on a single VPS.

4. **Deductive verification** (Verus/Creusot): Deferred to post-Stage 2. Deferral
   rationale: Verus/Creusot require nightly Rust and have limited ecosystem maturity.
   Kani (bounded model checking) is the Stage 0 verification tool per ADR-VERIFICATION-001.
   Proptest + Kani provide sufficient confidence during initial implementation.

5. **TLA+ specifications**: Protocol model checking is specified but the TLA+ models
   themselves are implementation artifacts, not guide content.

---

## §12.6 Stage Dependencies

```
Stage 0a (49 INV) ← Store + Layout + Schema + Query + Resolution (foundation)
    ↓
Stage 0b (34 INV) ← Harvest + Seed + Merge + Guidance + Interface + Trilateral (lifecycle)
    ↓
Stage 1  (26 INV) ← Budget + Advanced Graph Metrics + Bilateral + Confusion Signal
    ↓
Stage 2  (23 INV) ← Branching + Deliberation + Topology
    ↓
Stage 3  (11 INV) ← Multi-Agent + Full Signal System
    ↓
Stage 4  (2 INV)  ← Intelligence
```

**Total**: 145 INVs across all stages. Stage 0 contains 57.2% of all invariants (83 INV) —
it is the foundation. Stages 1–4 add 62 INVs as extensions.

**Stage 0 sub-staging**: The 83-INV scope splits naturally into Stage 0a
(STORE/LAYOUT/SCHEMA/QUERY/RESOLUTION = 49 INV) and Stage 0b
(HARVEST/SEED/MERGE/GUIDANCE/INTERFACE/TRILATERAL = 34 INV). Stage 0a validates the core
store hypothesis before Stage 0b builds the lifecycle layer. See guide/README.md for the
full breakdown and cross-stage dependency notes.

---

## §12.7 INV Activation by Stage

INVs that move from deferred to active at each stage transition:

| Transition | Newly Active INVs |
|------------|-------------------|
| → Stage 1 (26 INV) | INV-BUDGET-001–006, INV-GUIDANCE-003–004, INV-BILATERAL-001–002/004–005, INV-INTERFACE-004/007, INV-QUERY-003/008–009/015–016/018, INV-SIGNAL-002, INV-HARVEST-004/006, INV-SEED-007–008, INV-TRILATERAL-004 |
| → Stage 2 (23 INV) | INV-STORE-013, INV-SCHEMA-008, INV-MERGE-003–007, INV-DELIBERATION-001–006, INV-SIGNAL-005, INV-GUIDANCE-006/011, INV-BILATERAL-003, INV-QUERY-004/011/019–020, INV-HARVEST-008–009 |
| → Stage 3 (11 INV) | INV-SYNC-001–005, INV-SIGNAL-001/003–004/006, INV-QUERY-010, INV-INTERFACE-006 |
| → Stage 4 (2 INV) | INV-GUIDANCE-005, INV-INTERFACE-005 |

---
