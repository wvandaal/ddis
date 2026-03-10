# 08 — Open Questions

> **Summary:** Five areas remain unexplored or uncertain. Each is analyzed with what we know,
> what's uncertain, and what evidence or design work would resolve it. These are candidates
> for future exploration sessions.

---

## 1. Signal System for Topology Events

### What We Know
The Braid spec (spec/09-signal.md) defines a signal system with typed divergence events
routed through three-tier severity. The topology framework introduces new divergence types
that need signal definitions.

### Proposed Signals

| Signal | Fires When | Severity | Route To |
|--------|-----------|----------|----------|
| TopologyDrift | F(T) decreases for 3 consecutive measurements | Medium | Agent + human notification |
| CouplingSpike | Coupling between two agents exceeds 0.7 (high) | Low | Agent notification (suggest tighter merge) |
| MergeConflict | Merge produces conflict entity | Medium | Affected agents |
| ScalingRecommendation | Scaling authority recommends add/remove | Low-Medium | Human (if Tier 2/3) |
| TopologyTransitionFailed | Tier NM barrier timed out | High | Human required |
| AgentIdle | Agent utilization < 0.1 for > 10 minutes | Low | Assignment policy (Pi) |
| CriticalPathBlocked | Task on critical path has no assigned agent | High | Human + scaling policy |
| BalanceDrift | D5 (balance) < 0.5 | Medium | Assignment policy (Pi) |

### What's Uncertain
- **Signal composition:** Can multiple topology signals fire simultaneously? How do they interact?
- **Signal priority:** When TopologyDrift and CouplingSpike fire together, which takes precedence?
- **Signal attenuation:** Should topology signals respect the k* attention budget (spec/13-budget.md)?
- **Cross-signal inference:** Does CouplingSpike + MergeConflict together imply a stronger
  recommendation than either alone?

### What Would Resolve It
Design the signal interaction rules as part of the Stage 3 specification. The existing
signal system (spec/09-signal.md) has a framework for signal composition — extend it to
topology signals. Key design question: are topology signals a new signal category or
instances of existing categories (Confusion, Conflict, Drift)?

---

## 2. Agent Capability Modeling

### What We Know
The assignment policy Pi references `capability_match(agent, task)` — matching agent skills
to task requirements. Agent capabilities are stored as multi-value attributes:

```
[agent:alpha :agent/capabilities #{:store :query :schema} tx:1 assert]
```

### Proposed Capability Schema

| Attribute | Type | Purpose |
|-----------|------|---------|
| :agent/capabilities | Multi-value keywords | Domain competencies (store, query, schema, etc.) |
| :agent/model | Keyword | LLM model (opus-4.6, sonnet-4.6, haiku-4.5) |
| :agent/program | Keyword | Harness (claude-code, codex, gemini) |
| :agent/success-rate | Double per task-type | Historical success rate |
| :agent/specialization-score | Double per domain | Learned specialization strength |

### What's Uncertain
- **Capability discovery:** How does the system learn an agent's capabilities? Self-declaration?
  Observation of successful task completions? Both?
- **Capability granularity:** Is `:store` one capability, or should it be `:store/transact`,
  `:store/merge`, `:store/index`?
- **Capability evolution:** Do capabilities change over time? An agent that successfully
  completes 5 store tasks may develop a `:store` specialization it didn't start with.
- **Model-specific capabilities:** Does opus-4.6 have inherently different capabilities than
  sonnet-4.6? How is this represented?
- **Human capabilities:** How are human capabilities represented? Humans have domain expertise,
  aesthetic judgment, and authority that AI agents don't.

### What Would Resolve It
Empirical observation across 5-10 multi-agent sessions. Track which agents succeed at which
task types. Compute specialization scores after each session. The capability model should
emerge from data, not be predefined. Start with coarse capabilities (:store, :query, :harvest)
and refine granularity based on observed prediction accuracy.

---

## 3. Cross-Project Topology

### What We Know
The user works across multiple projects simultaneously: ddis-braid, ddis-cli, ddis-modular,
research, and others. Agents may work on related tasks across projects (e.g., spec change in
ddis-braid requires CLI update in ddis-cli).

### Proposed Architecture

**Option A: Separate stores, shared coordination layer**
Each project has its own datom store. A meta-coordination layer (separate store or shared
namespace) tracks cross-project topology.

**Option B: Single store with project namespaces**
One datom store with project as a namespace attribute:
```
[task:t1 :task/project "ddis-braid" tx:100 assert]
[task:t2 :task/project "ddis-cli" tx:101 assert]
[task:t1 :task/cross-depends task:t2 tx:102 assert]  ;; cross-project dependency
```

**Option C: Federated stores with merge**
Each project has its own store. Cross-project coordination uses the same CRDT merge
mechanism — stores are merged when cross-project coordination is needed, carrying only
the coordination datoms (not all knowledge).

### What's Uncertain
- **Merge scope:** Should cross-project merges carry all datoms or only coordination-relevant
  ones? Full merge is simple but may leak project-specific knowledge.
- **Coupling across projects:** How is cross-project coupling computed? File paths don't
  overlap across projects. Invariant coupling might (if projects share spec elements).
- **Topology granularity:** Is there one topology per project, or one global topology across
  all projects?
- **Isolation requirements:** Can an agent working on ddis-braid accidentally see sensitive
  datoms from a different project?

### What Would Resolve It
Implement Option C (federated stores with merge) as the default. Each project maintains
its own datom store with its own topology. Cross-project coordination creates a temporary
merge scope containing only coordination datoms (tasks, assignments, coupling scores).
This preserves project isolation while enabling cross-project topology optimization.

