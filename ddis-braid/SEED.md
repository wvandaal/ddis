# DDIS: Decision-Driven Implementation Specification

## Spec Seed Document

*This document is the seed from which the formal specification, the implementation, and the system itself will grow. The specification will be written using DDIS methodology — invariants with IDs and falsification conditions, ADRs with alternatives and rationale, negative cases, explicit uncertainty markers, contradiction detection — not as traditional prose documents. The system bootstraps from its own specification: the invariants written about the datom store become the first invariants the datom store will manage, the ADRs about design choices become the first ADRs it will track, and the contradictions caught during specification become the first test cases for contradiction detection. The specification is both the plan and the first dataset.*

*Every claim here should be either self-evident or traceable to a design rationale. Nothing should be here because it sounds good. Everything should be here because removing it would leave a gap.*

---

## 1. What DDIS Is

DDIS is a specification standard, a protocol, and a knowledge substrate whose purpose is to maintain **verifiable coherence** between what you want, what you specified, what got built, and how it actually behaves — across people, across AI agents, and across time.

The ultimate goal: to be able to say, at any point in a project of any scale, with formal justification rather than subjective confidence:

> I know what I want. It is logically coherent and internally consistent. This specification is a full and accurate formalization of what I want and how we will build it. The implementation traces back to the specification at every point. Where divergence exists, I know exactly where it is, what type it is, why it arose, and what resolving it requires.

This is true whether the project involves one person or a hundred AI agents. It is true whether the project has been running for a day or a year. It is true not because of human discipline or process compliance, but because of the structure of the system itself.

---

## 2. The Fundamental Problem: Divergence

Every project drifts. What you want, what you wrote down, how you said to build it, and what actually got built inevitably diverge. This happens to solo developers who forget why they made a decision three weeks ago. It happens to organizations where the spec says one thing and the implementation does another. It happens inside a single person's head when they hold contradictory beliefs about how a system should work.

The divergence occurs at every boundary in the chain from intent to reality:

- **Intent → Specification**: the spec doesn't capture what you actually want (axiological divergence)
- **Specification → Specification**: the spec contradicts itself (logical divergence)
- **Specification → Implementation**: the code doesn't match the spec (structural divergence)
- **Implementation → Observed Behavior**: the code doesn't do what it claims (behavioral divergence)

Existing tools treat coherence as a **process obligation**: "keep the docs updated," "review the spec before coding," "write tests." Process obligations decay under pressure — deadlines, fatigue, enthusiasm for the next feature. DDIS makes coherence a **structural property**: the specification, the implementation facts, and the verification machinery all live in the same substrate and are checked by the same automated mechanisms, continuously, on every change.

### The AI Agent Problem: A Special Case

AI agents make divergence worse because they're prolific producers of artifacts with zero durable memory. An AI agent can produce thousands of lines of code in an hour, but its "memory" of why it made each choice lasts only until the conversation ends. The volume of potential divergence scales with output volume, and AI agents have enormous output volume with zero cross-session continuity.

But AI agents also make divergence **more solvable than it has ever been**. A human checking a 50,000-line codebase against a 200-page spec is performing an impossibly tedious verification task. An AI agent with the right substrate — a structured, queryable store where specification and implementation facts coexist — can perform that verification continuously, automatically, and at scale. The same capability that produces divergence (fast, voluminous output) can be turned toward detecting and resolving it (fast, voluminous verification).

DDIS provides the substrate that makes this possible.

---

## 3. The Specification Formalism

The DDIS specification formalism provides specific machinery for detecting and resolving each type of divergence. Each element addresses a specific failure mode in the divergence chain (§2):

| Primitive | ID Pattern | Divergence Addressed | Role |
|-----------|-----------|---------------------|------|
| Invariant | `INV-{NS}-{NNN}` | Logical, structural | Falsifiable claim about what must hold |
| ADR | `ADR-{NS}-{NNN}` | Axiological | Record of why a choice was made |
| Negative Case | `NEG-{NS}-{NNN}` | Structural (underspecification) | Safety property: what must NOT happen |
| Uncertainty Marker | confidence 0.0–1.0 | Epistemic | Explicitly provisional claim |
| Contradiction Detection | 5-tier | Logical (internal) | Finds conflicts between specification elements |
| Fitness Function | F(S) → [0, 1] | All types (quantified) | Measures overall convergence |
| Bilateral Loop | — | All boundaries | Checks alignment in both directions |

The first four are **element types** — artifacts with IDs that become datoms in the store (C7). The last three are **verification mechanisms** — processes that operate over elements to detect and measure divergence.

**Invariants** are falsifiable claims about the system. Not wishes, not goals — statements that can be mechanically checked. Each invariant has an ID, a formal statement, an explicit violation condition ("this invariant is falsified if..."), and a verification method. Invariants are the primary mechanism for detecting **logical** and **structural** divergence. Every invariant requires:

- **ID** (`INV-{NAMESPACE}-{NNN}`) — individually addressable, traceable to this seed
- **Formal statement** — the claim about system behavior
- **Falsification condition** — "this invariant is violated if..." (C6: non-negotiable; an invariant without a falsification condition is not an invariant — it is a wish)
- **Verification method** — how the claim is mechanically checked

