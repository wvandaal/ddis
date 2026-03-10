# 10 — Design Decisions (ADRs)

> **Summary:** All architectural decision records from this exploration. Each documents the
> problem, options considered, decision made, rationale, and consequences. These will be
> distilled into formal ADRs when incorporated into the Braid spec.

---

## Naming Convention

```
TD-{CATEGORY}-{NNN}

Categories:
  SUBSTRATE  = Data substrate decisions
  ISOMORPHISM = Coordination model decisions
  COUPLING   = Coupling model decisions
  CALM       = CALM/transition decisions
  AUTHORITY  = Scaling authority decisions
  COLDSTART  = Cold-start decisions
  FITNESS    = Fitness function decisions
  REPLACE    = Replacement architecture decisions
```

---

## TD-SUBSTRATE-001: Coordination Messages Are Ephemeral; Decisions Are Datoms

**Problem:** Should coordination messages (Agent Mail, status pings, heartbeats) be stored
as immutable datoms in the Braid store?

**Options:**
1. **All coordination events as datoms.** Every message, heartbeat, status update is a datom.
   - Pro: Full history, complete queryability
   - Con: Massive store bloat from high-frequency ephemeral events. Violates C1 (append-only
     means they can never be cleaned up).
2. **No coordination events as datoms.** All coordination stays in external systems.
   - Pro: Store stays focused on knowledge
   - Con: Loses coordination learning capability; no topology improvement over time
3. **Two-tier: ephemeral substrate + harvested datoms.** Messages live in external systems
   (Agent Mail, session JSONL). The harvest functor extracts significant decisions, outcomes,
   and patterns into datoms.
   - Pro: Best of both worlds — real-time communication stays fast, durable knowledge is permanent
   - Con: Requires harvest bridge between substrates

**Decision:** Option 3 — Two-tier model.

**Rationale:** The harvest/seed lifecycle (SEED.md S5) already solves this exact problem for
human/AI conversations. Session JSONL is the ephemeral substrate; datoms are the durable
extract. The same architecture applies to AI/AI coordination. No new mechanism needed.

**Consequences:**
- Agent Mail (or equivalent) remains as the real-time communication substrate
- Harvest pipeline must be extended to ingest external coordination logs
- Seed assembly must project coordination context alongside knowledge context
- Coordination learning happens through the same bilateral loop as knowledge learning

---

## TD-ISOMORPHISM-001: Human/AI and AI/AI Coordination Are Structurally Identical

**Problem:** Should the topology framework have separate mechanisms for human/AI coordination
vs. AI/AI coordination?

**Options:**
1. **Separate mechanisms.** Different protocols for human-involved and AI-only coordination.
   - Pro: Can optimize each for its specific bandwidth/latency characteristics
   - Con: Doubles the design surface; requires bridging between the two systems
2. **Unified mechanism.** One coordination algebra for both, with differences handled by
   existing datom model features (provenance type, resolution mode, commitment weight).
   - Pro: Simpler design; leverages existing algebra; consistent behavior
   - Con: May under-optimize for specific human/AI interaction patterns

**Decision:** Option 2 — Unified mechanism.

**Rationale:** From first principles, both are instances of bounded-context frontier-bearing
agents exchanging assertions through a shared medium. The differences (bandwidth, latency,
authority) are quantitative, not qualitative. The datom model's provenance lattice
(Observed > Derived > Inferred > Hypothesized) and three-tier conflict routing already
handle the authority difference. No new mechanism needed.

**Consequences:**
- Human topology decisions use provenance = Observed (highest authority)
- AI topology recommendations use provenance = Derived or Inferred
- Three-tier conflict routing handles disagreements
- Same harvest/seed lifecycle for both coordination types

---

## TD-COUPLING-001: Composite Coupling Signal with Five Mechanisms and Learnable Weights

**Problem:** How should task coupling be measured to drive topology selection?

