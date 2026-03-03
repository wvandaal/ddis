# §12. Future Stages (1–4) — Roadmap & Extension Points

> **Spec reference**: [spec/17-crossref.md](../spec/17-crossref.md) §17.3
> **SEED.md**: §10 (The Bootstrap)
> **Purpose**: What each stage adds, what Stage 0 code must leave room for,
> and what this guide explicitly does NOT cover.

---

## §12.1 Stage 1 — Budget-Aware Output + Guidance Injection

**18 additional INVs** | Builds on Stage 0

### New Capabilities

- **Q(t) measurement**: Continuous attention budget tracking (INV-BUDGET-001–005)
- **Output precedence**: Prioritize high-value content when budget is low (INV-BUDGET-003)
- **Guidance compression**: Adapt guidance footers to remaining budget (INV-GUIDANCE-003–004)
- **Harvest warnings**: Proactive Q(t)-based harvest prompts (INV-INTERFACE-004/007)
- **Statusline bridge**: Real-time metrics to IDE statusbar (INV-INTERFACE-004)
- **Significance tracking**: Hebbian access log for datom relevance (INV-QUERY-003/008–009)
- **Frontier-scoped queries**: Stratified Datalog with frontier parameter (INV-QUERY-003)
- **Basic bilateral loop**: F(S) computation, convergence monitoring (INV-BILATERAL-001–002/004–005)
- **Confusion signal**: First signal type — agent confusion detection (INV-SIGNAL-002)

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

**17 additional INVs** | Builds on Stage 1

### New Capabilities

- **W_α working set**: Per-agent isolated workspace (INV-STORE-013, INV-MERGE-003–007)
- **Patch branches**: Create/switch/merge/compare/prune/lock (INV-MERGE-002–007)
- **Branch comparison**: Structured diff between competing branches (INV-MERGE-006)
- **Deliberation lifecycle**: Propose → Discuss → Stabilize → Crystallize (INV-DELIBERATION-001–006)
- **Precedent queries**: Query past deliberation outcomes for guidance (INV-DELIBERATION-003)
- **Stability guard**: Block premature crystallization (INV-DELIBERATION-002)
- **Bilateral symmetry**: Forward and backward flow use identical branch mechanics (INV-BILATERAL-003)
- **Diamond lattice signals**: Contradiction detection from lattice incomparability (INV-SIGNAL-005)
- **Guidance expansion**: Drift detection triggers branch-level guidance (INV-GUIDANCE-006)
- **Seed enrichment**: Branch-aware seed with competing contexts (INV-SEED-006)
- **Query lookahead**: Branch-scoped speculative queries (INV-QUERY-004, 010–011)

### Stage 0 Extension Points

| Extension Point | Stage 0 Design | Stage 2 Addition |
|----------------|---------------|-----------------|
| Store structure | Single flat datom set | Branching G-Set: trunk + branch sets |
| Merge | Pure set union of two stores | Branch merge with ancestry tracking |
| EntityId | Content-addressed from content | Same — branch metadata uses new attributes |
| Frontier | Per-agent TxId | Per-agent-per-branch TxId |

---

## §12.3 Stage 3 — Multi-Agent Coordination

**6 additional INVs** | Builds on Stage 2

### New Capabilities

- **Full sync barriers**: Consistent cut across agent frontiers (INV-SYNC-001–005)
- **Eight signal types**: Complete signal taxonomy with three-tier routing (INV-SIGNAL-001/003–004/006)
- **Subscription completeness**: Every signal type has at least one subscriber (INV-SIGNAL-006)
- **Conservative conflict detection**: Multi-agent conflict resolution (INV-RESOLUTION-003)
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
   in memory. Optimization to persistent B-tree indexes (redb) is a binary-crate concern,
   not a kernel concern.

3. **Deployment infrastructure**: Docker, systemd, monitoring. Stage 0 is a CLI tool
   invoked by agents on a single VPS.

4. **Deductive verification** (Verus/Creusot): Deferred to post-Stage 2. The cost is high
   and proptest + Kani provide sufficient confidence during initial implementation.

5. **TLA+ specifications**: Protocol model checking is specified but the TLA+ models
   themselves are implementation artifacts, not guide content.

---

## §12.6 Stage Dependencies

```
Stage 0 (64 INV) ← Foundation
    ↓
Stage 1 (18 INV) ← Budget + Guidance
    ↓
Stage 2 (17 INV) ← Branching + Deliberation
    ↓
Stage 3 (6 INV)  ← Multi-Agent
    ↓
Stage 4 (2 INV)  ← Intelligence
```

**Total**: 107 INVs across all stages. Stage 0 contains 59.8% of all invariants —
it is the bulk of the system. Stages 1–4 are extensions, not foundations.

---

## §12.7 INV Activation by Stage

INVs that move from deferred to active at each stage transition:

| Transition | Newly Active INVs |
|------------|-------------------|
| → Stage 1 | INV-BUDGET-001–006, INV-GUIDANCE-003–004, INV-BILATERAL-001–002/004–005, INV-INTERFACE-004/007, INV-QUERY-003/008–009, INV-SIGNAL-002 |
| → Stage 2 | INV-STORE-013, INV-MERGE-002–007, INV-SEED-006, INV-DELIBERATION-001–006, INV-SIGNAL-005, INV-GUIDANCE-006, INV-BILATERAL-003, INV-QUERY-004/010–011, INV-RESOLUTION-007 |
| → Stage 3 | INV-SYNC-001–005, INV-SIGNAL-001/003–004/006, INV-RESOLUTION-003, INV-INTERFACE-006 |
| → Stage 4 | INV-GUIDANCE-005, INV-INTERFACE-005 |

---