**Architectural Decision Records (ADRs)** capture why a choice was made, what alternatives were considered, and what tradeoffs were accepted. ADRs prevent the specific failure mode where someone revisits a decision without knowing why it was made, reverses it for seemingly good reasons, and breaks downstream invariants that depended on the original choice. ADRs are the primary mechanism for detecting **axiological divergence** — when the implementation undermines the goals that motivated the design. Every ADR requires:

- **ID** (`ADR-{NAMESPACE}-{NNN}`) — individually addressable
- **Problem** — the question being decided
- **Options** — all alternatives considered, with tradeoffs for each
- **Decision** — which option was chosen, with formal justification
- **Consequences** — what follows, especially which invariants depend on this choice

**Negative cases** define what the system must NOT do. They bound the solution space from the opposite direction as invariants: where invariants say "this must hold," negative cases say "this must never happen in any reachable state." Without explicit negative cases, an agent optimizing for one invariant may violate an unstated constraint. Negative cases prevent **structural divergence** arising from overspecification in one dimension and underspecification in another. Every negative case requires:

- **ID** (`NEG-{NAMESPACE}-{NNN}`) — individually addressable
- **Safety property** — what must never occur, stated as a temporal logic formula where possible
- **Violation condition** — observable evidence that the property has been breached

**The bilateral feedback loop** continuously checks alignment in both directions. Forward: does the implementation satisfy the specification? Backward: does the specification accurately describe the implementation? The loop converges when the specification fully describes the implementation and the implementation fully satisfies the specification.

**Contradiction detection** operates in tiers of increasing subtlety: exact (identical claims with different values), logical (mutually exclusive implications), semantic (different words, same conflict), pragmatic (compatible in isolation, incompatible in practice), and axiological (internally consistent but misaligned with goals). These tiers surface divergence that simpler checks would miss.

**The fitness function** quantifies convergence toward full coherence across multiple dimensions: coverage (every goal traces to invariants and back), depth (invariants are fully specified with violation conditions), coherence (no internal contradictions), completeness (no gaps between spec and implementation), formality (claims are falsifiable, not aspirational), certainty (uncertainty markers resolved toward commitment), and commitment (provisional decisions stabilized into ADRs). The formula F(S) is a weighted combination of these seven components (see docs/design/ADRS.md: CO-009).

**Explicit uncertainty markers** acknowledge what the specification doesn't know yet. Where a claim is provisional, it carries a confidence level and a description of what would resolve the uncertainty. This prevents the failure mode where aspirational prose reads like settled commitment, and agents implement uncertain claims as though they were axioms. Uncertainty is not a defect in the specification — it is information about where the specification needs more work. Every uncertainty marker requires:

- **Confidence** (0.0–1.0) — how settled the claim is
- **Source** — which element (invariant, ADR) the uncertainty qualifies
- **Impact if wrong** — what breaks if the uncertain claim proves false
- **Resolution criteria** — what evidence or experience would settle it
- **What survives** — which guarantees hold regardless of how the uncertainty resolves

### How the Primitives Interact

The seven primitives form a constraint network, not a flat list:

- **INV ← ADR**: Every invariant exists because a design decision established the property it asserts. Reversing an ADR without checking downstream invariants is a defect.
- **NEG bounds INV**: Invariants define what must hold; negative cases define what must not happen. Together they constrain the solution space from both directions.
- **UNC qualifies INV and ADR**: A high-confidence invariant is a commitment; a low-confidence one is a hypothesis. Agents must treat them differently — implementing a 0.4-confidence claim as an axiom is a known failure mode.
- **Contradiction detection operates over INV and ADR pairs**: Each detected contradiction becomes input to the deliberation mechanism (§6), producing a new ADR that resolves it.
- **F(S) measures all element types**: Coverage gaps (missing INVs), conflicts (contradicting INVs), unresolved uncertainties (open UNCs) all reduce the score.
- **Bilateral loop uses INV and NEG as its test suite**: Forward — does the implementation satisfy every INV and respect every NEG? Backward — does every implemented behavior trace to an INV or ADR? Untraceable behavior is a spec gap; unimplemented invariants are implementation gaps.

Adding or changing any element has consequences across the network. This is why contradiction detection and the bilateral loop exist — to surface those consequences automatically rather than discovering them by accident.

When this machinery is reified as structured data in the knowledge store, the coherence verification itself becomes queryable and auditable. "Show me every invariant and its current validation status." "Show me every ADR and whether the decision it records is still reflected in the implementation." "Show me every unresolved contradiction, classified by type and severity." "Show me every uncertainty marker and what would resolve it." These are queries, not manual audits. They run continuously, automatically, triggered by every new fact assertion.

---

## 4. The Core Abstraction: Datoms

The knowledge substrate underlying DDIS is an append-only store of **datoms** — atomic facts of the form:

```
[entity, attribute, value, transaction, operation]
```

A datom records that someone observed something about some entity at some point in time, and whether they're asserting or retracting it. That's the entire data model. There are no tables, no columns, no upfront schema. The schema emerges from usage, defined as data in the store itself (schema-as-data).