**Options:**
1. **Single signal (file paths only).** Simple Jaccard similarity of file sets.
   - Pro: Always available, easy to compute, directly predicts merge conflicts
   - Con: Misses semantic coupling, schema coupling, historical patterns
2. **Fixed multi-signal.** All five mechanisms with hardcoded weights.
   - Pro: More complete coupling picture
   - Con: Hardcoded weights may be wrong for specific project/task types
3. **Learnable multi-signal.** Five mechanisms with weights stored as datoms and updated
   through bilateral feedback loop.
   - Pro: Adapts to project-specific coupling patterns; improves over time
   - Con: More complex; cold-start requires reasonable defaults

**Decision:** Option 3 — Learnable multi-signal.

**Rationale:** INS-005 establishes that task-level regime routing outperforms fixed topology.
By analogy, task-level coupling measurement should outperform fixed coupling signals. The
bilateral learning loop (SEED.md S7) provides the update mechanism. The cold-start defaults
(file-path-dominated) provide reasonable initial behavior.

**Consequences:**
- Five coupling mechanisms defined (file, invariant, schema, causal, historical)
- Coupling weights are datoms, updated via bilateral feedback
- Staged introduction: signals added as data becomes available (Stage 0 -> Stage 3)
- Composite score preserves semilattice structure (composes with CRDT merge)

---

## TD-CALM-001: CALM Compliance Stratifies Topology Transitions

**Problem:** How should topology transitions (changing from one topology to another) be handled
safely?

**Options:**
1. **All transitions require barrier.** Every topology change goes through sync protocol.
   - Pro: Maximum safety
   - Con: Unnecessary overhead for safe (monotonic) changes; blocks all agents during transition
2. **No barriers.** All transitions are instant.
   - Pro: Maximum speed
   - Con: Non-monotonic transitions may lose in-flight data or orphan agents
3. **CALM-stratified.** Monotonic transitions (adding capacity) are barrier-free. Non-monotonic
   transitions (removing capacity) require barriers.
   - Pro: Optimal balance — safe changes are fast, risky changes are safe
   - Con: Requires correct classification of transitions (monotonic vs non-monotonic)

**Decision:** Option 3 — CALM-stratified.

**Rationale:** The CALM theorem is already the foundation of Braid's query classification
(spec/03-query.md). Applying it to topology transitions is a natural extension. The
classification is mechanical (compare channel sets before and after) — no human judgment
needed. This gives the best of both worlds: monotonic changes propagate instantly through
existing channels, while non-monotonic changes go through the sync barrier protocol.

**Consequences:**
- Two-tier protocol: Tier M (monotonic, no barrier) and Tier NM (non-monotonic, barrier required)
- Tier classification is automatic based on channel set comparison
- Non-monotonic transitions include grace period and connectivity verification
- Rollback is always monotonic (re-adding channels) -> always safe

---

## TD-AUTHORITY-001: Scaling Authority A(d) = R(1-C)T with Earned Trust

**Problem:** Who decides when to add/remove agents, and how is that authority determined?

**Options:**
1. **Always human.** All scaling decisions require human approval.
   - Pro: Maximum safety
   - Con: Human becomes bottleneck; can't scale autonomously
2. **Always autonomous.** System decides all scaling.
   - Pro: Maximum speed and autonomy
   - Con: High-commitment decisions without human oversight; trust not established
3. **Authority function with earned trust.** A(d) = R(1-C)T determines tier:
   autonomous (A > 0.5), recommend (0.2-0.5), human-only (A < 0.2). Trust T earned
   over time from successful outcomes.
   - Pro: Starts conservative, earns autonomy; high-commitment always involves human
   - Con: More complex; requires outcome measurement

**Decision:** Option 3 — Authority function with earned trust.

