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

The DDIS specification formalism provides specific machinery for detecting and resolving each type of divergence. Each element addresses a different failure mode:

**Invariants** are falsifiable claims about the system. Not wishes, not goals — statements that can be mechanically checked. Each invariant has an ID, a formal statement, an explicit violation condition ("this invariant is falsified if..."), and a verification method. Invariants are the primary mechanism for detecting **logical** and **structural** divergence.

**Architectural Decision Records (ADRs)** capture why a choice was made, what alternatives were considered, and what tradeoffs were accepted. ADRs prevent the specific failure mode where someone revisits a decision without knowing why it was made, reverses it for seemingly good reasons, and breaks downstream invariants that depended on the original choice. ADRs are the primary mechanism for detecting **axiological divergence** — when the implementation undermines the goals that motivated the design.

**Negative cases** define what the system must NOT do. They bound the solution space. Without explicit negative cases, an agent optimizing for one invariant may violate an unstated constraint. Negative cases prevent **structural divergence** arising from overspecification in one dimension and underspecification in another.

**The bilateral feedback loop** continuously checks alignment in both directions. Forward: does the implementation satisfy the specification? Backward: does the specification accurately describe the implementation? The loop converges when the specification fully describes the implementation and the implementation fully satisfies the specification.

**Contradiction detection** operates in tiers of increasing subtlety: exact (identical claims with different values), logical (mutually exclusive implications), semantic (different words, same conflict), pragmatic (compatible in isolation, incompatible in practice), and axiological (internally consistent but misaligned with goals). These tiers surface divergence that simpler checks would miss.

**The fitness function** quantifies convergence toward full coherence across multiple dimensions: coverage (every goal traces to invariants and back), depth (invariants are fully specified with violation conditions), coherence (no internal contradictions), completeness (no gaps between spec and implementation), and formality (claims are falsifiable, not aspirational).

**Explicit uncertainty markers** acknowledge what the specification doesn't know yet. Where a claim is provisional, it carries a confidence level and a description of what would resolve the uncertainty. This prevents the failure mode where aspirational prose reads like settled commitment, and agents implement uncertain claims as though they were axioms. Uncertainty is not a defect in the specification — it is information about where the specification needs more work.

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

---

## 5. The Harvest/Seed Lifecycle

The harvest/seed cycle is the mechanism by which knowledge survives conversation boundaries. This is the make-or-break feature.

**During a conversation**, the agent works normally — reading code, making decisions, discovering dependencies. Alongside this work, it asserts datoms: observations, decisions, dependency discoveries, uncertainty assessments. These assertions are cheap (one CLI call) and their cost is amortized by the value they provide later.

**At conversation end** (detectable by measuring actual context consumption), the agent **harvests**: it reviews what it learned, ensures all valuable observations are in the store, and the conversation ends. The harvest also detects what the agent *failed* to record — the gap between what the agent knew and what the store knows is a measurable drift metric.

**A new conversation begins with a seed**: the system assembles a compact, relevant summary from the store, tailored to what the next session will work on. The seed contains active invariants, unresolved uncertainties, recent decisions, recommended next actions — all generated from datoms, not from conversation fragments. The agent begins with full relevant knowledge, zero irrelevant noise, and a fresh attention budget.

The insight: **conversations are disposable; knowledge is durable.** Right now, people fight to keep conversations alive because losing them means losing knowledge. DDIS inverts this: knowledge lives in the store, so conversations become lightweight, disposable reasoning sessions. Start one, work for 30 turns, harvest, discard, start fresh. The agent never loses anything. The human never re-explains anything.

The store grows monotonically across conversations. Each conversation is a bounded trajectory — high-quality reasoning for a limited window before attention degrades. The trajectory produces durable knowledge (datoms) and ephemeral reasoning (conversation text). When the conversation ends, the ephemeral reasoning is released and the durable knowledge persists.

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

The goal is to drive every type of divergence toward zero — or toward an **explicitly acknowledged and documented residual.** When divergence can't be fully resolved (genuine tradeoffs exist, information is incomplete), the system records it, documents why it persists, and tracks what resolving it would require. Perfect coherence is the asymptote. Documented, tracked, understood residual divergence is the practical reality. The difference from every other approach is that DDIS makes residuals visible and queryable rather than hidden and discovered by accident.