The store is the set of all datoms ever asserted. Nothing is deleted or mutated. "Changing" something means asserting a new fact; the retraction of the old fact is itself a new, appended fact. This gives three properties that matter enormously:

**Temporal completeness.** You can ask "what was believed to be true at any point in time" because the full history exists. Every state the system has ever been in is recoverable.

**Conflict-free merge.** Two agents who independently discover facts about the same project can merge their stores by set union — the mathematical operation, not a heuristic. No merge conflicts, no rebasing. This is a CRDT (specifically a G-Set) by construction, not by bolt-on.

**Causal traceability.** Every fact records its provenance: which agent, in which transaction, with which causal predecessors. You can always trace why something is believed, who asserted it, and what it depends on.

The choice of EAV (Entity-Attribute-Value) over relational tables or document stores is deliberate. EAV is schema-on-read: the structure of the data is determined at query time, not at write time. This matters because the ontology of a project evolves as the project evolves. Early in development, you don't know what entity types you'll need. With EAV, you assert facts and the schema crystallizes from usage. This aligns with how AI agents actually work: they discover the structure of the problem as they explore it.

### Design Commitments (Axioms)

These are the locked algebraic commitments of the system:

1. **Identity.** A datom is [e, a, v, tx, op]. The transaction tx is itself an entity in the store, carrying provenance (who, when, why) as its own attributes. Two agents independently asserting the same fact produce one datom (identity by content).

2. **Store.** The store is (P(D), ∪) — a grow-only set of datoms. Retractions are datoms with op=retract. The store never shrinks. All semantic resolution ("what's currently true?") is a query-layer concern.

3. **Snapshots.** Default query mode is the local frontier: all datoms known to this agent. Optional sync barriers establish consistent cuts for coordinated queries.

4. **Queries.** Monotonic queries (those that can only gain results as facts are added) run without coordination. Non-monotonic queries (negation, aggregation) are frontier-relative and may give different results on different agents until they sync.

5. **Resolution.** Each attribute declares a resolution mode for concurrent conflicting values: lattice-resolved (domain-specific merge), last-writer-wins, or multi-value (keep all). Lattice definitions are themselves facts in the store.

### Implementation Architecture

The store is an **embedded library** (no daemon process; FD-010), **file-backed** with git as the coordination and versioning layer (SR-006; SR-007 flock coordination superseded by structural concurrency via content-addressed per-transaction files — see SR-014, ADR-LAYOUT-006 in `spec/01b-storage-layout.md`). The runtime has a **three-layer architecture**: storage layer (datom store + indexes), protocol layer (operations + merge + sync), and interface layer (CLI + MCP + TUI + guidance; AA-003). Four core indexes — **EAVT** (entity lookup), **AEVT** (attribute-centric), **VAET** (reverse reference traversal), **AVET** (unique/range) — support efficient access patterns, with a fifth **LIVE** index materializing current-state views (SR-001, SR-002). Transaction ordering uses **Hybrid Logical Clocks**, providing causally-ordered, globally-unique timestamps without central coordination (SR-004). The schema bootstraps from **17 axiomatic meta-schema attributes** in a genesis transaction (tx=0) that define themselves recursively (SR-008, PO-012).

### Query Engine

Queries use a **Datomic-style Datalog dialect** with semi-naive evaluation and **CALM compliance**: monotonic queries run without coordination; non-monotonic queries are frontier-relative (FD-003). Queries classify into **six strata** of increasing power and cost (17 named patterns), each with a defined `QueryMode` controlling frontier scope and coordination requirements (SQ-004, SQ-009). **Frontier state is itself a datom attribute** (`:tx/frontier`), enabling Datalog frontier clauses that query "what does agent X know?" as ordinary data (SQ-002, SQ-003). At the Datalog/imperative boundary, derived functions use an **FFI mechanism** — Datalog handles relational joins while host-language functions handle computation (SQ-010).

### Agent Working Set and Recovery

Each agent maintains a **private working set W_α** — a two-tier store where uncommitted datoms are visible only locally; explicit `commit` promotes them to the shared store S (PD-001). The **crash-recovery model** recovers durable frontiers on restart; uncommitted W_α datoms replay or discard based on durability markers (PD-003). The canonical **TRANSACT** operation has a seven-field type signature: `(Store, Entity, Attribute, Value, TxData, CausalPredecessors, AgentID) → (Store', TxReceipt)` (PO-001). The **conflict predicate** is causal independence: two assertions conflict iff neither causally precedes the other and they target the same entity-attribute pair (CR-006).

Further protocol-level decisions — provenance typing lattice (PD-002), at-least-once delivery (PD-004), branch operations with competing-branch locks (PO-007), merge cascade semantics (PO-006), confusion signals triggering re-ASSOCIATE (PO-005), and the ten-step agent cycle (PO-011) — are indexed with full rationale in **`docs/design/ADRS.md`**.

---

## 5. The Harvest/Seed Lifecycle

The harvest/seed cycle is the mechanism by which knowledge survives conversation boundaries. This is the make-or-break feature.