Key design question to resolve empirically: how often do cross-project dependencies actually
create meaningful coupling? If rarely, separate-stores-with-occasional-merge is fine. If
frequently, a shared coordination namespace may be needed.

---

## 4. Observability and Debugging

### What We Know
Humans need to understand, debug, and intervene in topology decisions. The topology is
computed by Datalog queries over datoms — it's not immediately visible or intuitive.

### Proposed Observability Features

**4.1 Topology Visualization**
- Current topology as graph (agents as nodes, channels as edges, merge frequency as edge weight)
- Coupling matrix heatmap (which agents are tightly coupled)
- F(T) dashboard (seven dimensions, current scores, trends)
- Cluster membership diagram (which agents are in which clusters)

**4.2 Diagnostic Queries**
```
braid topology status          ;; Current topology summary
braid topology fitness         ;; F(T) with dimension breakdown
braid topology coupling        ;; Coupling matrix with sources
braid topology history         ;; Topology changes over time
braid topology explain <change> ;; Why was this change recommended?
braid topology simulate <change> ;; What would F(T) be if we made this change?
```

**4.3 Intervention Commands**
```
braid topology override <new-topology>  ;; Human override (provenance = Observed)
braid topology pin <channel> <frequency> ;; Pin a channel to specific frequency
braid topology freeze                    ;; Prevent automated topology changes
braid topology unfreeze                  ;; Re-enable automated changes
```

**4.4 Guidance Footer Integration**
Every agent's guidance footer includes topology context:
```
[topology] Hybrid: A(alpha,beta):mesh B(gamma,delta):mesh | F(T)=0.82 | D2=0.02
```

### What's Uncertain
- **Visualization medium:** How are graphs rendered? ASCII art in CLI? Mermaid diagrams?
  Interactive web UI? The Braid spec (spec/14-interface.md) defines five interface layers
  (ambient/CLI/MCP/guidance/TUI) — which is appropriate for topology visualization?
- **Simulation fidelity:** Can we accurately predict F(T) for a proposed topology change
  without actually enacting it? This requires a model of how topology affects outcomes,
  which is what the learning loop produces over time.
- **Information overload:** How much topology information should be in the guidance footer?
  The k* budget (spec/13-budget.md) limits guidance token count.

### What Would Resolve It
Start with CLI text output (braid topology status) and guidance footer integration. Add
Mermaid diagram export (braid topology graph --format mermaid) for documentation. Defer
interactive visualization to Stage 4 (TUI). The simulation capability requires historical
outcome data — defer to Stage 3+.

---

## 5. Formal Verification Strategy

### What We Know
The Braid spec defines a verification pipeline (spec/16-verification.md) with five gates:
1. cargo check (V:TYPE)
2. cargo test / proptest (V:PROP)
3. cargo kani (V:KANI)
4. stateright model checking (V:MODEL)

Each topology invariant should have a verification method.

### Proposed Verification Plan

| Invariant | Method | Strategy |
|-----------|--------|----------|
| INV-TOPO-TRANS-001 (Monotonic safety) | V:TYPE + V:PROP | Type system enforces monotonic transition only adds channels. Proptest: random transitions, assert channel count non-decreasing. |
| INV-TOPO-TRANS-002 (Grace period) | V:PROP | Inject datom just before grace period, verify arrival. |
| INV-TOPO-TRANS-003 (Connectivity) | V:PROP | After each transition, BFS connectivity check. |
| INV-TOPO-TRANS-004 (Rollback safety) | V:TYPE + V:PROP | Rollback function only adds channels. |
| INV-TOPO-COLD-001 (Monotonic relaxation) | V:PROP | Track coordination intensity across sessions. |
| INV-TOPO-COLD-002 (Cold-start safety) | V:PROP | Random coupling matrices, verify intensity bound. |
| INV-TOPO-FIT-001 (F(T) improvement) | V:PROP | Track F(T) before/after changes, assert improvement. |
| INV-TOPO-FIT-002 (F(T)/F(S) independence) | V:PROP | Correlation test across change events. |

### What's Uncertain
- **Kani feasibility:** Can Kani verify topology properties? The topology involves graph
  algorithms (partitioning, connectivity, critical path) which may exceed Kani's solver
  bounds. May need higher unwind bounds.
- **Model checking scope:** StateRight model checking for the transition protocol (state
  machine with PROPOSED -> ENACTED transitions) would provide strong guarantees. But the
  state space may be large (n agents x m channels x k topology types).
- **Property-based test effectiveness:** Proptest with 256 random inputs may not find
  edge cases in the coupling/topology/fitness interaction. May need guided fuzzing.

### What Would Resolve It
Implement V:PROP tests first (lowest cost, catches most bugs). Attempt V:KANI for the
transition protocol invariants (INV-TOPO-TRANS-*) with conservative unwind bounds.
Attempt V:MODEL for the full transition state machine with n <= 5 agents. Measure
verification time and coverage. Adjust bounds based on results.

---

## 6. Summary of Open Questions

| # | Question | Severity | Resolution Path |
|---|----------|----------|-----------------|
| 1 | Signal composition for topology events | Medium | Stage 3 spec design |
| 2 | Agent capability discovery and evolution | Medium | Empirical observation (5-10 sessions) |
| 3 | Cross-project topology architecture | Low | Implement federated stores; observe cross-project frequency |
| 4 | Observability and debugging | Medium | Start with CLI; iterate based on usage |
| 5 | Formal verification feasibility | Low | Implement V:PROP first; attempt V:KANI; measure |

None of these block the core topology framework design. All can be resolved incrementally
through implementation and observation. The framework is designed to be extended — new
signals, capability dimensions, and verification methods are additions to the schema,
not changes to the algebra.

---

*Next: `09-invariants-catalog.md` — complete catalog of all topology invariants.*