**Rationale:** The DDIS methodology emphasizes structural guarantees over process obligations.
A static "always human" or "always autonomous" policy is a process obligation that doesn't
adapt. The authority function is a structural guarantee: the system CAN'T act autonomously
on high-commitment decisions until it has earned trust through demonstrated competence.
The 4x harmful weight on bad outcomes ensures quick trust reduction for failures.

**Consequences:**
- Three-tier decision model (autonomous, recommend, human-only)
- Trust score T starts at 0.3 (conservative) and evolves
- Harmful outcomes decrease trust 4x faster than helpful outcomes increase it
- High-commitment decisions (removing active agent, scaling down by half) may never
  reach autonomous tier even at maximum trust — by design

---

## TD-COLDSTART-001: Monotonic Relaxation from Mesh (Minimax Strategy)

**Problem:** How should the topology be selected when there is no prior coordination history?

**Options:**
1. **Random topology.** Start with a random topology, learn from outcomes.
   - Pro: Explores topology space broadly
   - Con: May produce bad coordination; exploration phase wastes work
2. **Fixed conservative topology.** Always start with mesh.
   - Pro: Never under-coordinates; safe default
   - Con: May over-coordinate for large groups (O(n^2) overhead)
3. **Coupling-aware conservative.** Compute coupling from available signals (file paths),
   then select topology using cold-start algorithm. Start from maximum coordination
   within each cluster. Relax as evidence accumulates.
   - Pro: Adapts to project structure even on first session; safe upper bound
   - Con: More complex; depends on coupling signal quality

**Decision:** Option 3 — Coupling-aware conservative with monotonic relaxation.

**Rationale:** This is a minimax strategy: minimize the maximum possible loss. The cost
of over-coordination (merge overhead) is bounded and predictable. The cost of under-
coordination (conflicts, rework, knowledge loss) is unbounded. Starting from the safe
upper bound and relaxing monotonically guarantees that the system never under-coordinates
while converging toward optimal efficiency.

**Consequences:**
- COLD_START algorithm uses file coupling (always available) + invariant coupling (if available)
- Agent count thresholds determine initial topology type (mesh/star/hybrid)
- Spectral partitioning identifies natural clusters
- Each session relaxes coordination where evidence shows it's unnecessary
- Convergence in 3-10 sessions depending on project variance

---

## TD-FITNESS-001: Seven-Dimensional F(T) with Bilateral Convergence Loop

**Problem:** How should topology quality be measured?

**Options:**
1. **Single metric.** One number (e.g., throughput) measures topology quality.
   - Pro: Simple, easy to optimize
   - Con: Misses important dimensions (conflicts, balance, knowledge loss)
2. **Multi-dimensional.** Seven dimensions with weighted composition.
   - Pro: Comprehensive; identifies specific bottlenecks; enables diagnostic mapping
   - Con: More complex; weight selection is non-trivial
3. **Unstructured.** No formal fitness function; rely on human judgment.
   - Pro: Flexible
   - Con: Not mechanically evaluable; can't drive automated optimization

**Decision:** Option 2 — Seven-dimensional F(T) with weighted composition.

**Rationale:** F(S) already uses multi-dimensional composition (seven dimensions for
specification quality). Extending the pattern to topology provides consistency and
enables the bilateral convergence loop. The diagnostic mapping (dimension degradation
-> corrective action) is the key value — it turns topology optimization from guesswork
into a systematic process.

**Consequences:**
- Seven dimensions: throughput, conflict rate, staleness, merge overhead, balance,
  blocking time, knowledge loss
- Each dimension independently measurable via Datalog queries
- Dimension weights are datoms, tunable via bilateral feedback
- Bilateral loop: detect F(T) drop -> diagnose which dimension -> prescribe adjustment
- F(T) composes with F(S) into F_total for quadrilateral convergence

---

## TD-REPLACE-001: Braid Replaces Intelligence Layer; Ephemeral Substrate Remains

**Problem:** What is the relationship between the topology framework and the existing
ACFS coordination stack (br, bv, Agent Mail, ntm)?