**During a conversation**, the agent works normally — reading code, making decisions, discovering dependencies. Alongside this work, it asserts datoms: observations, decisions, dependency discoveries, uncertainty assessments. These assertions are cheap (one CLI call) and their cost is amortized by the value they provide later.

**At conversation end** (detectable by measuring actual context consumption), the agent **harvests**: it reviews what it learned, ensures all valuable observations are in the store, and the conversation ends. The harvest also detects what the agent *failed* to record — the gap between what the agent knew and what the store knows is a measurable drift metric.

**A new conversation begins with a seed**: the system assembles a compact, relevant summary from the store, tailored to what the next session will work on. The seed contains active invariants, unresolved uncertainties, recent decisions, recommended next actions — all generated from datoms, not from conversation fragments. The agent begins with full relevant knowledge, zero irrelevant noise, and a fresh attention budget.

The insight: **conversations are disposable; knowledge is durable.** Right now, people fight to keep conversations alive because losing them means losing knowledge. DDIS inverts this: knowledge lives in the store, so conversations become lightweight, disposable reasoning sessions. Start one, work for 30 turns, harvest, discard, start fresh. The agent never loses anything. The human never re-explains anything.

The store grows monotonically across conversations. Each conversation is a bounded trajectory — high-quality reasoning for a limited window before attention degrades. The trajectory produces durable knowledge (datoms) and ephemeral reasoning (conversation text). When the conversation ends, the ephemeral reasoning is released and the durable knowledge persists.

### Operational Parameters

The harvest/seed cycle follows a **20–30 turn lifecycle** (seven steps: orient → plan → execute → monitor → harvest → seed → handoff) calibrated to LLM attention degradation (LM-011). Harvest is **semi-automated**: the system detects harvestable knowledge and presents candidates for agent confirmation, with quality measured by false-positive/false-negative rates and a calibration drift metric (LM-005, LM-006). Observation staleness is tracked via freshness scores that decay per-namespace, enabling seed assembly to prioritize recent knowledge (UA-007). The system issues **proactive harvest warnings** at context consumption thresholds (70%, 85%, 95%) to prevent unharvested session termination (IB-012). Multi-agent harvest supports multiple **delegation topologies** — centralized, distributed, or hierarchical — recorded as datoms (LM-012). **Crystallization** of harvested knowledge has a **stability guard**: promoted datoms cannot finalize until a quorum period elapses with no new competing assertions (CR-005).

---

## 6. The Reconciliation Mechanisms

The system maintains coherence through specific mechanisms, each targeting a different type of divergence. These are not independent features — they are all instances of one fundamental operation: **detect divergence, classify it, resolve it back to coherence.**

**Harvest** closes the gap between agent knowledge and store knowledge. The agent discovered facts during a session; some were transacted, some weren't. Harvest detects and closes this epistemic gap.

**Associate and Assemble** prevent divergence from arising in the first place by surfacing relevant prior knowledge before the agent acts. An agent that implements a feature without knowing about a governing invariant will produce structurally divergent code — not from disagreement but from ignorance. Associate finds the relevant knowledge neighborhood; Assemble compresses it to fit the available attention budget.

**Guidance** steers agents toward actions that maintain coherence. Every tool response includes a brief pointer to the next methodologically correct action. Without guidance, agents drift into pretrained coding patterns within 15-20 turns. With guidance, each tool response reactivates the DDIS methodology.

**Merge** combines knowledge from independent agents and surfaces conflicts. The merge operation (set union) is mathematically clean, but it also reveals where agents disagree — conflicting values for the same entity-attribute pair become visible only after merge.

**Deliberation and Decision** provide structured resolution for conflicts that merge or contradiction detection surfaces. A deliberation states the competing positions; agents or humans argue them; a decision is reached and recorded with full rationale, so future agents can understand the reasoning.

**Signal** routes detected divergence to the appropriate resolution mechanism. Different signal types correspond to different divergence classes: confusion signals (epistemic), conflict signals (logical), drift signals (structural), goal-drift signals (axiological).

**Sync Barrier** establishes a consistent cut — a shared reference point where all participants agree on the same facts. This is the most expensive mechanism because it requires coordination, but it's necessary for decisions that depend on the absence of certain facts.

**The Bilateral Loop** continuously checks alignment between specification and implementation in both directions. Forward: does the implementation satisfy the spec? Backward: does the spec accurately describe the implementation? Convergence means no further changes in either direction.

**Dynamic CLAUDE.md** reconciles divergence between the methodology the agent should follow and the methodology it actually follows. The system tracks drift patterns (what agents forget, which checks they skip) and generates operating instructions that preemptively correct for observed failure patterns. The instructions improve with use as drift data accumulates.

### The Reconciliation Taxonomy

Every divergence the system detects falls into one of eight classes. Each has a characteristic boundary, detection mechanism, and resolution path:

