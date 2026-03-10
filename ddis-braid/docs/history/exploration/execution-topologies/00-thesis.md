# 00 — The Core Thesis

> **Summary:** The coordination topology for multi-agent work is not an external orchestration
> concern — it is an emergent property of the datom store, computable by the same query engine
> that validates specifications, learnable through the same harvest/seed lifecycle that preserves
> knowledge, and convergent through the same bilateral loop that aligns spec with implementation.

---

## 1. The Problem: Coordination Is External to Knowledge

In the current ACFS coordination stack, four separate systems manage multi-agent work:

| Layer | Tool | Data Model | Communication |
|-------|------|-----------|---------------|
| Issues | br (beads) | SQLite + JSONL | CLI |
| Intelligence | bv (beads_viewer) | Reads br's JSONL | CLI (read-only) |
| Messaging | Agent Mail MCP | SQLite + Git archive | MCP/HTTP |
| Orchestration | ntm | tmux state | CLI + Agent Mail |

These tools work well. Agents self-organize effectively using bv's graph analysis, Agent Mail's
threading, and ntm's spawning. But there is a structural property of this architecture that
limits it:

**Coordination is _about_ knowledge but _external to_ the knowledge substrate.**

br tracks _what needs doing_. Agent Mail communicates _about it_. bv analyzes _it_. But none
of these live in the same substrate as the knowledge they coordinate around. Every bridge
between them — bead ID as Agent Mail thread_id, bv reading br's JSONL, ntm querying Agent
Mail for idle agents — is a handcrafted integration point that can drift. There is no single
query language, no unified reconciliation framework, no learning mechanism that spans all four.

---

## 2. The Insight: Topology Emerges from the Datom Store

Braid's datom store is `(P(D), union)` — a G-Set CvRDT where everything is a datom
`[entity, attribute, value, transaction, operation]`. The store has properties that make
it uniquely suited for coordination:

1. **Content-addressed identity (C2)**: If two agents independently discover the same coupling
   pattern, it deduplicates to one datom. Coordination knowledge converges automatically.

2. **Per-attribute resolution (spec S4)**: Different coordination attributes can use different
   conflict resolution modes. Task assignment: LWW (latest claim wins). File reservations:
   multi-value (all visible). Topology decisions: lattice (join toward more coordination).

3. **Frontier-relative queries (SQ-001-003)**: Each agent sees exactly what it knows about the
   coordination state. Non-monotonic coordination queries ("are all agents done?") require
   sync barriers — which DDIS already specifies.

4. **CALM compliance**: Monotonic coordination operations (adding tasks, claiming work, asserting
   observations) run without barriers. Non-monotonic operations (deciding "all tasks done,"
   selecting optimal topology from complete data) require explicit sync. This is exactly the
   right coordination/query boundary.

5. **Harvest/seed**: At session boundaries, coordination knowledge (which topologies worked,
   which agent specializations were effective) gets harvested into the store. Next session's
   seed includes that knowledge. The system _learns coordination patterns over time_.