**Options:**
1. **Braid replaces everything.** All coordination through datom store.
   - Pro: Maximum unification
   - Con: Real-time messaging through datom store may be too slow; every status ping is
     an immutable datom (store bloat)
2. **Braid augments (advisory only).** Existing tools remain; Braid provides recommendations.
   - Pro: Low risk; additive
   - Con: Doesn't achieve unification; two systems to maintain; learning loop limited
3. **Braid replaces intelligence layer; ephemeral substrate remains.** Braid's Datalog
   queries replace bv's graph analysis and ntm's assignment logic. Agent Mail (or equivalent)
   remains for real-time messaging. Harvest bridges the two.
   - Pro: Best of both worlds — unified intelligence, fast real-time communication
   - Con: Still two systems, but with clean separation (ephemeral vs durable)

**Decision:** Option 3 — Replace intelligence, keep ephemeral substrate.

**Rationale:** Consistent with TD-SUBSTRATE-001 (two-tier model). The intelligence layer
(topology selection, coupling computation, fitness measurement, outcome learning) naturally
lives in the datom store where it can be queried, versioned, and verified. The communication
layer (messages, heartbeats, file reservation checks) has different performance requirements
and naturally lives in a fast ephemeral substrate.

**Consequences:**
- bv's graph analysis (PageRank, betweenness, critical path) moves to Braid's Datalog queries
- ntm's assignment logic moves to Braid's assignment policy Pi
- Agent Mail remains for real-time AI/AI messaging and file reservations
- ntm remains for tmux session management (enactment layer)
- Harvest pipeline bridges Agent Mail -> datom store
- Seed assembly projects coordination context from datom store -> agent sessions

---

## Decision Dependency Graph

```
TD-SUBSTRATE-001 (two-tier model)
  |
  +-> TD-ISOMORPHISM-001 (unified mechanism)
  |
  +-> TD-REPLACE-001 (replace intelligence, keep ephemeral)
  |
  +-> TD-COUPLING-001 (learnable coupling)
       |
       +-> TD-COLDSTART-001 (coupling-aware cold-start)
       |
       +-> TD-FITNESS-001 (F(T) uses coupling for topology evaluation)

TD-CALM-001 (CALM-stratified transitions)
  |
  +-> TD-AUTHORITY-001 (authority determines transition tier for scaling)
```

Two independent decision chains:
1. **Substrate chain:** Two-tier -> unified mechanism -> replace intelligence -> learnable coupling -> cold-start + fitness
2. **Safety chain:** CALM transitions -> authority function

---

## Traceability Summary

| ADR | Traces to SEED.md | Traces to Spec | Traces to Research |
|-----|-------------------|----------------|-------------------|
| TD-SUBSTRATE-001 | S5 (harvest/seed) | spec/05-harvest.md | — |
| TD-ISOMORPHISM-001 | S6 (reconciliation taxonomy) | spec/01-store.md ADR-STORE-008 | — |
| TD-COUPLING-001 | S7 (self-improvement loop) | spec/12-guidance.md (R(t)) | INS-005 |
| TD-CALM-001 | S4 (CRDT merge) | spec/03-query.md (CALM), spec/08-sync.md | — |
| TD-AUTHORITY-001 | S6 (reconciliation) | spec/04-resolution.md (three-tier routing) | — |
| TD-COLDSTART-001 | S4 (CRDT), S7 (self-improvement) | spec/01-store.md L4 | Swarm Kernel Architecture |
| TD-FITNESS-001 | S7 (fitness function) | spec/10-bilateral.md (F(S)) | — |
| TD-REPLACE-001 | S5 (harvest/seed), S8 (interface) | spec/14-interface.md | INS-022 |

---

*This completes the exploration documentation. All design decisions, invariants, algorithms,
and open questions from the execution-topologies exploration session are captured in these
11 documents (README + 00 through 10).*