| Divergence Type | Boundary | Detection | Resolution |
|----------------|----------|-----------|------------|
| Epistemic | Store vs. agent knowledge | Harvest gap detection | Harvest (promote to datoms) |
| Structural | Implementation vs. specification | Bilateral scan / drift | Associate + guided reimplementation |
| Consequential | Current state vs. future risk | Uncertainty tensor | Guidance (redirect before action) |
| Aleatory | Agent vs. agent | Merge conflict detection | Deliberation + Decision |
| Logical | Invariant vs. invariant | Contradiction detection (5-tier) | Deliberation + new ADR |
| Axiological | Implementation vs. goals | Fitness function / goal-drift signal | Human review + ADR revision |
| Temporal | Agent frontier vs. agent frontier | Frontier comparison | Sync barrier |
| Procedural | Agent behavior vs. methodology | Drift detection (access log) | Dynamic CLAUDE.md |

If a feature under development doesn't address at least one of these divergence types, either a ninth type has been discovered (document it) or the feature doesn't belong.

### Uncertainty, Authority, and Conflict

Uncertainty is modeled as a **three-dimensional tensor** (epistemic, aleatory, model), with independent detection and resolution per dimension (UA-001). Epistemic uncertainty exhibits **temporal decay** parameterized per attribute namespace (UA-002). **Authority** is computed via **spectral decomposition** (SVD) of the agent-attribute interaction matrix, yielding continuous scores rather than binary permissions (UA-003). Agents operate under one of four **delegation classes** — autonomous, supervised, collaborative, advisory — determining which operations require confirmation (UA-005). Resolution capacity is **monotonically non-decreasing**: once the system can resolve a class of divergence, it never loses that capability (UA-012). The query layer computes a **stability score** per result, quantifying sensitivity to future assertions (UA-009).

Conflict detection is **conservative** (no false negatives; CR-001), routing through **three tiers**: automated lattice resolution, structured deliberation, and escalated human decision (CR-002). Deliberation produces three entity types — Deliberation, Position, Decision — all stored as datoms with full provenance (CR-004). The system implements **dual-process architecture at the protocol level**: fast-path operations (query, assert, retract) execute without deliberation; slow-path operations (merge conflicts, contradiction resolution, schema changes) trigger structured resolution (AA-001). **Dynamic CLAUDE.md generation** uses a **three-way collapse**: static methodology + observed drift patterns + session-specific context → coherent operating instructions (GU-004). **Human-to-agent signal injection** provides a TUI mechanism for high-authority assertions (IB-009). **Test results are stored as datoms**, making outcomes queryable alongside the specifications they verify (CO-011). Four recognized **taxonomy gaps** in the reconciliation framework are tracked as open uncertainties pending resolution through practice (CO-007).

The goal is to drive every type of divergence toward zero — or toward an **explicitly acknowledged and documented residual.** When divergence can't be fully resolved (genuine tradeoffs exist, information is incomplete), the system records it, documents why it persists, and tracks what resolving it would require. Perfect coherence is the asymptote. Documented, tracked, understood residual divergence is the practical reality. The difference from every other approach is that DDIS makes residuals visible and queryable rather than hidden and discovered by accident.

---

## 7. The Self-Improvement Loop

Three things improve with use, distinguishing DDIS from "a database the agent writes to":

**The knowledge graph densifies.** Agents store not just facts but associations between facts. Over dozens of sessions, these edges create a navigable graph. The more the system is used, the richer the graph, the faster and more accurate the retrieval.

**The operating instructions adapt.** The system tracks methodology drift: how often agents forget to record observations, which types of work produce the most un-harvested discoveries. Before each session, it generates a dynamic CLAUDE.md — operating instructions that include targeted corrections for observed failure patterns. This is empirical prompt optimization running in production: the system writes its own operating instructions, and those instructions improve with use.

**The retrieval heuristics sharpen.** Frequently-queried datoms accumulate significance. When the agent asks "what's relevant to auth token refresh?" the system weights results by significance — facts that have been useful in the past surface first. Connections that are traversed together strengthen together. Significance accumulation uses a **separate access log** to prevent feedback loops where frequently-queried datoms gain significance merely from being queried (AS-007).

### Feedback Loop Architecture

The **basin competition model** (GU-006) identifies the central failure mode: pretrained coding patterns (Basin B) constantly compete with DDIS methodology (Basin A), and without continuous corrective force the agent drifts toward Basin B within 15–20 turns. Six **anti-drift mechanisms** — guidance injection, dynamic CLAUDE.md, harvest warnings, methodology scoring, significance weighting, and recency-biased footers — form an integrated architecture maintaining Basin A dominance (GU-007). The self-improvement loop includes four **metacognitive entity types** — observation, reflection, strategy, and evaluation — enabling the system to reason about its own reasoning patterns (AA-004).

---

## 8. The Interface Principles

The tools must respect the agent's finite attention. Every tool response enters the context window and competes for the same budget as the agent's own reasoning.

**Budget-aware output.** The attention budget is computed from the agent's actual context consumption (measured, not estimated). Early in a session, queries return rich detail. Late in a session, the same query returns a compressed summary. The compression is lossy but prioritized: the most relevant results survive, lower-priority details are dropped.

**Guidance injection.** Every tool response includes a brief, spec-language pointer to the next recommended action. This continuous seeding keeps the agent on-methodology without consuming significant budget.