---

## 7. The Self-Improvement Loop

Three things improve with use, distinguishing DDIS from "a database the agent writes to":

**The knowledge graph densifies.** Agents store not just facts but associations between facts. Over dozens of sessions, these edges create a navigable graph. The more the system is used, the richer the graph, the faster and more accurate the retrieval.

**The operating instructions adapt.** The system tracks methodology drift: how often agents forget to record observations, which types of work produce the most un-harvested discoveries. Before each session, it generates a dynamic CLAUDE.md — operating instructions that include targeted corrections for observed failure patterns. This is empirical prompt optimization running in production: the system writes its own operating instructions, and those instructions improve with use.

**The retrieval heuristics sharpen.** Frequently-queried datoms accumulate significance. When the agent asks "what's relevant to auth token refresh?" the system weights results by significance — facts that have been useful in the past surface first. Connections that are traversed together strengthen together.

---

## 8. The Interface Principles

The tools must respect the agent's finite attention. Every tool response enters the context window and competes for the same budget as the agent's own reasoning.

**Budget-aware output.** The attention budget is computed from the agent's actual context consumption (measured, not estimated). Early in a session, queries return rich detail. Late in a session, the same query returns a compressed summary. The compression is lossy but prioritized: the most relevant results survive, lower-priority details are dropped.

**Guidance injection.** Every tool response includes a brief, spec-language pointer to the next recommended action. This continuous seeding keeps the agent on-methodology without consuming significant budget.

**Five interface layers.** Ambient awareness (CLAUDE.md, always-present context), CLI (the primary agent interface), MCP (machine-to-machine protocol), Guidance (injected methodology steering), and TUI (human monitoring dashboard). Each layer serves a different consumer at a different frequency.

---

## 9. What Exists Today

There is an existing Rust implementation of approximately 60,000 lines of code. Before building further, a gap analysis must determine which modules align with this specification, which diverge, and which are missing. The categories are:

- **ALIGNED**: implements the spec correctly — keep, minor fixes
- **DIVERGENT**: implements differently than the spec — adapter or surgical edit
- **EXTRA**: implements what the spec doesn't address — evaluate: extend spec or remove
- **BROKEN**: doesn't work — fix or rewrite
- **MISSING**: spec requires but doesn't exist — build

This gap analysis is the bridge between the existing codebase and the staged implementation plan.

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

### Concrete Next Steps

1. **Edit this seed** (by hand). Fix anything that doesn't sound right. Fill in the codebase description in section 9. The test: if Claude Code read only this document, would it understand what to build and why?

2. **Produce the DDIS-structured specification** (Claude Code, 2-4 hours across multiple sessions). Feed Claude Code this seed plus the conversation transcripts. The instruction: produce a specification using DDIS methodology — invariants with IDs and falsification conditions, ADRs with alternatives and rationale, negative cases, explicit uncertainty markers. Every element individually addressable, traceable to the seed, structured for eventual migration into the datom store.

3. **Produce the implementation guide** (Claude Code, companion to the specification). Stage 0-4 deliverables, CLI command specs with examples, CLAUDE.md template, file formats, success criteria. Written for the agent that will build the system.

4. **Gap analysis** (fresh Claude Code session). Categorize every existing module as ALIGNED, DIVERGENT, EXTRA, BROKEN, or MISSING relative to the specification.

5. **Triage** (by hand). Mark modules GREEN/YELLOW/RED/GREY. Scope Stage 0 precisely.

6. **Implement Stage 0** (Claude Code, 1-2 weeks). First act: transact the specification elements as datoms. Use the system to build the system from the first hour.

7. **Validate and feedback.** Use Stage 0 for real work. Record DDRs (DDIS Decision Records) for every spec gap revealed by practice. Update the specification. Re-triage for Stage 1.

The critical discipline: **practice the methodology before the tools exist.** Harvest manually between sessions — write down key decisions and carry them forward. Perform the harvest/seed lifecycle by hand. When the tools arrive, they automate a practice already established. Methodology precedes tooling. Tools encode a way of working that you've already validated.

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