The critical observation: **the coordination topology IS the merge topology.** In a CRDT store,
agents synchronize by merging their stores (set union). The _pattern_ of who merges with whom,
at what frequency, determines:
- Information flow (how quickly facts propagate)
- Consistency guarantees (how stale an agent's view can be)
- Coordination overhead (how much time is spent merging vs. working)

This means the coordination topology is not an external orchestration decision — it is a
structural property of how the datom store is used. And because the store contains the task
dependency graph, coupling information, and historical outcomes, the _optimal_ topology is
_computable from the store itself_ via Datalog queries.

---

## 3. The Conversation/Datom Boundary

There are two distinct layers in any agent coordination system:

### 3.1 The Ephemeral Conversation Substrate

High-volume, append-only, unstructured. Examples:
- Human/AI session logs (JSONL files outside project root)
- AI/AI messages (Agent Mail SQLite + Git archive)
- Real-time status updates, heartbeats, progress pings
- File reservation checks and conflict notifications

These are like the ocean — vast, mostly undifferentiated, flowing constantly. They are the
raw medium through which agents coordinate in real time.

### 3.2 The Durable Datom Store

Low-volume, append-only, structured, permanent. Examples:
- Coordination decisions (topology choices with rationale)
- Task state transitions (assigned, in-progress, completed, blocked)
- Coordination outcomes (quality scores, conflict counts, merge overhead)
- Learned patterns (topology-to-outcome correlations with confidence)
- Agent capability profiles (skill specializations, success history)

These are like mineral deposits — concentrated, structured, durable. They are the refined
extract that the system learns from.

### 3.3 The Harvest Functor

The bridge between these layers is the harvest operation — a functor:

```
H : ConversationLog -> DatomStore
```

Properties of H:
- **H is lossy**: Most conversation is noise. H extracts only signal.
- **H preserves provenance**: Every harvested datom carries a reference to its source.
- **H is monotonic**: More conversation can only add datoms, never retract.
- **H is idempotent on content**: Harvesting the same decision twice produces one datom (C2).

This is identical whether the source is human<->AI conversation or AI<->AI coordination
messages. The functor doesn't care about source type — it cares about content.

**Key analogy:** Just as a 500-turn conversation with Claude produces maybe 5-10 datoms worth
harvesting (key decisions, discovered invariants, observed patterns), a 200-message Agent Mail
thread between 5 agents produces maybe 10-20 datoms worth harvesting (topology decisions,
task completions, coordination patterns that worked, conflict resolutions).

---

## 4. The Human/AI Coordination Isomorphism

### 4.1 Claim: There Is No Structural Difference

Human/AI coordination and AI/AI coordination are structurally identical. Both are instances
of **bounded-context, frontier-bearing agents exchanging assertions through a shared medium.**

The structural isomorphism:

| Property | Human <-> AI | AI <-> AI |
|----------|-------------|-----------|
| Bounded context | Working memory (~7 +/- 2 chunks) | Context window (~200K tokens) |
| Produces assertions | Natural language, decisions, code reviews | Messages, code, status updates |
| Epistemic gap | Forgets between sessions | Context lost between sessions |
| Frontier | What the human remembers | What the agent's store contains |
| Harvest | "Write down what we decided" | Extract decisions as datoms |
| Seed | "Read yesterday's notes before starting" | Load relevant datoms into context |
| Divergence types | All eight types apply | All eight types apply |

### 4.2 What Differs: Resolution Authority, Not Coordination Structure

The differences are quantitative (bandwidth, latency, structure level), not qualitative:

- **Bandwidth**: AI/AI can exchange structured data at MB/s; human/AI limited by typing speed
- **Latency**: AI/AI merge within milliseconds; human/AI requires human processing time
- **Structure**: AI/AI messages can be schema-conformant; human messages are natural language
- **Scale**: AI/AI scales to dozens of agents; human/AI typically 1-3 humans
- **Authority**: Humans have axiological authority (define "good"); AI agents don't
- **Drift**: AI agents experience basin-competition drift; humans don't

The datom model handles these differences entirely through existing machinery:

1. **Provenance lattice** (ADR-STORE-008): Human assertions carry Observed (highest confidence).
   AI assertions carry Derived or Inferred. When human and AI disagree, resolution respects
   provenance ordering.

2. **Three-tier conflict routing**: Human/AI disagreements route to Tier 3 (human-required).
   AI/AI disagreements may resolve at Tier 1 (automatic) or Tier 2 (agent-notified).

3. **Commitment weight**: Human topology decisions have higher commitment weight (forward
   causal cone includes all subsequent agent work). AI topology recommendations have lower
   commitment weight (can be overridden by human).

No special human/AI distinction is needed at the architectural level. The coordination
algebra is the same. The differences are data (provenance type, commitment weight), not
structure.

---

## 5. Why This Matters for Braid

### 5.1 Unification Eliminates Impedance Mismatch

Currently, coordination and knowledge live in different systems with different data models,
query languages, and reconciliation mechanisms. Every bridge between them is a handcrafted
integration point. Datoms eliminate this: one substrate, one query language, one reconciliation
framework.

### 5.2 Temporal Completeness Enables Learning

The current stack has no memory of coordination patterns. bv computes optimal work _now_ but
doesn't know what worked _before_. Datoms carry full history — the system can query "what
topology produced the best outcomes for this task type over the last 5 sessions?"

### 5.3 Self-Bootstrap Applies to Coordination

The coordination system's own invariants are datoms in the store, verified by the same
contradiction engine. If the coordination topology selection invariants are incoherent,
the system detects it. DDIS specifies DDIS -> DDIS coordinates DDIS.

### 5.4 CALM Compliance Gives the Right Taxonomy

Monotonic coordination operations (adding tasks, claiming work) need no barriers. Non-monotonic
operations (deciding "all tasks done," selecting topology from complete data) require sync.
This is exactly the right boundary, and it falls out of the existing CALM compliance framework.

### 5.5 The Research Validates Topology as Critical

INS-022 established that coordination topology is the strongest system differentiator across
all examined lineages. INS-005 established that task-level regime routing outperforms fixed
topology. The datom store is the natural substrate for implementing these findings: it contains
the task coupling data, the historical outcomes, and the query engine to compute optimal
topology — all in one place.

---

## 6. The Architecture (High Level)

```
+-------------------------------------------------------------+
|             Conversation Substrate (ephemeral)               |
|  +------------+  +------------+  +----------------------+   |
|  | Session    |  | Agent      |  | Real-time            |   |
|  | JSONL      |  | Mail       |  | status/heartbeat     |   |
|  | (H<->AI)   |  | (AI<->AI)  |  | (AI<->AI)            |   |
|  +-----+------+  +-----+------+  +----------+-----------+   |
|        |               |                    |                |
|        +---------------+--------------------+                |
|                        |                                     |
|                +-------v--------+                            |
|                |   HARVEST (H)  | <-- extracts signal        |
|                +-------+--------+                            |
+------------------------+------------------------------------+
                         |
+------------------------v------------------------------------+
|              Datom Store (durable)                            |
|                                                              |
|  Knowledge datoms    |  Coordination datoms                  |
|  +----------------+  | +-------------------------------+    |
|  | Spec elements  |  | | Task states & assignments     |    |
|  | Invariants     |  | | Coupling weights (learned)    |    |
|  | ADRs           |  | | Topology decisions            |    |
|  | Observations   |  | | Coordination outcomes         |    |
|  +----------------+  | | Agent capability profiles     |    |
|                       | +-------------------------------+    |
|                       |                                      |
|  +--------------------v----------------------------------+   |
|  |  Query Engine (Datalog + Graph Algorithms)             |  |
|  |  - Coupling computation                                |  |
|  |  - Topology selection                                  |  |
|  |  - Assignment optimization                             |  |
|  |  - Fitness measurement                                 |  |
|  +--------------------+----------------------------------+   |
|                       |                                      |
|                +------v-------+                              |
|                |  SEED / R(t) | <-- projects context         |
|                +------+-------+                              |
+-------------------------------+-----------------------------+
                         |
                  +------v-------+
                  | Agent Context | <-- includes topology
                  | (CLAUDE.md)   |    recommendations
                  +--------------+
```

---

## 7. Traceability

| Claim in this document | Traces to |
|------------------------|-----------|
| Coordination topology = merge topology | SEED.md S4 (CRDT merge by set union) |
| Harvest functor bridges ephemeral <-> durable | SEED.md S5 (Harvest/Seed Lifecycle) |
| Human/AI isomorphism via provenance lattice | spec/01-store.md ADR-STORE-008 |
| CALM stratification of coordination | spec/03-query.md (CALM compliance, six strata) |
| Topology learning through bilateral loop | SEED.md S7 (Self-Improvement Loop) |
| Topology context in dynamic CLAUDE.md | SEED.md S8 (Interface Principles) |
| Self-bootstrap for coordination | SEED.md S1 (DDIS specifies itself), C7 |
| Eight divergence types apply to coordination | SEED.md S6 (Reconciliation Taxonomy) |
| INS-022, INS-005 research grounding | /data/projects/research/topics/agentic-coding-revolution/insights/ |

---

*Next: `01-algebraic-foundations.md` — the formal algebraic structure that makes this work.*