**Five interface layers.** Ambient awareness (CLAUDE.md, always-present context), CLI (the primary agent interface), MCP (machine-to-machine protocol), Guidance (injected methodology steering), and TUI (human monitoring dashboard). Each layer serves a different consumer at a different frequency. A **Layer 4.5** (statusline bridge) provides persistent low-bandwidth signaling between TUI and CLI (IB-001).

### Budget and Output Architecture

The CLI attention budget is a **hard invariant** with five-level precedence (system > methodology > user-requested > speculative > ambient) — lower-priority output is truncated before higher-priority output is touched (IB-004). **k*** (remaining effective attention) is measured from actual token consumption via a formula Q(t) that accounts for recency bias and topic diversity (IB-005). Budget-aware output uses a **four-level projection pyramid**: at high k*, full detail; at medium k*, structured summaries; at low k*, compressed pointers; at critical k*, harvest-or-stop signals (SQ-007, IB-006). The runtime exposes **three CLI output modes** — demonstration (worked examples), constraint (invariants and rules), and diagnostic (drift and health) — selected by the guidance system based on agent state (IB-002). Every tool response includes a **four-part guidance footer**: next action, methodology reminder, recency-weighted context pointer, and harvest status (GU-005). **Intention anchoring** prevents goal dilution by requiring every operation to trace back to a declared intention entity (AA-005). The **bilateral query layer** structures queries in both directions: forward (does the implementation satisfy the spec?) and backward (does the spec accurately describe the implementation?), enabling automated bilateral verification at the query level (SQ-006).

---

## 9. What Exists Today

### 9.1 The Existing Codebase

There is an existing **Go** implementation (`../ddis-cli/`) of approximately **62,500 lines of code** across 36 internal packages, 45 CLI commands, and 12,000 lines of integration/behavioral tests. It is backed by a **39-table normalized SQLite schema** and a **three-stream JSONL event-sourcing pipeline**. The implementation is specified by a companion DDIS specification (`../ddis-cli-spec/`) containing **112 invariants**, **82 ADRs**, and a **9-module constitution** across 10 domains. The meta-standard that governs the specification format itself lives in `../ddis-modular/` (23 invariants, 14 ADRs, 7 quality gates). The specification reached a verified fixpoint (F(S) = 1.0, 0 drift, 100% coverage) on 2026-02-27. A subsequent 5-agent cleanroom audit (2026-03-01) surfaced 4 HIGH, 28 MEDIUM, and 20+ LOW severity findings, with a remediation plan classifying them into implementation fixes and 8 new invariants.

### 9.2 Gap Analysis

> **Canonical reference**: `docs/design/GAP_ANALYSIS.md` contains the full module-by-module analysis (920 lines, 10 sections, 36 modules classified). What follows is a summary of the central findings and their implications for Braid. For per-module details, reusability assessments, stage impact analysis, and the complete ALIGNED/DIVERGENT/EXTRA/BROKEN/MISSING breakdown, see `docs/design/GAP_ANALYSIS.md`.

The gap analysis classifies every existing module and capability against the Braid specification established in sections 1–8 of this document:

| Category | Count | Summary |
|----------|-------|---------|
| **ALIGNED** | 8 modules | Concepts proven correct; logic portable to new substrate (contradiction detection, guidance injection, fitness function, witness/challenge, validation, search, annotation, parser) |
| **DIVERGENT** | 12 modules | Right concept, wrong substrate — the information is what Braid needs but the storage model (relational SQL vs. EAV datoms) is fundamentally different |
| **EXTRA** | 6 modules | Useful features the spec doesn't address; evaluated for inclusion at appropriate stages |
| **BROKEN** | 4 items | Known bugs from cleanroom audit (identity collapse, dead provenance, parser code-block blindness, non-atomic dual-write) — each traces to an architectural weakness the datom store eliminates by construction |
| **MISSING** | 15 capabilities | Required by SEED.md or settled design decisions, not implemented in any form |

#### The Central Finding: Substrate Divergence

The Go CLI proves that DDIS *concepts* work at scale — it achieved F(S) = 1.0 fixpoint convergence. But it was built on a **relational substrate** (39-table normalized SQLite, three-stream JSONL events, global LWW merge) whereas SEED.md specifies a **datom substrate** (`[e, a, v, tx, op]` tuples, content-addressable identity, schema-as-data, per-attribute resolution, Datalog queries, set-union merge). This divergence pervades every layer: identity (sequential IDs vs. content-addressed), mutability (UPDATE/DELETE vs. append-only), schema (hardcoded DDL vs. schema-as-data), queries (SQL vs. Datalog), and merge (event-level LWW vs. datom set union).

#### Critical Missing Capabilities (Stage 0 Blockers)

Five capabilities required by SEED.md have no implementation in any form and block Stage 0:

1. **Datom store** (§4, C1–C4) — the fundamental data structure. Everything depends on this.
2. **Datalog query engine** (§4, Axiom 4) — the query layer all access flows through.
3. **Harvest operation** (§5) — end-of-session knowledge extraction.
4. **Seed operation** (§5) — start-of-session knowledge assembly.
5. **Dynamic CLAUDE.md generation** (§6, §7) — adaptive operating instructions from drift patterns.

Ten additional capabilities (agent frontiers, sync barriers, signal system, deliberation records, MCP interface, TUI, significance accumulation, schema-as-data, per-attribute resolution, agent working set / patch branches) are required at Stages 1–4. See `docs/design/GAP_ANALYSIS.md` §4 for the complete list with staging.

#### Key Empirical Finding: Bilateral Lifecycle Non-Adoption

The most significant finding is systemic: across two external projects, the bilateral lifecycle (discover → crystallize → absorb) has **0% adoption**, while the structural layer (parse, validate, coverage, drift) generalizes at **100%**. The root cause is information-theoretic: within a session, the agent's context window is strictly fresher than the tool's state, making tool consultation redundant. This empirically validates the harvest/seed architecture (§5) — knowledge injection at session boundaries rather than within sessions avoids the ceremonial usage problem entirely.

### 9.3 Summary: The Bridge to Implementation

The gap analysis reveals a clear architectural boundary: the Go CLI has **strong behavioral coverage** (bilateral loop, contradiction detection, guidance injection, fitness convergence, witness/challenge adjunction) built on a **fundamentally different substrate** (relational SQL, three-stream JSONL, causal DAG fold) than what Braid requires (datom store, Datalog queries, set-union merge).

The implementation strategy follows from this observation:

1. **Build the substrate first** (Stage 0): datom store, Datalog queries, schema-as-data, per-attribute resolution. Everything else depends on this.
2. **Port the behavioral concepts** (Stage 0–1): harvest/seed, guidance injection, dynamic CLAUDE.md, bilateral loop — reimplemented as operations over the datom store rather than SQL + JSONL.
3. **Reimplement ALIGNED modules as Datalog queries** (Stage 0–1): contradiction detection, validation, search, coverage, fitness function. The logic transfers; the query substrate changes.
4. **Defer EXTRA modules** to the stage where they become relevant. Most become trivial once the datom store and Datalog engine exist.
5. **Learn from BROKEN findings**: every defect in the Go CLI traces to an architectural weakness that the datom store eliminates by construction (identity by content, append-only immutability, single canonical store, EAV coverage).

The critical risk is the datom store and Datalog engine. These are the load-bearing novelties — everything else is a reimplementation of proven concepts on a new substrate. If the datom store works, the rest follows. If it doesn't, nothing else matters.

---

## 10. Where We're Going: The Bootstrap

### The Bootstrap Principle

DDIS specifies itself. The specification of the datom store is written using DDIS methodology. When the datom store exists, its specification migrates into it — each invariant becomes a set of datoms, each ADR becomes an entity with traceable attributes, each negative case becomes a queryable constraint. The specification process generates the first real dataset for the system it specifies.

This means the specification is not a traditional document. It is a structured collection of DDIS elements expressed in markdown (because the datom store doesn't exist yet), organized so that migration to the store is mechanical rather than interpretive. Every element has:

- **An ID** (e.g., INV-STORE-001, ADR-EAV-001, NEG-MUTATION-001)
- **A type** (invariant, ADR, negative case, section, goal, uncertainty)
- **Traceability** to the goals in this seed document
- **A falsification condition** (for invariants and negative cases)
- **Explicit uncertainty** where the spec isn't sure yet, stated as a confidence level rather than aspirational prose that reads like certainty

When an agent in a Claude Code session produces a specification element, it follows this structure. When the datom store comes online in Stage 0, these elements are the first transacted facts. The system's first act of coherence verification is checking its own specification for contradictions.

### The Staging Model

Implementation proceeds in stages. Every stage must leave the codebase better than it found it, deliver usable value, and be excellent within its scope.

**Stage 0: Harvest/Seed Cycle.** Validate the core hypothesis: harvest/seed transforms the workflow from "fight context loss" to "ride context waves." Deliverables: transact, query, status, harvest, seed, guidance, dynamic CLAUDE.md generation. Success criterion: work 25 turns, harvest, start fresh with seed — new session picks up without manual re-explanation. First act: migrate the DDIS-structured specification elements into the store as datoms.

**Stage 1: Budget-Aware Output + Guidance Injection.** Context-aware tools with graceful degradation. Every tool output adapts to remaining attention budget. Guidance injection measurably reduces drift.

**Stage 2: Branching + Deliberation.** Structured exploration of alternatives. Branch isolation, competing proposals, deliberation records queryable as precedent.

**Stage 3: Multi-Agent Coordination.** CRDT merge, frontier tracking, sync barriers, signal system. Two agents work on separate branches, merge cleanly, resolve conflicts through deliberation.

**Stage 4: Advanced Intelligence.** Self-authored associations, significance-weighted retrieval, spectral authority, learned guidance, full TUI.

### Bootstrap Specifics

**Every command is a store transaction** — there is no read-only path that bypasses the store; even queries produce transaction records for provenance (FD-012). The **genesis transaction** (tx=0) installs the 17 axiomatic meta-schema attributes, bootstrapping the schema from nothing (PO-012). **Branch operations** support six sub-operations (create, switch, merge, compare, prune, lock) with a competing-branch lock preventing concurrent modifications to the same branch (PO-007). The agent lifecycle is a **ten-step composition**: orient → associate → assemble → plan → execute → observe → harvest → seed → reflect → handoff (PO-011). After Stage 0, the system uses a **DDR (DDIS Decision Record) feedback loop**: every spec gap revealed by practice becomes a DDR, which updates the specification, which updates the implementation, in a continuous convergence cycle (LM-014).

### Concrete Next Steps

1. **Edit this seed** (by hand). Fix anything that doesn't sound right. Fill in the codebase description in section 9. The test: if Claude Code read only this document, would it understand what to build and why?

2. **Produce the DDIS-structured specification** (Claude Code, 2-4 hours across multiple sessions). Feed Claude Code this seed plus the conversation transcripts. The instruction: produce a specification using DDIS methodology — invariants with IDs and falsification conditions, ADRs with alternatives and rationale, negative cases, explicit uncertainty markers. Every element individually addressable, traceable to the seed, structured for eventual migration into the datom store.

3. **Produce the implementation guide** (Claude Code, companion to the specification). Stage 0-4 deliverables, CLI command specs with examples, CLAUDE.md template, file formats, success criteria. Written for the agent that will build the system.

4. **Gap analysis** (fresh Claude Code session). Categorize every existing module as ALIGNED, DIVERGENT, EXTRA, BROKEN, or MISSING relative to the specification.

5. **Triage** (by hand). Mark modules GREEN/YELLOW/RED/GREY. Scope Stage 0 precisely.

6. **Implement Stage 0** (Claude Code, 1-2 weeks). First act: transact the specification elements as datoms. Use the system to build the system from the first hour.

7. **Validate and feedback.** Use Stage 0 for real work. Record DDRs (DDIS Decision Records) for every spec gap revealed by practice. Update the specification. Re-triage for Stage 1.

The critical discipline: **practice the methodology before the tools exist.** Harvest manually between sessions — write down key decisions and carry them forward. Perform the harvest/seed lifecycle by hand. When the tools arrive, they automate a practice already established. Methodology precedes tooling. Tools encode a way of working that you've already validated.

### The Failure Mode Registry

`docs/design/FAILURE_MODES.md` is the live catalog of failure modes observed when using AI agents for complex, long-running work during the DDIS/Braid bootstrap. Each entry documents a real failure — knowledge lost across sessions, provenance fabricated, analysis scope anchored incorrectly — and maps it to the DDIS/Braid mechanism designed to prevent or detect that failure class. The entries serve as **test cases and acceptance criteria** for evaluating whether the methodology works: does the harvest mechanism catch ≥99% of dropped decisions? Does the provenance lattice eliminate fabricated attributions? Does the single-substrate store eliminate anchoring bias?

When the datom store exists, each failure mode becomes a datom and `docs/design/FAILURE_MODES.md` becomes a projection. Until then, it is the manual catalog. Agents must check it at session start (to avoid repeating known failure patterns) and record new failure modes immediately when discovered. The failure mode lifecycle (OBSERVED → MAPPED → TESTABLE → VERIFIED) tracks whether the methodology addresses the failure class, not whether a human has manually patched around it.

---

## 11. The Design Rationale

**Why append-only?** Mutable state is the root of most correctness bugs in distributed systems. Append-only plus git gives you arbitrary time-travel for free. Truncation to a known-good state is always possible.

**Why EAV instead of relational?** The ontology of a project evolves as the project evolves. EAV handles schema evolution without migrations. The schema crystallizes from usage rather than being declared upfront.

**Why Datalog for queries?** Datalog's join semantics naturally express the graph queries needed for traceability (trace from goal → invariant → implementation → test). Its stratified evaluation maps cleanly to the monotonic/non-monotonic distinction.

**Why not just use a vector database / RAG?** Vector similarity retrieval finds "related" content. It does not verify logical coherence, detect contradictions, or trace causal dependencies. It is a retrieval heuristic, not a verification substrate. DDIS needs both retrieval and verification; the datom store provides both.

**Why per-attribute conflict resolution instead of global policy?** Different attributes have different semantics. Task status has a natural lattice (todo < in-progress < done). Person names do not. Forcing one resolution policy on all attributes either loses information or produces nonsense.

**Why formalize at all?** Because "it seems to work" is not the same as "we can prove it's correct." For a system where the central promise is verifiable coherence, the system's own coherence must be verifiable. The formalism (invariants, axioms, falsification conditions) is not academic decoration — it is the mechanism by which the system's promises are kept.

**Why does DDIS specify itself?** Three reasons. First, integrity: a coherence verification system specified using methods that can't verify coherence would be self-undermining at the moment of its articulation. Second, bootstrapping: the specification elements (invariants, ADRs, negative cases) become the first dataset for the system, so Stage 0 has real data to work with on day one. Third, validation: if DDIS methodology can't handle the complexity of specifying DDIS itself, it can't handle anything else either. The specification process is the first stress test.

---

*This document is a seed, not a spec. The formal specification will derive from this document plus the full conversation transcripts, written using DDIS methodology (invariants, ADRs, negative cases, explicit uncertainty) expressed in markdown and structured for eventual migration into the datom store. The specification is both the plan for the system and the first dataset the system will manage. Everything grows from here.*

