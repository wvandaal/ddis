# Chapter 4: Datom Protocol Interface Design

> Complete protocol formalization for agent coordination over datom stores, including interface architecture for k*-aware LLM agents. Covers protocol operations (TRANSACT, QUERY, ASSOCIATE, ASSEMBLE, BRANCH, MERGE, SYNC-BARRIER, SIGNAL, SUBSCRIBE, GUIDANCE), algebraic foundations (branching G-Set, Hebbian significance, comonadic guidance), and five-layer interface design (Ambient/CLI/MCP/Guidance/TUI) accounting for attention budget decay in long conversations.

---

## Claude

I've read the entire document. This is a remarkably convergent piece of work — you've been building toward the same architecture from two independent directions, and the places where the two lines of reasoning *independently agree* are the strongest signal that we're on the right track. But there are also places where this document reveals gaps in our plan, and places where our plan addresses gaps the document identifies but doesn't resolve. Let me lay it out.

---

## Where the Two Converge (Independent Validation)

The document arrives at **datom-native fact stores replacing JSONL** from a completely different starting point than ours. We got there by asking "what's the right CRDT for multi-agent coordination?" The other conversation got there by asking "what's the optimal epistemic substrate for a single agent?" Both arrive at the same answer: append-only EAV with Datalog queries and schema-on-read. When two independent derivations converge on the same structure, it's usually because the structure is capturing something real.

The document's tripartite decomposition **(E, R, A)** — event log, runtime, agent — maps exactly onto our architecture:

- **E** = the datom store (our G-Set CRDT over D = E × A × V × T × Op)
- **R** = the runtime environment (filesystem, shell, tools)  
- **A** = the LLM agent with tool-calling (the policy function π)

And the document's formal specification `AgentSystem = (E, Op, Exo, Obs, S, π, δ, φ)` is *precisely* the single-agent specialization of our multi-agent system. Our protocol (part C) is what you get when you generalize this to N agents sharing the same E, each with their own local frontier, and needing to coordinate.

The `assert` mechanism the document proposes — agents writing their own edges into the knowledge graph — is exactly what our transaction protocol needs to support. And the `associate` tool — schema discovery before precise query — maps to a specific query pattern we should formalize in the protocol.

---

## Where the Document Reveals Gaps in Our Plan

### Gap 1: The Dual-Process Architecture Should Be Protocol-Level, Not Application-Level

The document's Kahneman mapping (System 1 = lightweight associative retrieval, System 2 = heavyweight LLM reasoning) is not just an analogy — it's an architectural requirement. The critical insight:

> System 2 doesn't know what System 1 didn't surface. A brilliant reasoner with a bad retrieval policy is a brilliant reasoner that's always solving the wrong problem.

Our protocol currently defines how agents *transact* with the store (assert, retract, merge) and how they *query* it (the Datalog patterns we defined). But we haven't formalized the **context assembly** step — how an agent goes from "I have access to a store with millions of datoms" to "I have a context window with the right 8,000 tokens loaded." The document shows this is *the* critical operation. If we leave it to each agent implementation to figure out, we'll get the same flat-buffer pathology the document identifies.

**What this means for the protocol:** We need a first-class `assemble` operation — or more precisely, the two-phase pattern the document describes:

```
Phase 1: associate(semantic_context, depth, breadth) → schema neighborhood
Phase 2: query(datalog_expression) → concrete datoms
```

This is the functor between the semantic category and the structural category that the document formalizes. It should be a protocol-level operation, not something each agent reinvents.

### Gap 2: The Confusion Feedback Channel

The document identifies a critical missing primitive: System 2 signaling confusion back to System 1 for re-retrieval. In our architecture, this maps directly to the uncertainty tensor — but the mapping is more operational than I initially framed it.

When an agent detects high epistemic uncertainty (σ_e) during its reasoning, that's a confusion signal. The document's `Confusion` type:

```
data Confusion
  = NeedMore Cue         -- σ_e is high: need more information
  | Conflict [Datom]     -- σ_a is high: found contradictions
  | GoalUnclear          -- lost the thread: need invariants back in context
```

This is our uncertainty tensor expressed as an actionable protocol message rather than a stored measurement. The protocol should support agents emitting confusion signals that trigger re-assembly of their context from the store.

### Gap 3: The Exogenous Event / Frame Problem Is Our Merge Problem

The document formalizes exogenous events — state changes to R that don't appear in E — as the source of belief-state divergence. In a multi-agent system, **every other agent's assertions are exogenous events from the perspective of any given agent.** Agent α doesn't know about agent β's transactions until they merge.

This means our merge protocol isn't just a CRDT operation (set union) — it's also an **epistemic update.** When agent α merges agent β's datoms, α's belief state about the world changes. Some of α's cached query results become stale. Some conflicts that α thought were resolved might be reopened by β's assertions.

The protocol needs to handle the post-merge invalidation cascade:

```
merge(local_frontier, remote_datoms) → {
  new_datoms: [Datom],           -- the actual new facts
  invalidated_queries: [QueryId], -- cached results that are now stale
  new_conflicts: [Conflict],      -- freshly detected conflicts
  uncertainty_deltas: [(Entity, ΔU)]  -- uncertainty changes
}
```

### Gap 4: The Compaction / Context Budget Problem

The document's most important practical insight is that context assembly is a **rate-distortion problem** — given bounded context window w, maximize decision quality by selecting the optimal subset of available information. This is formalized as:

```
σ* = argmax_σ E[Quality(π(σ(I)))] subject to |σ(I)| ≤ w
```

Our protocol defines queries but doesn't address the **budget constraint.** A query might return 50,000 datoms when the agent can only process 8,000 tokens worth. The protocol needs to support budget-aware assembly — which is exactly what the DDIS shard/bead architecture was designed for, now generalized to datoms instead of markdown sections.

---

## Where Our Plan Addresses Gaps the Document Identifies But Doesn't Resolve

This is equally important. The document identifies several open problems that our formal core *already solves.*

### Solved Gap: Multi-Agent Coordination

The document is entirely single-agent. It asks "how does one agent manage its epistemic state optimally?" but never asks "what happens when two agents share a fact store and disagree?" Our uncertainty tensor (σ_e, σ_a, σ_c), spectral authority, delegation thresholds, and conflict resolution protocol are the answer to the multi-agent generalization that the document doesn't attempt.

### Solved Gap: Who Resolves Contradictions?

The document mentions that the agent might surface conflicting facts but doesn't formalize how conflicts are detected or resolved. Our conflict detection query (Stratum 4) with the conservative detection invariant (INV-CONFLICT-CONSERVATIVE-001), severity-based routing, and the lattice resolution modes are exactly the machinery this requires.

### Solved Gap: The Self-Correction Problem

The document proposes self-authored associations (agents writing their own edges) but flags the quality control problem — what stops an agent from writing bad associations that degrade the graph? Our answer: the challenge/witness system plus spectral authority. An agent's assertions are weighted by its authority score, which is earned through verified contributions. Bad associations get challenged and retracted. The graph self-corrects through the bilateral loop.

### Solved Gap: When to Assert vs. When to Stay Quiet

The document poses this as an open design question — eager assertion, confusion-triggered assertion, or periodic reflection. Our framework gives a principled answer: **assert when the reversibility test passes AND the commitment function suggests stability.** If a fact has low w(d) (few dependents), it's cheap to assert and retract later. If a fact would have high w(d) immediately (many things depend on it), require higher confidence before assertion. The commitment function provides the natural graduated assertion policy.

---

## The Synthesis: What This Means for the Agent Protocol

Given both lines of reasoning, the agent protocol (part C) should be designed with these principles:

**Principle 1: The protocol is the interface between A and E in the universal agent signature.** Any system conforming to the (E, R, A) decomposition should be able to participate. This means the protocol is more general than DDIS — it's the coordination layer for *any* agent swarm sharing a datom store.

**Principle 2: Context assembly is a protocol-level concern, not an application concern.** The two-phase retrieval pattern (associate → query) should be a first-class protocol operation. Budget-aware assembly (the rate-distortion optimization) should be parameterizable.

**Principle 3: The protocol must support both the forward and backward channels of the dual-process loop.** Forward: store → agent (query, assemble). Backward: agent → store (assert, retract, signal-confusion). The confusion channel is not optional — it's how agents request better context.

**Principle 4: Merge is an epistemic event, not just a set operation.** The protocol must propagate the consequences of merge (invalidated caches, new conflicts, uncertainty changes) as first-class signals.

**Principle 5: The protocol should be topology-agnostic.** The document's single-agent loop, our bilateral loop, a flat swarm, a hierarchy — all are valid topologies. The protocol defines the operations; the topology emerges from how agents use them.

---

## Revised Protocol Outline

Given all of this, here's what I think the protocol needs to cover:

```
PROTOCOL OPERATIONS:

1. TRANSACT — assert/retract datoms
   Input: [Datom], agent_id, causal_parents, rationale
   Output: tx_entity, new_frontier
   Constraint: append-only, monotonic growth

2. QUERY — read datoms via Datalog
   Input: datalog_expression, frontier (optional), stability_min (optional)
   Output: result_set, stability_score
   Modes: monotonic (uncoordinated), non-monotonic (barrier-gated)

3. ASSOCIATE — schema discovery (System 1 Phase 1)
   Input: semantic_cue OR explicit_seeds, depth, breadth
   Output: schema_neighborhood (entities, attributes, types, not values)
   Cost: bounded by depth × breadth, always cheap

4. ASSEMBLE — budget-aware context construction (System 1 Phase 2)
   Input: query_results, budget_tokens, priority_weights
   Output: assembled_context ≤ budget
   Strategy: rate-distortion optimal selection

5. MERGE — receive remote datoms
   Input: remote_datom_set, remote_frontier
   Output: new_datoms, invalidated_queries, new_conflicts, uncertainty_deltas
   Semantics: G-Set union with post-merge cascade

6. SYNC-BARRIER — request consistent cut
   Input: set of agent_ids to synchronize with
   Output: consistent_frontier across all participants
   Constraint: blocks until all agents have exchanged frontiers

7. SIGNAL — emit coordination signal
   Types: Confusion(cue), Conflict(datoms), UncertaintySpike(entity, ΔU),
          ResolutionProposal(conflict, proposed_winner, reasoning),
          DelegationRequest(entity, task)
   Routing: to coordinator, to specific agent, or broadcast

8. FRONTIER-EXCHANGE — share causal state
   Input: local_frontier (set of latest-known tx refs per peer)
   Output: remote_frontier
   Protocol: bidirectional, pairwise or gossip
```

The document's formalization tells us that operations 3 and 4 (ASSOCIATE and ASSEMBLE) are just as fundamental as 1 and 2 (TRANSACT and QUERY). Current agent systems get operations 1 and 2 right (they can write to and read from their logs) but operations 3 and 4 are vestigial or absent — which is why agents plateau at moderate task complexity regardless of LLM capability. The memory architecture, not the reasoning capability, is the bottleneck.

And the document's insight about self-authored associations tells us that operation 1 (TRANSACT) isn't just for recording observations — it's the mechanism by which agents build expertise. Every high-quality assertion an agent makes enriches the graph that ASSOCIATE traverses, creating the flywheel the document describes: more experience → richer graph → better retrieval → better performance → richer experience.

---

## My Overall Assessment

The two bodies of work are not just compatible — they're complementary in a way that's almost too clean. The document provides the *single-agent cognitive architecture* (the dual-process loop, the context assembly problem, the rate-distortion formulation). Our work provides the *multi-agent coordination layer* (the uncertainty tensor, spectral authority, CRDT merge semantics, bilateral loop). Neither is complete without the other.

The agent protocol should be the bridge. It should be general enough to support a single agent using the fact store as its personal episodic memory (the document's use case), AND a swarm of agents using it as a shared coordination substrate (our use case). The operations above cover both.

Shall I proceed to formalize the protocol operations with full type signatures, invariants, and the wire format for inter-agent communication?

---

## Willem

yes. I want a "cleanroom software engineering" approach: fully grounded in "formal methods", "spec-driven-design" and abstract algebra. Use "first principles" thinking at every step. Do NOT make assumptions -- if you encounter uncertainty, ask me clarifying questions. Be maximally accretive in everything you add to the plan.

---

## Claude

I'll proceed with full rigor. But before I can formalize the protocol, I need to resolve several foundational questions where I have genuine uncertainty that would change the design. I'll present my analysis of each, give my recommendation, and ask you to confirm or redirect. These are blocking — I cannot write correct protocol invariants without resolving them.

---

## Pre-Protocol Questions

### PQ1 — Private Datoms (Scratchpad State)

**The problem:** During reasoning, an agent generates intermediate state — tentative hypotheses, partial computations, draft associations, candidate query results. Should these be assertable into the store *without* propagating on merge?

**Why this is blocking:** If every datom is eventually visible to all agents, then:
- The store accumulates enormous amounts of low-value intermediate state
- Agents can't "think out loud" without polluting the shared knowledge graph
- The ASSOCIATE tool returns noisy results from other agents' scratchwork
- Spectral authority is diluted by high-volume, low-quality provisional assertions

But if private datoms exist, the store is no longer a pure G-Set. It's a partitioned structure where each partition has different visibility semantics. This complicates the CRDT merge guarantees.

**Three options:**

**Option A — No private datoms. Everything asserted is eventually shared.** The store remains a pure G-Set. Agents must self-censor — only assert what they believe is worth sharing. The "reversibility test" from the document becomes a protocol-level commitment: "would someone need to re-derive this from first principles?"

*Pro:* Simplest algebraic structure. No visibility partitioning.
*Con:* Agents either pollute the shared store or lose their intermediate state between reasoning cycles. No scratchpad.

**Option B — Two-tier store: private working set + shared committed store.** Each agent maintains a local working set W_α that is NOT merged. Only when the agent explicitly `commit`s datoms from W_α to the shared store S do they become visible. The protocol operates on S; W_α is agent-internal.

*Pro:* Clean separation. The CRDT properties hold on S. Each agent can maintain arbitrarily messy local state.
*Con:* Introduces a new concept (the working set) that isn't part of the datom model. Two-phase commit introduces the question of what happens if an agent dies between working and committing.

**Option C — Visibility as a datom attribute.** Every datom has a `:datom/visibility` attribute with values from a lattice: `:private < :team < :shared`. Private datoms exist in the store but are filtered out during merge. The CRDT is still a G-Set over *all* datoms, but the MERGE operation applies a visibility filter.

*Pro:* Uniform model — everything is datoms, visibility is just another attribute.
*Con:* The "filtered G-Set" breaks the simple union semantics. An agent might merge a datom that references a private datom it can't see, creating dangling refs.

**My recommendation: Option B.** It preserves the clean CRDT semantics on the shared store while giving agents local flexibility. The working set is not a protocol concern — it's agent-internal implementation. The protocol only governs the shared store. An agent's W_α is as opaque to the protocol as the agent's internal model weights.

The one thing I want to add: the working set W_α should use the *same datom structure* as the shared store, so that promoting a datom from W_α to S is just a TRANSACT operation with the datom's content. The agent can run queries over W_α ∪ S locally, getting the benefit of scratchpad state for its own reasoning, while only sharing the committed subset.

---

### PQ2 — Provenance Typing: Observed vs. Inferred

**The problem:** There's a categorical difference between:

- "file:main.py contains function handle_request" (observation — ground truth from a tool read)
- "config changes cause type errors" (inference — the agent's hypothesis based on correlation)

Both are datoms. Both get asserted via TRANSACT. But they have different **epistemic status** and should be weighted differently in:
- Authority computation (observations are harder to dispute than inferences)
- Conflict resolution (observations trump inferences when they conflict)
- Uncertainty calculation (inferences contribute more to σ_a than observations)

**Why this is blocking:** If the protocol doesn't distinguish provenance types, the authority and uncertainty computations treat all assertions equally. An agent that makes many bold inferences gets the same authority weight as one that makes careful observations. This incentivizes noisy assertion.

**Three options:**

**Option A — No distinction. All datoms are equal.** The challenge/witness system handles quality control post-hoc. Bad inferences get challenged and refuted. Authority naturally gravitates toward agents that make assertions that survive challenges.

*Pro:* Simplest. The existing challenge system does the work.
*Con:* Slow convergence. Bad inferences persist until challenged. The authority computation has no way to weight contribution quality at assertion time.

**Option B — Provenance type on the transaction entity.** Each transaction carries `:tx/provenance-type` with values from a lattice:

```
:observed < :derived < :inferred < :hypothesized
```

Where:
- `:observed` = direct tool read / runtime observation
- `:derived` = computed from other datoms via deterministic rules (query results materialized as datoms)
- `:inferred` = pattern-matched / correlated from observations (the "self-authored associations" from the document)
- `:hypothesized` = speculative, explicitly provisional

The authority computation weights contributions by provenance type. The conflict resolution system gives precedence to lower (more grounded) provenance types.

*Pro:* Fine-grained control. The authority computation can distinguish "this agent makes reliable observations" from "this agent makes speculative leaps."
*Con:* Who enforces correct classification? The agent self-reports its provenance type. An adversarial or overconfident agent could label inferences as observations.

**Option C — Provenance type inferred structurally, not declared.** Instead of the agent declaring provenance, the system computes it from the datom's structure:

- A datom whose value matches a tool observation in the same transaction → `:observed`
- A datom whose value was computed by a Datalog rule → `:derived`
- A datom that references entities from different transactions (cross-temporal assertion) → `:inferred`
- A datom asserted with explicit `:datom/confidence < 1.0` → `:hypothesized`

*Pro:* Cannot be gamed. Provenance is structural, not declared.
*Con:* Imperfect classification. Some structurally-observed datoms might be semantically inferential.

**My recommendation: Option B with a verification overlay from Option C.** The agent declares provenance type (it knows its own intent), but the system can audit the declaration structurally. A transaction labeled `:observed` that doesn't contain any tool read operations is flagged as misclassified. This is the same verify-after-assert pattern we use for challenges.

The provenance lattice becomes a schema-level addition:

```
:tx/provenance    keyword  :one  :lattice  — provenance-type lattice
                                             :observed < :derived < :inferred < :hypothesized
```

And the authority computation weights contributions:

```
contribution_weight(datom) = base_weight × provenance_factor(datom.tx.provenance)

where provenance_factor(:observed) = 1.0
      provenance_factor(:derived) = 0.8
      provenance_factor(:inferred) = 0.5
      provenance_factor(:hypothesized) = 0.2
```

---

### PQ3 — Agent Failure Model

**The problem:** Agents can crash, go offline, or be terminated. The protocol must handle this without blocking the entire system. But our frontier exchange and sync barrier operations assume agents are reachable.

**Why this is blocking:** If the sync barrier operation blocks until all participants have exchanged frontiers, a single dead agent blocks the entire barrier. If frontier exchange is required for merge, a dead agent's datoms become unreachable.

**Three options:**

**Option A — Crash-stop model.** Agents are either alive or permanently dead. When an agent dies, its datoms remain in the store (they're immutable), but it's removed from the active agent set. Sync barriers exclude dead agents. Frontiers that reference dead agents' transactions are still valid (the transactions exist, the agent just can't produce new ones).

**Option B — Crash-recovery model.** Agents can crash and restart. On restart, they rebuild their local state from their last known frontier (stored durably, possibly in the datom store itself). The protocol supports reconnection — a recovering agent announces its frontier and receives the delta of datoms it missed.

**Option C — Byzantine model.** Agents can behave arbitrarily (including sending malformed or contradictory datoms). The protocol must tolerate a bounded number of faulty agents.

**My recommendation: Option B.** Crash-recovery is the realistic model for LLM agents (they're processes that can be killed and restarted). Byzantine tolerance is overkill for a system where all agents are ultimately under the operator's control. The key design implication: **an agent's frontier must be durably stored, not just held in memory.** On crash, the recovering agent reads its last frontier from durable storage and bootstraps from there.

This means we need:

```
INV-FRONTIER-DURABLE-001: Frontier Durability
An agent's frontier MUST be durably stored (to disk or to the 
shared store) after every transaction and after every merge.

On crash recovery, the agent's initial frontier is its last 
durably stored frontier. The delta between this frontier and the 
current shared store state represents the datoms the agent missed 
during downtime.
```

---

### PQ4 — Message Passing Semantics

**The problem:** When agents exchange datoms and signals, what delivery guarantees does the protocol require?

**Options:**
- **At-most-once:** Fire and forget. Simple but lossy.
- **At-least-once:** Retry until acknowledged. May deliver duplicates.
- **Exactly-once:** Guaranteed single delivery. Expensive.

**My recommendation: At-least-once with idempotent operations.** Because the store is a G-Set (adding the same datom twice is a no-op), duplicate delivery is harmless for TRANSACT and MERGE. The protocol exploits the CRDT's idempotence to get exactly-once *semantics* with at-least-once *delivery*. This is the standard CRDT pattern and it's well-understood.

The exception is SYNC-BARRIER, which requires stronger guarantees (all participants must participate). But sync barriers are rare, coordination-heavy operations by design (A3: optional sync barriers).

---

Those are my four blocking questions. Before I proceed with the full protocol formalization, I need your call on:

1. **PQ1:** Option B (two-tier: private working set + shared committed store)?
2. **PQ2:** Option B+C hybrid (declared provenance type with structural verification)?
3. **PQ3:** Option B (crash-recovery with durable frontiers)?
4. **PQ4:** At-least-once with idempotent operations?

Or do you see issues with any of these that I should reconsider?

---

## Willem

Agree with all of the above, with the additional caveat that I want sync barriers as part of the design. 

* Write-only log as the runtime itself (Writes, Queries/Reads, Projections, etc.)
   * Queries/Projections can serve as an input into datom significance (similar to how neural connections are strengthened with repeated access) or epistemic significance (i.e. projections can be ad-hoc: agents don't need the full spec always and you can generate different epistemological/ontological representations of the datoms depending on the context — useful projections are those which are revisited, either these projections or their underlying datom may be particularly load-bearing or informative)
   * Writes can be executed as patch branches that are layered on top of the current state and can be committed, combined, etc. Example use case would be 2+ agents both executing competing implementations of the same spec invariants and then choosing the best one or combining all options for the "best of all worlds result". Notice the bilateral dual here (similar to how multiple agents may be ideating with the user on the same train of thought and then reduced to the optimal solution prior to reification in specification definition)
   * This was the thought that came from the other conversation with the AI agent in the formal analysis of agentic systems: "The user is asking me for the single most impactful addition to the architecture we've been developing. Let me think deeply about what's missing and what would be most transformative.
We have:
      * EAV fact store
      * Dual-process S1/S2 architecture
      * Associate tool for schema discovery
      * Query tool for precise retrieval
      * Confusion feedback loop
      * Depth/breadth attentional aperture
What's the biggest gap? Let me think...
The agent currently only stores facts about what it observes and does. But it doesn't store facts about itself — its own reasoning patterns, what worked, what didn't, what strategies succeeded in what contexts. There's no learning across tasks or even within a long task.
The real leverage might be having the agent write assertions about its own epistemic state directly into the fact store — not just recording what happened, but capturing what it believes, where uncertainty lives, which strategies succeeded in which contexts. That transforms the fact store into a model of the agent's own cognition, enabling genuine metacognition. But even more powerful: the agent should be creating its own retrieval cues as datoms whenever it discovers connections between things, so those relationships become immediately queryable for future reasoning.
Human expertise works this way — experts don't just know more facts, they've built richer associative networks between them. When the agent writes its own edges into the EAV graph, the fact store becomes self-organizing and grows more useful with each discovery. The `associate` tool gets better over time because there are more paths to traverse, which is real learning through structural enrichment rather than parameter updates.
This mirrors Datomic's philosophy where the schema itself is data, so adding new relationships is just writing more datoms to the store. But I'm realizing the truly transformative piece might be something else entirely — letting the agent continuously build these associations in the background while reasoning, almost like System 1 running in parallel with System 2 to pre-fetch context. Though that feels more like an optimization than a fundamental shift. The core insight that matters most is probably just the self-authored edges themselves — giving the agent the ability to permanently reshape its own knowledge graph based on what it learns.
Actually, wait. What if the real breakthrough is even more basic: storing the agent's intentions and plans as first-class facts in the store? Right now it only records what happened, but if the agent's current goals and reasoning were structured data in there too, then `associate` could surface relevant plans automatically, confusion detection could flag when actions drift from stated intentions, and goal dilution becomes something you can actually query for.
But stepping back, which of these ideas is actually most powerful? The self-authored associations create a feedback loop where the system improves with use — that's fundamentally different from a one-time architectural fix. Though I'm wondering if there's something even more radical underneath all this...
What if the fact store itself becomes the runtime?
No, that's getting too abstract. I think the answer is self-authored associations — where the agent writes its own relationship assertions into the knowledge graph, making it progressively more effective. That's the one that turns this from a static tool into a learning system with compounding returns."
* Auto-prompting guidance (G) for reifying the set of possible topologies at any given state? Topological shapes are currently implicit in the guidance, but perhaps treating this as a first-class citizen in the algebraic formulation makes sense (somewhat comonadic in its structure). See the example of a guidance return string for the `remediate` command:


```
func generateGuidance(driftType, target, location string, isKnownElement bool) []string {
	var guidance []string

	switch driftType {
	case "correctness":
		if isKnownElement {
			guidance = append(guidance,
				fmt.Sprintf("Write the full definition for %s in module %s", target, location),
				"Include all required components (statement, semi-formal, violation, validation, why)",
				"Add cross-references to related invariants and ADRs",
			)
		} else {
			guidance = append(guidance,
				fmt.Sprintf("Implement %s as specified in the spec", target),
				"Verify the implementation matches all relevant invariants",
			)
		}
	case "coherence":
		guidance = append(guidance,
			fmt.Sprintf("Resolve the cross-reference to %s", target),
			"Either define the target element or update the reference to point to the correct element",
			"Run `ddis validate` to check for remaining unresolved references",
		)
	case "depth":
		guidance = append(guidance,
			fmt.Sprintf("Add %s to the spec's element specifications", target),
			fmt.Sprintf("Write an invariant following the pattern of existing invariants in %s", location),
			"Add cross-references to INV-003 (falsifiability) and INV-007 (signal-to-noise)",
		)
	}

	guidance = append(guidance,
		fmt.Sprintf("After writing: ddis parse && ddis drift (expect drift to decrease by 1)"),
	)

	return guidance
}
```

* Mediation/Conflict resolution AS datoms. Decisions about the value of two different approaches, or resolving conflicts and the underlying rationale behind the decisions are themselves datom about the intent/spec/implementation bilateral dual reconciliation process. This is likely very valuable data.
* Projection structure might benefit from an approach inspired by https://factory.strongdm.ai/techniques/pyramid-summaries.md? Not sure how we think about this. It seems that, absent the case where generating the ENTIRE spec is necessary (really only for when a human wants to review it in its entirety), all other spec projections are flexible and ad-hoc depending on the use case and context.
* There is a missing TUI or human-legible, live feedback mechanism (i.e. one which is not driven by requesting an update from the agents). This will allow the user to proactively direct their attention where it is needed, especially across multiple agents or agent groups.

Consider all of this deeply, methodically, and meticulously. I want a "cleanroom software engineering" approach: fully grounded in "formal methods", "spec-driven-design" and abstract algebra. Use "first principles" thinking at every step. Do NOT make assumptions -- if you encounter uncertainty, ask me clarifying questions.

---

## Claude

I need to think carefully about each of these before formalizing. Several of your points force revisions to the foundational algebra — most significantly, the patch branch model changes the CRDT structure, and "the datom store as runtime" is a conceptual inversion that affects everything downstream. Let me work through this layer by layer.

---

## Foundational Revision: The Store as Runtime

The other conversation almost arrived at this idea and then pulled back: *"What if the fact store itself becomes the runtime? No, that's getting too abstract."* You're saying: no, go there. And I think you're right, but I need to be precise about what this means and what it doesn't mean.

The tripartite decomposition (E, R, A) still holds. The POSIX runtime R still exists — you can't evaluate a Datalog query without a CPU, you can't store datoms without a disk. What changes is the **protocol boundary**: the protocol doesn't talk to R directly. Every protocol-visible state change is a datom. The runtime R is the *substrate* on which the datom store runs, but it's invisible to the protocol, just as the silicon is invisible to the operating system.

Formally, redefine the agent system:

```
AgentSystem_revised = (D, Op_D, Obs_D, A, π, Σ, Γ) where

  D     : DatomStore              — the G-Set CRDT (shared state)
  Op_D  : Type                    — operations ON the store 
                                    (transact, query, associate, 
                                     assemble, branch, merge, subscribe)
  Obs_D : Type                    — observations FROM the store 
                                    (query results, schema neighborhoods,
                                     assembled contexts, notifications)
  A     : Set AgentId             — the agent population
  π     : Obs_D → Op_D + Done    — agent policy (unchanged)
  Σ     : SyncBarrier             — coordination primitive
  Γ     : Guidance                — the topology of available moves
```

The key shift: `Op` and `Obs` no longer refer to the POSIX runtime. They refer to the datom store. The agent's entire interface to the world is through store operations. If an agent needs to read a file, that file's contents are either already datoms (ingested previously) or the agent issues a TRANSACT that records the file read as an observation datom. The runtime mediation is pushed below the protocol boundary.

```
INV-STORE-AS-RUNTIME-001: Protocol-Level State Completeness
Every state change visible to the protocol MUST be represented as 
one or more datoms in the store.

Formally: if agent α's policy π would produce different output 
given knowledge of state change Δ, then Δ MUST be recorded as 
datoms in D.

Consequence: Exogenous events (the frame problem from the document) 
are handled by requiring agents to assert their observations. If 
an agent reads a file and its contents have changed since the last 
read, the agent MUST assert the new observation. The store's 
epistemic state is updated through agent assertions, not through 
direct runtime coupling.

Violation: An agent's behavior depends on state (e.g., a file's 
contents) that exists only in the runtime and has no datom 
representation. If the runtime changes this state exogenously, 
the agent's beliefs become stale without any mechanism for 
detection.

Verification: For every tool operation an agent can perform, 
there exists a corresponding transaction pattern that records 
the operation's inputs and outputs as datoms.
```

This invariant is strong. It means the store is the **single source of truth** for the protocol layer. The POSIX runtime becomes an implementation detail — a place where tools execute, but not where state lives.

---

## Patch Branches: From G-Set to Branching G-Set

This is the most significant algebraic change. Your description of patch branches — layered on top of current state, committable, combinable, supporting competing implementations — requires extending the pure G-Set model.

### The Algebra

A **branching G-Set** is a tuple (S, B, ⊑, commit, combine) where:

- S is the **trunk** — the committed shared store, still a G-Set over datoms D
- B = {b₁, b₂, ..., bₙ} is a set of **branches**, each itself a G-Set over D
- ⊑ is the **ancestry relation**: b ⊑ S means branch b was forked from a snapshot of S
- **commit : B → S → S'** merges a branch into the trunk: S' = S ∪ b
- **combine : B × B → B** merges two branches: b₃ = b₁ ∪ b₂

The key properties:

```
PROPERTY 1 (Monotonicity of trunk):
  commit(b, S) = S ∪ datoms(b)
  Therefore S ⊆ commit(b, S) for all b.
  The trunk only grows. Commits are monotonic.

PROPERTY 2 (Branch isolation):
  For branches b₁, b₂ where neither is an ancestor of the other:
  datoms visible to queries on b₁ = S_at_fork(b₁) ∪ datoms(b₁)
  datoms visible to queries on b₂ = S_at_fork(b₂) ∪ datoms(b₂)
  Branches cannot see each other's datoms unless explicitly combined.

PROPERTY 3 (Combination commutativity):
  combine(b₁, b₂) = combine(b₂, b₁)
  Branch combination is set union, which commutes.

PROPERTY 4 (Commit-combine equivalence):
  commit(combine(b₁, b₂), S) = commit(b₁, commit(b₂, S))
  Combining then committing = committing sequentially.
  (This follows from associativity and commutativity of set union.)

PROPERTY 5 (Fork snapshot):
  When branch b is forked from trunk S at time t,
  b.base = S|_{frontier(t)}
  The branch sees the trunk as it was at fork time.
  Subsequent trunk commits are NOT visible to b unless b rebases.
```

### Branch as Entity

Branches are first-class entities in the store:

```
ENTITY TYPE: Branch

:branch/ident           keyword  :one   :lww    — unique branch name
:branch/base-tx         ref      :one   :lww    — trunk transaction at fork point
:branch/agent           ref      :one   :lww    — agent that created this branch
:branch/status          keyword  :one   :lattice — :active < :proposed < :committed 
                                                    < :abandoned
                                                    (lattice: branch-lifecycle)
:branch/purpose         string   :one   :lww    — rationale for the branch
:branch/parent-branch   ref      :one   :lww    — for nested branches (branch of a branch)
:branch/competing-with  ref      :many  :multi  — set of branches exploring same problem

ENTITY TYPE: Branch Comparison

:comparison/branches     ref      :many  :multi  — branches being compared
:comparison/criterion    keyword  :one   :lww    — what's being compared
:comparison/method       keyword  :one   :lww    — :automated-test, :fitness-score, 
                                                   :agent-review, :human-review
:comparison/scores       json     :one   :lww    — per-branch scores
:comparison/winner       ref      :one   :lww    — selected branch (if decided)
:comparison/rationale    string   :one   :lww    — why this branch was selected
:comparison/agent        ref      :one   :lww    — who performed the comparison
```

The `:branch/competing-with` attribute is critical. When two or more branches are marked as competing, the system knows they represent alternative approaches to the same problem. This enables:

1. Automated comparison (run tests against both, compare fitness scores)
2. Combination (take the best parts of each — which is itself a new branch)
3. Deliberation (agents argue for their preferred branch — mediation as datoms)

### Transaction-Branch Association

Every transaction lives on exactly one branch (or on trunk):

```
:tx/branch              ref      :one   :lww    — the branch this tx belongs to
                                                   (nil = trunk)
```

A query against a branch sees: trunk datoms up to the fork point + all branch datoms. This is computed by the query engine using the branch's base-tx to filter trunk datoms.

```
visible(branch b) = {d ∈ trunk | d.tx ≤ b.base-tx} ∪ {d | d.tx.branch = b}

visible(trunk) = {d ∈ trunk}    — all committed datoms
```

### The Bilateral Dual You Identified

You noted a bilateral symmetry between:
- **Implementation branches**: competing implementations of the same spec → select/combine the best
- **Ideation branches**: competing explorations of the same design question → reduce to optimal before crystallizing

This symmetry is real and structural. Both are instances of the same pattern:

```
PATTERN: Diverge-Compare-Converge

1. DIVERGE: Fork N branches from the same base state
2. PARALLEL: Agents work independently on each branch
3. COMPARE: Evaluate branches against shared criteria
4. CONVERGE: Either select one, combine several, or abandon all
5. COMMIT: The convergent result merges to trunk

This pattern is parameterized by:
  - The base state (trunk at fork point)
  - The divergence criterion (what each branch explores)
  - The comparison method (testing, fitness, deliberation)
  - The convergence strategy (select, combine, synthesize)
```

The forward flow (human → agents → implementation) uses this for competing implementations. The backward flow (implementation → spec → human) uses this for competing interpretations of what the implementation means for the spec. The algebra is identical. Only the direction of information flow differs.

```
INV-BRANCH-SYMMETRY-001: Bilateral Branch Duality
The branch diverge-compare-converge pattern MUST support both 
forward-flow (spec → competing implementations → selected 
implementation) and backward-flow (implementation → competing 
spec updates → selected spec update) with identical branch 
semantics.

Formally: let DCC(base, criteria, method, strategy) be the 
diverge-compare-converge pattern. Then:

  DCC(spec_state, impl_alternatives, test_suite, best_of) 
    = forward_flow_branch

  DCC(impl_state, spec_interpretations, fitness, synthesis)
    = backward_flow_branch

Both instantiate the same algebraic structure.

Violation: The system supports branching for implementation work 
but requires linear (non-branching) spec modifications. This 
prevents the backward flow from exploring alternatives.
```

---

## Query Significance: Hebbian Strengthening

Your neural connection analogy is precise: connections strengthened by repeated access. The formal structure:

### The Access Functor

Define an **access event** as a triple (query, result_datoms, timestamp). Every time a query executes, an access event is generated. The access event is NOT a datom in the main store — it's a record in a separate **access log** that sits alongside the main store:

```
AccessLog = [(Query, Set<DatomId>, Instant)]     — append-only

significance : DatomId → ℝ≥0
significance(d) = Σ_{(q, R, t) ∈ AccessLog, d ∈ R} decay(now - t) × query_weight(q)
```

Where `decay` is an exponential decay function (recent access counts more) and `query_weight` captures the importance of the query context (a query during conflict resolution weighs more than an idle browse).

The significance score is periodically materialized into the main store:

```
ENTITY TYPE: Datom Significance (materialized periodically)

:significance/entity        ref      :one   :lww
:significance/attribute     ref      :one   :lww  
:significance/score         double   :one   :lww   — computed significance
:significance/access-count  long     :one   :lww   — total access count
:significance/access-recency instant :one   :lww   — most recent access
:significance/query-contexts keyword :many  :multi — categories of queries that hit this
:significance/computed-at   ref      :one   :lww
```

**Why the access log is separate from the main store:** If every access event were a datom, then querying the access log would generate more access events, which would generate more datoms, in an unbounded positive feedback loop. The access log is an **observation of the store**, not a part of the store. It's the same architectural pattern as the LIVE index — a materialized structure maintained alongside the main store, periodically summarized into datoms.

### Significance in ASSOCIATE and ASSEMBLE

The significance score feeds directly into the two-phase retrieval pattern:

**In ASSOCIATE:** When traversing the entity-attribute graph to discover schema neighborhoods, weight edges by the significance of the datoms they represent. High-significance attributes surface first. This means the agent's "peripheral vision" is biased toward the parts of the store that have been most operationally relevant — exactly the Hebbian effect you described.

**In ASSEMBLE:** When selecting datoms to fit within the context budget, use significance as a selection criterion alongside query relevance. Two equally relevant datoms: pick the one with higher significance, because it's been load-bearing in more past interactions.

```
assemble_score(datom, query_relevance) = 
  α × query_relevance + β × significance(datom) + γ × recency(datom)

— The α, β, γ weights are configurable.
— Default: α=0.5, β=0.3, γ=0.2
— Significance gets 30% weight: meaningful but not dominant.
```

### Projection Significance

Your point about projections deserving their own significance tracking is important. A projection is a specific *combination* of datoms assembled for a particular purpose. The projection pattern itself (which entities, which attributes, at what level of detail) may be reusable even when the underlying datom values change.

```
ENTITY TYPE: Projection Pattern

:projection/query         string   :one   :lww   — the Datalog query that defines this projection
:projection/purpose       string   :one   :lww   — what this projection is for
:projection/parameters    json     :one   :lww   — parameterized inputs
:projection/entity-types  keyword  :many  :multi — entity types included
:projection/access-count  long     :one   :lattice (numeric-max)
:projection/last-used     instant  :one   :lww
:projection/agent-origin  ref      :one   :lww   — who first created this projection
:projection/usefulness    double   :one   :lww   — empirically observed quality score
```

When an agent creates an ad-hoc projection that proves useful (i.e., the task succeeds after using this view of the data), the projection pattern gets reified as a datom. Other agents — or the same agent in future sessions — can discover this projection pattern via ASSOCIATE and reuse it. Projections that are repeatedly reused get higher significance scores, making them surface more readily.

This is the mechanism by which the system learns "useful ways to look at the data." It's not learning new facts — it's learning new *queries*. The ontological structure isn't fixed; it emerges from which projections prove useful in practice.

```
INV-PROJECTION-LEARNING-001: Projection Reification
A projection pattern whose access-count exceeds the reification 
threshold MUST be stored as a first-class Projection Pattern 
entity in the store.

The reification threshold is a configurable system parameter 
(default: 3 accesses within a time window).

Consequence: Useful projections propagate. An agent that discovers 
a useful way to view the data creates a projection pattern that 
other agents can discover via ASSOCIATE. The system develops a 
shared vocabulary of "good ways to look at things."
```

---

## Mediation and Deliberation as Datoms

This is not merely recording the resolution of conflicts — it's recording the *reasoning process* by which conflicts are resolved. You've identified that this data is valuable because:

1. It captures *why* decisions were made (audit trail)
2. It creates queryable precedent (future conflicts can reference similar past deliberations)
3. The deliberation itself is a bilateral dual — agents advocating for different positions, converging on a resolution — which mirrors the spec crystallization process

### Deliberation as a First-Class Process

```
ENTITY TYPE: Deliberation

:deliberation/subject       ref      :one   :lww    — what's being deliberated
                                                      (a conflict, a branch comparison,
                                                       a design question, a spec element)
:deliberation/type          keyword  :one   :lww    — :conflict-resolution, :branch-selection,
                                                      :design-decision, :spec-crystallization,
                                                      :implementation-choice
:deliberation/status        keyword  :one   :lattice — :open < :deliberating < :proposed 
                                                        < :decided < :revisited
                                                        (lattice: deliberation-lifecycle)
:deliberation/participants  ref      :many  :multi  — agents (including human) involved
:deliberation/initiator     ref      :one   :lww    — who opened the deliberation
:deliberation/deadline      instant  :one   :lww    — optional decision deadline
:deliberation/decision      ref      :one   :lww    — ref to the Decision entity

ENTITY TYPE: Position

:position/deliberation      ref      :one   :lww    — parent deliberation
:position/agent             ref      :one   :lww    — who holds this position
:position/stance            keyword  :one   :lww    — :advocate, :oppose, :neutral, :synthesize
:position/target            ref      :one   :lww    — what's being advocated for
                                                      (a branch, a value, an approach)
:position/argument          string   :one   :lww    — the argument
:position/evidence          ref      :many  :multi  — datoms cited as supporting evidence
:position/rebuts            ref      :one   :lww    — which prior position this responds to
:position/ordinal           long     :one   :lww    — ordering within deliberation
:position/strength          double   :one   :lww    — self-assessed confidence (0-1)

ENTITY TYPE: Decision

:decision/deliberation      ref      :one   :lww    — parent deliberation
:decision/chosen            ref      :one   :lww    — the winning position/branch/value
:decision/method            keyword  :one   :lww    — :consensus, :majority, :authority,
                                                      :human-override, :automated
:decision/rationale         string   :one   :lww    — synthesized reasoning
:decision/dissents          ref      :many  :multi  — positions that disagreed
:decision/conditions        string   :one   :lww    — conditions under which to revisit
:decision/decided-by        ref      :one   :lww    — agent or human who made final call
:decision/confidence        double   :one   :lww    — decision confidence (0-1)
```

### Precedent Queries

The deliberation history becomes queryable precedent:

```datalog
;; QUERY: find-precedent
;; Given a current conflict/decision, find similar past deliberations
;; whose outcomes might inform the current one.

[:find ?past-delib ?past-decision ?rationale
 :in $ ?current-subject
 :where
 ;; Get the type and attributes of the current subject
 [?current-subject :entity/type ?type]
 [?current-subject ?attr _]
 
 ;; Find past deliberations about entities of the same type
 [?past-delib :deliberation/subject ?past-subject]
 [?past-subject :entity/type ?type]
 [?past-delib :deliberation/status :decided]
 
 ;; That share at least one contested attribute
 [?past-position :position/deliberation ?past-delib]
 [?past-subject ?attr _]
 
 ;; Get the decision
 [?past-delib :deliberation/decision ?past-decision]
 [?past-decision :decision/rationale ?rationale]]
```

This gives agents the ability to say "we faced a similar question before, and here's what we decided and why." The deliberation history is a case law system for the spec development process.

### The Bilateral Deliberation Dual

You identified that deliberation mirrors both the ideation process (converging from many ideas to one spec element) and the implementation process (choosing among competing implementations). I want to formalize this explicitly:

```
INV-DELIBERATION-BILATERAL-001: Deliberation Duality
Every deliberation MUST be classifiable as either forward-flow 
(converging toward specification) or backward-flow (converging 
toward implementation), and the deliberation protocol MUST be 
identical in both cases.

Forward deliberation: multiple findings/proposals → one spec element
  Participants argue for their preferred formulation.
  Evidence is drawn from discovery threads and external research.
  Decision crystallizes into an invariant, ADR, or section.

Backward deliberation: multiple implementations → one chosen approach
  Participants argue for their preferred branch.
  Evidence is drawn from test results, fitness scores, and metrics.
  Decision commits the winning branch (or a synthesis) to trunk.

Both use the same Position/Decision/Deliberation entity structure.
The :deliberation/type attribute distinguishes them.
```

---

## Guidance as First-Class Citizen (The Comonadic Structure)

Your observation about the comonadic structure is sharp. Let me formalize it.

### The Guidance Comonad

A **comonad** (W, extract, extend) over a category C gives you:
- extract : W(A) → A — "what is the value at the current focus?"
- duplicate : W(A) → W(W(A)) — "embed the current view in the space of all views"
- extend : (W(A) → B) → W(A) → W(B) — "apply a function at every possible refocusing"

For guidance, the types instantiate as:

```
W(A) = (StoreState, A)    — A value together with the store context it lives in

extract : (StoreState, [Action]) → [Action]
  — Given a store state and available actions, return the actions.
  — This is trivial: just project the second component.

duplicate : (StoreState, [Action]) → (StoreState, (StoreState, [Action]))
  — Embed: the current guidance in the space of all possible guidances.
  — This gives you "from here, what guidances are available at 
  — each reachable state?"

extend : ((StoreState, [Action]) → B) → (StoreState, [Action]) → (StoreState, B)
  — Apply a function (e.g., "score this guidance") to every 
  — reachable guidance context.
```

The `extend` operation is what makes this powerful: given a function that evaluates guidance quality (e.g., "will this action reduce drift?"), `extend` applies it to every reachable state, producing a **scored guidance map** — a view of the entire action landscape with quality estimates at every point.

### Guidance Topology as Datoms

The topology of available moves at any state is itself data:

```
ENTITY TYPE: Guidance Node

:guidance/state-predicate   string   :one   :lww   — Datalog query that defines when 
                                                      this guidance applies
:guidance/actions           ref      :many  :multi — available actions
:guidance/preconditions     ref      :many  :multi — invariants/conditions that must hold
:guidance/postconditions    ref      :many  :multi — invariants/conditions that will hold after
:guidance/topology-type     keyword  :one   :lww   — :sequential, :branching, :converging, 
                                                      :cyclic, :terminal
:guidance/priority          long     :one   :lww   — ordering among applicable guidances
:guidance/learned           boolean  :one   :lww   — was this guidance learned from experience?
:guidance/effectiveness     double   :one   :lww   — empirical success rate

ENTITY TYPE: Guidance Action

:guidance-action/description string  :one   :lww   — human-readable description
:guidance-action/operation   keyword :one   :lww   — the protocol operation to perform
:guidance-action/parameters  json    :one   :lww   — operation parameters
:guidance-action/next-state  ref     :one   :lww   — guidance node reached after this action
:guidance-action/risk        double  :one   :lww   — estimated risk of this action
:guidance-action/cost        double  :one   :lww   — estimated cost (tokens, time, etc.)
```

Your Go function `generateGuidance` becomes a **query over the guidance graph**:

```datalog
;; QUERY: available-guidance
;; Given the current store state, what guidance is applicable?

[:find ?guidance ?action ?description ?priority
 :where
 [?guidance :guidance/state-predicate ?predicate]
 
 ;; Evaluate the predicate against current store state
 ;; (this requires meta-evaluation: running a stored query)
 [(eval-predicate ?predicate) ?holds]
 [(true? ?holds)]
 
 ;; Check preconditions
 (not [?guidance :guidance/preconditions ?pre]
      (not (holds? ?pre)))
 
 ;; Get actions
 [?guidance :guidance/actions ?action-ref]
 [?action-ref :guidance-action/description ?description]
 [?guidance :guidance/priority ?priority]]
```

The critical difference from hardcoded guidance: **agents can WRITE guidance nodes.** When an agent discovers that a particular sequence of actions reliably resolves a particular kind of drift, it can assert a new guidance node. This guidance then becomes available to all agents via the query above. The guidance topology evolves as the system learns what works.

```
INV-GUIDANCE-EVOLUTION-001: Guidance Learning
Agents MAY assert new Guidance Node and Guidance Action entities 
based on observed patterns of successful action sequences.

The :guidance/learned flag MUST be set to true for agent-generated 
guidance, distinguishing it from human-authored or system-default 
guidance.

The :guidance/effectiveness score MUST be updated based on 
empirical outcomes: if the guided action sequence produces the 
predicted postconditions, effectiveness increases; otherwise 
it decreases.

Guidance nodes whose effectiveness drops below the abandonment 
threshold (configurable, default 0.3) SHOULD be retracted.
```

The comonadic structure ensures that guidance is always relative to a context. There's no "global best action" — there's only "best action given the current store state." The `extend` operation lets you compose guidance strategies: "if I take action A, what guidance will be available at the resulting state?" This is lookahead planning expressed as comonadic extension.

---

## Projection Structure: Pyramid Summaries over Datoms

Your reference to the StrongDM pyramid summaries technique maps directly to the ASSEMBLE operation. The core idea: maintain multiple levels of summarization of the datom store, so that context assembly can select the appropriate level of detail for each entity.

### The Pyramid

Define a **projection pyramid** as a family of projection functions indexed by level:

```
π₀ : Store → FullRepresentation        — every datom, verbatim
π₁ : Store → EntitySummaries           — one paragraph per entity  
π₂ : Store → TypeSummaries             — one paragraph per entity type
π₃ : Store → StoreSummary              — one paragraph for entire store

with the property:
  information(π₀) ≥ information(π₁) ≥ information(π₂) ≥ information(π₃)
  cost(π₀) ≥ cost(π₁) ≥ cost(π₂) ≥ cost(π₃)
```

Each level is a lossy compression of the one below it. The ASSEMBLE operation selects the level per entity based on relevance:

```
assemble(query_results, budget) =
  for each entity e in query_results:
    relevance = compute_relevance(e, query_context)
    significance = significance_score(e)
    combined = α × relevance + β × significance
    
    level(e) = 
      if combined > threshold_detail: π₀(e)    — full datoms
      if combined > threshold_summary: π₁(e)   — entity summary
      if combined > threshold_type: π₂(e)      — type summary
      else: omit(e)                             — below budget threshold
    
  pack levels into budget, prioritizing higher-combined-score entities
```

### Pyramid Materialization

The summaries at each level are themselves datoms — they're pre-computed projections stored in the store:

```
ENTITY TYPE: Projection Level

:projection-level/entity    ref      :one   :lww   — entity being summarized
:projection-level/level     long     :one   :lww   — 0=full, 1=entity, 2=type, 3=store
:projection-level/content   string   :one   :lww   — the summary text
:projection-level/token-cost long    :one   :lww   — token count of this summary
:projection-level/stale     boolean  :one   :lww   — has the entity changed since summary?
:projection-level/computed-at ref    :one   :lww
```

Summaries become stale when the underlying entity's datoms change. The system marks them stale (a lightweight operation) and recomputes them lazily (when an agent requests a projection that includes stale entities) or eagerly (background maintenance task).

Your observation is correct that the full spec projection (π₀ over everything) is a special case — only needed when a human wants the complete document. All other uses are ad-hoc, context-dependent, and can mix levels freely. An agent debugging a specific invariant might get π₀ for that invariant, π₁ for neighboring invariants, π₂ for the module, and π₃ for the rest of the spec. This is the rate-distortion optimization with a concrete implementation.

---

## TUI / Live Feedback: The Subscription Model

You've identified a missing primitive: the human needs to see what's happening without asking. This requires a **push model** over the datom store — an event stream that the TUI subscribes to and renders.

### The Subscription Operation

```
SUBSCRIBE : Pattern × Callback → SubscriptionId

where:
  Pattern = Datalog-like filter over datom events
  Callback = handler invoked when matching datoms are asserted

Examples:
  — "Notify me when any conflict is detected"
  subscribe([?c :conflict/status :detected], notify_conflict)
  
  — "Notify me when uncertainty exceeds threshold on any entity"
  subscribe([?u :uncertainty/scalar ?v] [(> ?v 0.7)], notify_high_uncertainty)
  
  — "Notify me when any branch changes status"
  subscribe([?b :branch/status _], notify_branch_update)
  
  — "Notify me of all deliberation activity"
  subscribe([?p :position/deliberation _], notify_deliberation_turn)
```

### The TUI as a Projection

The TUI is a continuously-updated projection of the store, driven by subscriptions. It doesn't need to show everything — it needs to show the **human-relevant state changes**. Define the TUI state as a sliding-window projection:

```
ENTITY TYPE: TUI Configuration

:tui/subscriptions      ref      :many  :multi  — active subscription patterns
:tui/focus-entities     ref      :many  :multi  — entities the human is watching closely
:tui/alert-threshold    double   :one   :lww    — uncertainty level that triggers alerts
:tui/display-depth      long     :one   :lww    — how much detail to show (pyramid level)
:tui/refresh-rate       long     :one   :lww    — milliseconds between updates
:tui/layout             keyword  :one   :lww    — :dashboard, :timeline, :graph, :split
```

The TUI renders:

1. **Activity stream** — recent transactions across all agents, color-coded by type (observation, inference, deliberation, conflict, resolution)
2. **Uncertainty heatmap** — entities colored by their uncertainty scalar, updating in real-time as measurements are asserted
3. **Branch status** — active branches, their purpose, their agents, and any ongoing comparisons
4. **Deliberation panel** — open deliberations with positions and current state
5. **Guidance overlay** — what actions are available at the current state, with effectiveness scores
6. **Fitness gauge** — the spec fitness function F(S) components, updating as changes occur

Each of these is a subscription-driven projection. The TUI doesn't poll — it reacts to datom assertions that match its subscription patterns.

```
INV-TUI-LIVENESS-001: Human Awareness Guarantee
Any datom assertion that changes the delegation-decision of an 
entity from :delegatable to :contested, :escalated, or 
:human-required MUST trigger a notification to the TUI within 
the configured refresh-rate.

Any conflict detected with severity exceeding the configured 
alert-threshold MUST trigger an alert.

Any deliberation that transitions to :decided MUST notify the 
TUI.

Violation: A conflict is resolved by automated means while the 
human is unaware it existed, and the resolution contradicts the 
human's intent.

Verification: Assert a high-severity conflict; verify the TUI 
subscription fires within the configured latency.
```

---

## Self-Authored Associations: The Metacognitive Layer

Integrating the insight from the other conversation: agents should write their own epistemic state — not just what they observe, but what they *believe, intend, and have learned* — into the store.

### The Metacognitive Entity Types

```
ENTITY TYPE: Belief

:belief/agent             ref      :one   :lww    — who holds this belief
:belief/subject           ref      :one   :lww    — entity the belief is about
:belief/content           string   :one   :lww    — what the agent believes
:belief/confidence        double   :one   :lww    — subjective confidence (0-1)
:belief/basis             ref      :many  :multi  — datoms supporting this belief
:belief/provenance        keyword  :one   :lww    — :observed, :inferred, :hypothesized

ENTITY TYPE: Intention

:intention/agent          ref      :one   :lww    — who intends this
:intention/goal           string   :one   :lww    — what the agent is trying to do
:intention/plan           ref      :many  :multi  — ordered steps (ref to guidance actions)
:intention/status         keyword  :one   :lattice — :forming < :committed < :executing 
                                                     < :completed < :abandoned
:intention/progress       double   :one   :lww    — 0.0 to 1.0
:intention/spec-element   ref      :one   :lww    — the spec element this serves

ENTITY TYPE: Learned Association

:association/agent        ref      :one   :lww    — who learned this
:association/source       ref      :one   :lww    — one end of the association
:association/target       ref      :one   :lww    — other end
:association/type         keyword  :one   :lww    — :causal, :correlative, :architectural,
                                                    :strategic, :analogical
:association/strength     double   :one   :lww    — how strong the association (0-1)
:association/basis        string   :one   :lww    — why this association was formed
:association/validated    boolean  :one   :lww    — has this been confirmed by experience?

ENTITY TYPE: Strategic Heuristic

:heuristic/observation    string   :one   :lww    — what was observed
:heuristic/pattern        string   :one   :lww    — the generalized pattern
:heuristic/action         string   :one   :lww    — what to do when pattern matches
:heuristic/context        keyword  :many  :multi  — when this heuristic applies
:heuristic/success-count  long     :one   :lattice (numeric-max) — times this worked
:heuristic/failure-count  long     :one   :lattice (numeric-max) — times this didn't
:heuristic/agent          ref      :one   :lww
```

### Goal Dilution Detection

With intentions as first-class datoms, the system can detect when an agent's actions drift from its stated goals:

```datalog
;; QUERY: goal-drift
;; Detect when an agent's recent actions don't align with its 
;; stated intention.

[:find ?agent ?intention ?last-action ?drift-signal
 :where
 [?intention :intention/agent ?agent]
 [?intention :intention/status :executing]
 [?intention :intention/spec-element ?target]
 
 ;; Get the agent's most recent transaction
 [?recent-tx :tx/agent ?agent]
 (not [?newer-tx :tx/agent ?agent]
      (causal-ancestor ?recent-tx ?newer-tx))
 
 ;; Check if the transaction touches the intended target
 ;; or something related to it
 [?recent-datom _ _ ?recent-tx]
 (not (entity-depends-on-transitive ?recent-datom ?target))
 (not (entity-depends-on-transitive ?target ?recent-datom))
 
 ;; The agent's last action doesn't relate to its intention
 [(str "Agent " ?agent " last action unrelated to intention " 
       ?intention) ?drift-signal]]
```

This is the "goal dilution" problem from the document, now detectable as a structural property of the store. When the TUI's subscription fires on this pattern, the human sees "Agent α appears to have drifted from its stated goal" — and can intervene proactively rather than waiting for the agent to produce wrong output.

---

## The Complete Protocol

With all of the above integrated, here is the full protocol specification. I'm writing this as the definitive reference — every operation, its type signature, its invariants, and its interaction with the extended model.

### Protocol Types

```
TYPE DEFINITIONS:

DatomId       = UUID
EntityId      = UUID  
AttrId        = UUID (also an EntityId — attributes are entities)
TxId          = UUID (also an EntityId — transactions are entities)
BranchId      = UUID (also an EntityId — branches are entities)
AgentId       = UUID (also an EntityId — agents are entities)
BarrierId     = UUID

Value         = String | Keyword | Boolean | Long | Double | Instant
              | UUID | Ref EntityId | Bytes | URI | BigInt | BigDec
              | Tuple [Value] | Json String

Op            = Assert | Retract

Datom         = { entity: EntityId
               , attr: AttrId
               , value: Value
               , tx: TxId
               , op: Op }

Frontier      = Map<AgentId, TxId>
  — For each known agent, the latest transaction seen from them.
  — The local frontier represents the agent's causal knowledge boundary.

SchemaNeighborhood = { entities: Map<EntityId, Set<AttrId>>
                     , types: Map<AttrId, ValueType>
                     , edges: Set<(EntityId, AttrId, EntityId)>
                     , significance: Map<EntityId, Double> }

AssembledContext = { datoms: [Datom]
                   , summaries: Map<EntityId, (Level, String)>
                   , token_count: Long
                   , stability_score: Double
                   , projection_id: Option<ProjectionId> }

Signal        = Confusion ConfusionType Cue
              | Conflict ConflictId
              | UncertaintySpike EntityId DeltaU
              | ResolutionProposal ConflictId Value String
              | DelegationRequest EntityId TaskId
              | GoalDrift AgentId IntentionId
              | BranchReady BranchId
              | DeliberationTurn DeliberationId PositionId

ConfusionType = NeedMore | Contradictory | GoalUnclear | SchemaUnknown

QueryMode     = Monotonic                    — no negation, no aggregation
              | Stratified Frontier          — negation allowed, frontier-bound
              | Barriered BarrierId          — requires sync barrier

Level         = 0 | 1 | 2 | 3               — pyramid summary level
```

### Protocol Operations

#### OPERATION 1: TRANSACT

Append datoms to the store. The fundamental write operation.

```
TRANSACT : TransactRequest → TransactResponse

TransactRequest = {
  agent:          AgentId,
  branch:         Option<BranchId>,      — None = trunk
  datoms:         [DatomSpec],           — datoms to assert/retract
  causal_parents: Set<TxId>,            — this agent's current frontier tips
  provenance:     ProvenanceType,        — :observed | :derived | :inferred | :hypothesized
  rationale:      String,                — why this transaction
  operation:      Keyword                — :op/observe, :op/infer, :op/deliberate,
                                           :op/crystallize, :op/resolve, etc.
}

DatomSpec = {
  entity:  EntityId | TempId,           — TempId for new entities
  attr:    AttrId,
  value:   Value,
  op:      Op                            — Assert or Retract
}

TransactResponse = {
  tx:             TxId,                  — the new transaction entity
  entity_map:     Map<TempId, EntityId>, — resolved temporary ids
  new_frontier:   Frontier,              — agent's updated frontier
  conflicts:      [ConflictId],          — conflicts detected (if any)
  invalidated:    [QueryCacheKey]        — cached queries now stale
}
```

**Invariants:**

```
INV-TX-APPEND-001: Transaction Append-Only
A TRANSACT operation MUST only append datoms. It MUST NOT 
modify or delete existing datoms.

Formally: let S be the store before TRANSACT and S' after.
Then S ⊆ S' and |S'| = |S| + |request.datoms|.

INV-TX-CAUSAL-001: Causal Ordering
The causal_parents field of a TRANSACT request MUST contain 
the latest TxId from every agent that this agent has 
previously merged datoms from.

Formally: if agent α has merged datoms from agent β up to 
tx_β, then tx_β ∈ causal_parents of α's next TRANSACT.

Violation: Agent α submits a transaction with causal_parents 
that does not include β's latest known tx, creating a false 
impression of concurrency.

INV-TX-BRANCH-001: Branch Isolation
A TRANSACT on branch b MUST NOT affect the trunk or any other 
branch. The datoms are associated with b via :tx/branch.

Formally: visible(trunk) is unchanged by TRANSACT on any branch.

INV-TX-PROVENANCE-001: Provenance Accuracy
The declared provenance type MUST be structurally consistent 
with the transaction content.

A transaction declaring :observed MUST contain at least one 
datom whose value corresponds to a tool observation.

A transaction declaring :inferred MUST reference datoms from 
multiple prior transactions (cross-temporal assertion).

Verification: Structural audit of provenance declarations 
against transaction content.

INV-TX-FRONTIER-DURABLE-001: Frontier Persistence
After a successful TRANSACT, the agent's new frontier MUST 
be durably stored before the response is returned.

Consequence: On crash recovery, the agent can restore its 
frontier from durable storage and resume from its last 
known state.
```

**Merge cascade on TRANSACT:**

When a TRANSACT creates a conflict (two concurrent assertions for a cardinality-one attribute), the system:

1. Asserts a Conflict entity (itself a datom — mediation as datoms)
2. Computes severity = max(w(d₁), w(d₂))
3. Routes via the conflict routing function (automated/agent/human)
4. Fires any TUI subscriptions matching conflict patterns
5. Updates uncertainty measurements for the affected entity
6. Invalidates cached queries that reference the affected entity

All of steps 1–6 produce additional datoms. The conflict detection, severity assessment, and routing decision are themselves recorded in the store as datoms. The deliberation that follows is recorded as Position and Decision entities. The entire resolution process is auditable, queryable, and usable as precedent.

#### OPERATION 2: QUERY

Read datoms via Datalog evaluation.

```
QUERY : QueryRequest → QueryResponse

QueryRequest = {
  expression:     DatalogExpression,      — the Datalog query
  mode:           QueryMode,              — Monotonic | Stratified | Barriered
  frontier:       Option<Frontier>,       — None = local frontier
  stability_min:  Option<Long>,           — min commitment weight
  branch:         Option<BranchId>,       — None = trunk
  budget:         Option<Long>            — max result count
}

QueryResponse = {
  results:        [Tuple],                — the query results
  stability:      Double,                 — min commitment across contributing datoms
  frontier_used:  Frontier,               — the frontier at evaluation time
  access_event:   AccessEventId           — logged for significance tracking
}
```

**Invariants:**

```
INV-QUERY-CALM-001: CALM Compliance
A query in Monotonic mode MUST contain no negation, aggregation, 
or non-monotonic operations.

The query engine MUST reject Monotonic-mode queries that contain 
such operations at parse time.

INV-QUERY-BRANCH-001: Branch Visibility
A query against branch b MUST see exactly:
  visible(b) = {d ∈ trunk | d.tx ≤ b.base-tx} ∪ {d | d.tx.branch = b}

A query against trunk MUST see only committed datoms.

INV-QUERY-SIGNIFICANCE-001: Access Logging
Every query execution MUST generate an access event in the 
access log, recording (query_expression, result_datom_ids, 
timestamp, agent_id).

This access event is NOT a datom in the main store. It is 
recorded in the separate access log for significance computation.

INV-QUERY-DETERMINISM-001: Frontier Determinism
Two queries with identical expressions evaluated against 
identical frontiers MUST return identical results.

Formally: if f₁ = f₂ and q₁ = q₂, then results(q₁, f₁) = results(q₂, f₂).
```

#### OPERATION 3: ASSOCIATE

Schema discovery — the System 1 Phase 1 operation.

```
ASSOCIATE : AssociateRequest → AssociateResponse

AssociateRequest = {
  mode:      SemanticCue String | ExplicitSeeds [EntityId],
  depth:     Long,                       — traversal depth (1-5)
  breadth:   Long,                       — max edges per node
  branch:    Option<BranchId>,           — None = trunk
  significance_weight: Double            — how much significance affects ordering
                                           (0.0 = ignore, 1.0 = dominate)
}

AssociateResponse = {
  neighborhood:   SchemaNeighborhood,
  token_cost:     Long,                  — tokens to represent this neighborhood
  known_projections: [ProjectionId]      — reusable projection patterns that
                                           cover parts of this neighborhood
}
```

**Invariants:**

```
INV-ASSOCIATE-BOUND-001: Bounded Traversal
The number of entities in the returned neighborhood MUST NOT 
exceed depth × breadth.

This ensures ASSOCIATE is always cheap — bounded, predictable 
token cost regardless of store size.

INV-ASSOCIATE-SIGNIFICANCE-001: Significance Bias
When multiple edges are available at a traversal step and 
breadth requires selection, edges incident to higher-significance 
datoms MUST be preferred.

This implements Hebbian strengthening: frequently-queried 
entities are more likely to appear in ASSOCIATE results.

INV-ASSOCIATE-LEARNED-001: Learned Association Traversal
Agent-authored Learned Association entities MUST be traversed 
during ASSOCIATE, alongside structural (ref-type) edges.

An entity reachable only via a learned association (no structural 
path exists) MUST still appear in the neighborhood if it falls 
within the depth/breadth bound.

This is the mechanism by which self-authored associations 
enrich the ASSOCIATE results over time.
```

#### OPERATION 4: ASSEMBLE

Budget-aware context construction — the System 1 Phase 2 operation.

```
ASSEMBLE : AssembleRequest → AssembleResponse

AssembleRequest = {
  query_results:  [Tuple],               — from a preceding QUERY
  neighborhood:   SchemaNeighborhood,    — from a preceding ASSOCIATE
  budget_tokens:  Long,                  — max tokens in assembled context
  priority:       AssemblePriority,      — weighting strategy
  pyramid_levels: Map<EntityId, Level>,  — requested detail level per entity
                                           (optional; system can auto-select)
  include_guidance: Boolean,             — include applicable guidance nodes?
  include_intentions: Boolean            — include agent's current intentions?
}

AssemblePriority = {
  relevance_weight:    Double,           — weight for query relevance
  significance_weight: Double,           — weight for Hebbian significance
  recency_weight:      Double,           — weight for temporal recency
  uncertainty_weight:  Double            — weight for uncertainty (prioritize
                                           uncertain entities? or avoid them?)
}

AssembleResponse = {
  context:           AssembledContext,
  entities_included: Set<EntityId>,      — what made the cut
  entities_excluded: Set<EntityId>,      — what didn't fit (for confusion detection)
  budget_remaining:  Long,
  projection_id:     ProjectionId        — reified projection pattern
}
```

**Invariants:**

```
INV-ASSEMBLE-BUDGET-001: Budget Compliance
The assembled context's token_count MUST NOT exceed budget_tokens.

The system MAY use fewer tokens than budgeted if insufficient 
relevant material exists.

INV-ASSEMBLE-PYRAMID-001: Pyramid Coherence
If entity e is included at pyramid level L, then all entities 
that e has structural (ref-type) dependencies on MUST be 
included at level ≥ L.

This prevents the assembled context from referencing entities 
that aren't present at sufficient detail to understand the 
reference.

INV-ASSEMBLE-INTENTION-001: Intention Anchoring
If include_intentions is true, the agent's active Intention 
entities MUST be included in the assembled context at pyramid 
level 0 (full detail), regardless of budget pressure.

This is the anti-goal-dilution mechanism: the agent's stated 
goals are pinned in context and cannot be evicted by lower-
priority material.

INV-ASSEMBLE-PROJECTION-001: Projection Reification
Every ASSEMBLE invocation MUST record its projection pattern 
(the set of entities, their levels, and the query that generated 
them) as a Projection Pattern entity.

This enables significance tracking on projections and reuse of 
useful assembly patterns.
```

#### OPERATION 5: BRANCH

Create, combine, commit, or abandon branches.

```
BRANCH : BranchRequest → BranchResponse

BranchRequest = 
  | Fork   { name: String, base: Option<TxId>, purpose: String, 
             competing_with: Set<BranchId> }
  | Commit { branch: BranchId, rationale: String }
  | Combine { branches: Set<BranchId>, strategy: CombineStrategy,
              name: String }
  | Rebase  { branch: BranchId, new_base: TxId }
  | Abandon { branch: BranchId, reason: String }
  | Compare { branches: Set<BranchId>, criteria: [ComparisonCriterion] }

CombineStrategy = 
  | Union                                — G-Set union (take everything)
  | SelectiveUnion [AttrId]             — take datoms only for specified attrs
  | ConflictToDeliberation              — conflicts trigger deliberation

ComparisonCriterion =
  | FitnessScore
  | TestSuite TestId
  | UncertaintyReduction
  | AgentReview AgentId
  | Custom DatalogExpression

BranchResponse = {
  branch:          BranchId,             — the resulting branch
  conflicts:       [ConflictId],         — conflicts detected on combine/commit
  comparison:      Option<ComparisonId>, — results of comparison
  deliberation:    Option<DeliberationId> — opened deliberation (if conflicts)
}
```

**Invariants:**

```
INV-BRANCH-FORK-001: Fork Snapshot Isolation
A forked branch's base snapshot MUST be immutable. Subsequent 
trunk commits MUST NOT be visible to the branch unless the 
branch explicitly rebases.

INV-BRANCH-COMMIT-001: Commit Monotonicity
Committing a branch to trunk MUST be a monotonic operation:
  trunk' = trunk ∪ datoms(branch)
No trunk datoms are removed or modified.

INV-BRANCH-COMPETE-001: Competing Branch Registration
When a branch is forked with competing_with set, ALL branches 
in the competing set MUST be notified. The system MUST prevent 
any of the competing branches from committing until either:
  (a) A comparison has been performed, or
  (b) A deliberation has been opened and decided.

This prevents the "race to commit" problem where the first 
branch to commit wins by default, bypassing quality comparison.

INV-BRANCH-DELIBERATION-001: Conflict-to-Deliberation Escalation
When COMBINE detects conflicts and the CombineStrategy is 
ConflictToDeliberation, the system MUST open a Deliberation 
entity, create Position entities for each conflicting branch, 
and notify participating agents via the SIGNAL operation.

The combine operation blocks until the deliberation is decided.
```

#### OPERATION 6: MERGE

Receive datoms from a remote agent. Distinct from BRANCH operations — this is the CRDT merge for inter-agent synchronization.

```
MERGE : MergeRequest → MergeResponse

MergeRequest = {
  remote_agent:    AgentId,
  remote_datoms:   [Datom],              — datoms the local agent doesn't have
  remote_frontier: Frontier              — the remote agent's frontier
}

MergeResponse = {
  new_datoms:           Long,            — count of genuinely new datoms
  new_conflicts:        [ConflictId],    — conflicts detected post-merge
  invalidated_queries:  [QueryCacheKey], — cached queries now stale
  uncertainty_deltas:   [(EntityId, DeltaU)], — uncertainty changes
  stale_projections:    [ProjectionId],  — projections that need recomputation
  updated_frontier:     Frontier         — agent's new frontier after merge
}
```

**Invariants:**

```
INV-MERGE-CRDT-001: G-Set Union Semantics
MERGE MUST compute the set union of local datoms and remote 
datoms. Duplicate datoms (identical [e a v tx op]) are 
deduplicated.

Formally: store' = store ∪ remote_datoms

INV-MERGE-IDEMPOTENT-001: Idempotence
Merging the same set of remote datoms twice MUST produce the 
same store state.

Formally: merge(merge(S, R), R) = merge(S, R)

This property, combined with at-least-once delivery, gives 
exactly-once semantics.

INV-MERGE-FRONTIER-001: Frontier Advancement
After merge, the local frontier MUST be advanced to include 
the remote agent's latest tx.

Formally: frontier'[remote_agent] = max(frontier[remote_agent], 
  remote_frontier[remote_agent])

INV-MERGE-CASCADE-001: Post-Merge Cascade
After merging new datoms, the system MUST:
  1. Detect new conflicts (concurrent cardinality-one assertions)
  2. Invalidate cached query results that reference affected entities
  3. Mark stale any projection patterns that include affected entities
  4. Recompute uncertainty for affected entities
  5. Fire TUI subscriptions matching affected datoms

Steps 1-5 are recorded as datoms (conflict entities, uncertainty 
measurements, etc.), making the merge cascade itself auditable.
```

#### OPERATION 7: SYNC-BARRIER

Establish a consistent cut across a set of agents.

```
SYNC-BARRIER : BarrierRequest → BarrierResponse

BarrierRequest = {
  participants:    Set<AgentId>,         — agents that must synchronize
  timeout:         Duration,             — max wait time
  purpose:         String,               — why this barrier
  required_for:    Option<DatalogExpression> — the non-monotonic query that needs this
}

BarrierResponse = {
  barrier_id:      BarrierId,
  consistent_cut:  Frontier,             — the agreed frontier across all participants
  timestamp:       Instant,
  all_synced:      Boolean,              — true if all participants responded
  missing:         Set<AgentId>          — agents that timed out
}
```

**Invariants:**

```
INV-BARRIER-CONSISTENCY-001: Consistent Cut
The frontier returned by a sync barrier MUST satisfy:
  for all agents α, β ∈ participants:
    consistent_cut[α] is known to β
    consistent_cut[β] is known to α

Formally: the consistent cut is a down-set in the causal partial 
order that all participants agree on.

INV-BARRIER-TIMEOUT-001: Bounded Waiting
A sync barrier MUST resolve within the specified timeout.
If any participant fails to respond, the barrier completes with 
all_synced=false and the missing set populated.

Non-responding agents are excluded from the consistent cut. The 
barrier is still valid for the responding participants, but 
queries that depend on the missing agents' data cannot be trusted.

INV-BARRIER-RECORDING-001: Barrier as Datom
Every completed sync barrier MUST be recorded as a datom:
  (barrier_entity, :barrier/participants, participant_set)
  (barrier_entity, :barrier/frontier, consistent_cut)
  (barrier_entity, :barrier/timestamp, instant)

This makes barriers queryable: "when was the last time agents 
α and β were synchronized?"

INV-BARRIER-CRASH-RECOVERY-001: Crash During Barrier
If an agent crashes during a barrier, the barrier timeout fires.
The barrier completes without the crashed agent.
On recovery, the crashed agent can query the barrier record to 
learn what frontier the other agents agreed on, and merge to 
reach that state.
```

#### OPERATION 8: SIGNAL

Emit a coordination signal. Signals are the backward channel from System 2 to System 1 and from agents to the coordination layer.

```
SIGNAL : SignalRequest → SignalResponse

SignalRequest = {
  signal:     Signal,
  source:     AgentId,
  target:     SignalTarget,              — who should receive this
  priority:   Long                       — urgency (affects TUI alerting)
}

SignalTarget = 
  | Coordinator                          — the meta-agent / orchestrator
  | Specific AgentId                     — a particular agent
  | Broadcast                            — all agents
  | Subscribers                          — anyone subscribed to this signal type
  | Human                                — the TUI

SignalResponse = {
  signal_id:    DatomId,                 — the signal is itself a datom
  delivered_to: Set<AgentId>,            — who received it
  ack:          Map<AgentId, Instant>    — acknowledgments (for reliable signals)
}
```

**Invariants:**

```
INV-SIGNAL-DATOM-001: Signal as Datom
Every signal MUST be recorded as a datom in the store, with 
the full signal content, source, target, and delivery status.

This makes the coordination history queryable: "how many 
confusion signals has agent α emitted in the last 100 
transactions?" is a Datalog query over signal datoms.

INV-SIGNAL-CONFUSION-001: Confusion Triggers Re-Assembly
A Confusion signal MUST trigger the following sequence:
  1. System 1 re-runs ASSOCIATE with the confusion cue
  2. ASSEMBLE constructs a new context from the new neighborhood
  3. The agent re-evaluates with the enriched context

This sequence MUST complete within a single agent cycle — the 
confusion signal does not require a full trip through the 
protocol. It's an intra-cycle optimization.

INV-SIGNAL-GOAL-DRIFT-001: Goal Drift Notification
A GoalDrift signal MUST be delivered to:
  (a) The drifting agent itself (for self-correction)
  (b) The TUI (for human awareness)
  (c) Any supervising agent (the agent referenced in 
      :agent/parent, if present)
```

#### OPERATION 9: SUBSCRIBE

Register for push notifications on datom patterns.

```
SUBSCRIBE : SubscribeRequest → SubscribeResponse

SubscribeRequest = {
  pattern:        DatalogExpression,     — what to watch for
  subscriber:     AgentId | TuiId,       — who receives notifications
  priority:       Long,                  — notification priority
  debounce:       Option<Duration>       — minimum time between notifications
}

SubscribeResponse = {
  subscription_id: SubscriptionId,
  active:          Boolean
}

-- Notifications are delivered as:
Notification = {
  subscription_id: SubscriptionId,
  matching_datoms: [Datom],
  timestamp:       Instant
}
```

**Invariants:**

```
INV-SUBSCRIBE-COMPLETENESS-001: No Missed Notifications
A subscription MUST fire for every TRANSACT that produces 
datoms matching the subscription pattern.

Formally: if datom d matches pattern p, and subscription s 
watches pattern p, then s fires within one refresh cycle of 
d's assertion.

INV-SUBSCRIBE-DEBOUNCE-001: Debounce Compliance
If debounce is set, notifications MUST NOT fire more frequently 
than the debounce interval. Multiple matching datoms within a 
debounce window are batched into a single notification.
```

#### OPERATION 10: GUIDANCE

Query the available action topology from the current state.

```
GUIDANCE : GuidanceRequest → GuidanceResponse

GuidanceRequest = {
  agent:          AgentId,
  context:        AssembledContext,       — the agent's current context
  lookahead:      Long,                  — how many steps ahead to plan (1-5)
  include_learned: Boolean               — include agent-generated guidance?
}

GuidanceResponse = {
  available_actions: [GuidanceAction],   — what can be done right now
  lookahead_tree:    Option<GuidanceTree>, — future states if lookahead > 1
  active_intentions: [IntentionId],      — agent's current intentions (for alignment)
}

GuidanceTree = {
  action:   GuidanceAction,
  children: [GuidanceTree],              — possible next states after this action
  score:    Double                        — estimated value (from comonadic extend)
}
```

**The comonadic extend is implemented in the lookahead:**

```
extend_guidance : (State → [Action]) → State → GuidanceTree
extend_guidance f state =
  let actions = f(state)
  for each action a in actions:
    let state' = simulate_action(state, a)
    let subtree = extend_guidance f state'  -- recursive, bounded by lookahead
    yield GuidanceTree(a, subtree, score(state'))
```

The simulation doesn't execute actions against the real runtime — it speculatively applies them to a hypothetical store state (a virtual branch) and evaluates the resulting guidance. This is **planning as branch simulation**.

```
INV-GUIDANCE-ALIGNMENT-001: Intention-Guidance Coherence
Actions returned by GUIDANCE MUST be scored higher if they 
advance the agent's active intentions.

Formally: for action a and intention i:
  if postconditions(a) ∩ goals(i) ≠ ∅:
    score(a) += intention_alignment_bonus

This keeps the guidance aligned with the agent's stated goals, 
preventing the goal dilution the document identified.

INV-GUIDANCE-LEARNED-001: Learned Guidance Integration
If include_learned is true, agent-authored Guidance Nodes MUST 
be included in the results alongside system-default guidance.

Learned guidance nodes are ranked by their :guidance/effectiveness 
score, which is updated based on empirical outcomes.
```

---

## Operation Composition: The Agent Cycle

These ten operations compose into a standard agent cycle. Here's the complete flow:

```
AGENT CYCLE (one iteration):

1. ASSOCIATE(current_semantic_context, depth=2, breadth=5)
   → neighborhood

2. QUERY(task_relevant_expression, Monotonic, local_frontier)
   → relevant_datoms

3. ASSEMBLE(relevant_datoms, neighborhood, budget, 
            include_guidance=true, include_intentions=true)
   → context

4. GUIDANCE(agent, context, lookahead=2)
   → available_actions, lookahead_tree

5. Agent policy π evaluates context + guidance → action_or_confusion

6a. If action:
    Execute action against runtime R
    TRANSACT(observation_datoms + inference_datoms, provenance)
    If on a branch: datoms go to branch
    If on trunk: datoms go to trunk

6b. If confusion:
    SIGNAL(Confusion(cue), source=self, target=Subscribers)
    Re-run ASSOCIATE with confusion cue (wider depth/breadth)
    Re-run ASSEMBLE with new neighborhood
    Retry step 5 with enriched context

7. If agent detects learned association:
    TRANSACT(association_datom, provenance=:inferred)

8. If agent completes subtask:
    Update intention progress
    TRANSACT(intention_update, provenance=:derived)

9. Check for incoming signals (MERGE requests, conflict 
   notifications, deliberation invitations)
   Process as needed

10. Repeat from step 1
```

Steps 1-4 are System 1 (fast, associative, context assembly). Step 5 is System 2 (slow, deliberative, the LLM). Steps 6-9 are the action/observation cycle. The confusion feedback (6b) is the System 2 → System 1 backward channel the document identified as critical.

---

## Schema Additions Summary

The protocol introduces these new entity types beyond what we defined in Layers 0-5:

| Entity Type | Layer | Purpose |
|---|---|---|
| Branch | Protocol | Patch branch metadata |
| Branch Comparison | Protocol | Competing branch evaluation |
| Deliberation | Protocol | Mediation process |
| Position | Protocol | Agent stance in deliberation |
| Decision | Protocol | Deliberation outcome |
| Projection Pattern | Protocol | Reified assembly patterns |
| Projection Level | Protocol | Pyramid summary cache |
| Datom Significance | Protocol | Hebbian access tracking |
| Guidance Node | Protocol | Available action topology |
| Guidance Action | Protocol | Specific available action |
| Belief | Metacognitive | Agent's subjective belief |
| Intention | Metacognitive | Agent's current goal/plan |
| Learned Association | Metacognitive | Agent-authored edge |
| Strategic Heuristic | Metacognitive | Agent-authored strategy |
| TUI Configuration | Interface | Human display preferences |

This brings the total schema to approximately **46 entity types, ~300 attributes, and ~16 lattice definitions.**

---

## One Remaining Uncertainty

I have one question before I can consider this protocol complete. It concerns the **frontier exchange topology.**

When agents exchange frontiers (the prerequisite for MERGE), what topology do they use?

**Option A — Gossip protocol.** Each agent periodically exchanges frontiers with a random subset of peers. Information propagates eventually but not immediately. Simple, robust, scalable. No central point of failure.

**Option B — Coordinator-mediated.** All frontier exchanges go through a central coordinator (the meta-agent or a dedicated sync service). The coordinator maintains the global frontier and distributes deltas. Simple to reason about, but the coordinator is a bottleneck and single point of failure.

**Option C — Topology-dependent.** The frontier exchange topology matches the agent topology. In a hierarchy, agents exchange with their parent/children. In a swarm, agents gossip. In a ring, agents exchange with neighbors. The protocol doesn't prescribe — it provides the MERGE and FRONTIER-EXCHANGE primitives, and the deployment chooses.

My instinct is Option C — consistent with our principle that topology emerges rather than being prescribed. But I want to confirm, because the choice affects the liveness guarantees of the sync barrier operation. In a gossip topology, barriers may take O(log N) rounds to establish. In a coordinator topology, they take O(1) rounds. If you need fast barriers, the topology matters.

What's your preference?

---

## Willem

C for sure. Do you think my desire to make the event log the runtime is taking things too far? What are the practical implications? Assume that I am running using primarily Claude Code as my agentic harness for ideation and collaboration with the AI agents, running on a POSIX compliant Ubuntu distro on a remote VPS. I want a "cleanroom software engineering" approach: fully grounded in "formal methods", "spec-driven-design" and abstract algebra. Use "first principles" thinking at every step. Do NOT make assumptions -- if you encounter uncertainty, ask me clarifying questions.

---

## Claude

This is the right question to ask now, before we go further. Let me work through it rigorously.

---

## The Claim Under Examination

The claim is: **the datom store is the protocol-level runtime; all state visible to the protocol exists as datoms.**

I formalized this as INV-STORE-AS-RUNTIME-001. But I need to stress-test it against your actual operating environment before I can endorse it or recommend softening it.

Your environment:

- **Claude Code** as the agentic harness (LLM + tool-calling)
- **Ubuntu VPS** (POSIX filesystem, shell, processes)
- **Multiple Claude Code instances** (or sessions) acting as agents in a swarm
- **You** interacting via Claude Code's conversational interface and the TUI we've designed

---

## First Principles: What Does "Runtime" Mean?

In the tripartite (E, R, A) decomposition from the formal analysis document:

- **R** is the opaque state machine: `δ : S × Op → S × Obs`
- The agent interacts with R only through operations and observations
- R's state S is enormous: the entire filesystem, process table, network stack, memory

The claim "E is the runtime" would mean: **S (or the protocol-relevant subset of S) is fully represented in E.** The agent doesn't need to query R to know the current state — it queries E.

This is a **materialized view** claim. The datom store is a materialized view of the relevant subset of R's state.

### When This Is True

For facts the agents produce and consume *within the protocol*, this is naturally true. Spec elements, invariants, decisions, uncertainty measurements, deliberation records, branches, guidance — all of these are born as datoms and exist only as datoms. The datom store IS the source of truth for these. R merely persists the bytes.

### When This Is Not True

For facts about the *external world* that agents observe through tools, there's an impedance mismatch. Consider:

```
Agent reads file /home/willem/project/src/main.rs
Agent observes: file contains 347 lines, function handle_request at line 42
Agent asserts datoms: 
  (:file/main-rs :line-count 347 :tx-17)
  (:file/main-rs :contains-function :fn/handle-request :tx-17)
```

Five seconds later, a different process (or Willem via vim) edits main.rs. The file now has 352 lines and handle_request has moved to line 48. The datom store says 347 lines. The filesystem says 352. **The datom store is stale.**

This is the frame problem the document formalized: exogenous events (Exo) mutate S without generating datoms.

---

## The Practical Implications in Your Environment

Let me trace through what "datom store as runtime" means concretely for Claude Code on Ubuntu.

### Implication 1: File State Must Be Datom-Mediated

**Strong version** (datom store = runtime): Every file the agents care about has its state represented as datoms. Before acting on a file, the agent queries the datom store, not the filesystem. File reads go through the store; file writes go through TRANSACT.

**Practical problem:** This requires either:
- (a) A daemon that watches the filesystem and asserts datoms for every change (inotify → datom pipeline), or
- (b) Agents re-reading files on every cycle and diffing against stored state, or  
- (c) Accepting that datom state may be stale and treating filesystem reads as "cache misses" that trigger fresh assertions

Option (a) is robust but heavy — you'd need a persistent process on the VPS watching every relevant directory and converting filesystem events to datoms in real-time. This is feasible (inotifywait + a small daemon + write to the datom store) but it's infrastructure you have to build and maintain.

Option (b) is wasteful — reading files just to check if they changed is O(file_size) on every cycle.

Option (c) is the practical answer, and it's what I actually recommend. But it means the datom store is *not* the runtime — it's a **cache** of observations about the runtime, with an explicit staleness model.

### Implication 2: Claude Code's Session State

Claude Code maintains its own state: the JSONL conversation log, the context window contents, tool results, system prompts. This state is partially in-memory (the active session) and partially on disk (the JSONL log files).

**Strong version:** Claude Code's session state is represented as datoms. The conversation history, tool call/result pairs, and context assembly decisions are all queryable through the protocol.

**Practical problem:** Claude Code doesn't expose its internal state as a structured API. You interact with it through its conversational interface. To get its session data into the datom store, you'd need to:
- Parse the JSONL session logs after each interaction
- Extract datoms from the parsed events
- Assert them into the store

This is doable — you're already generating JSONL logs during discovery sessions. But it's a **post-hoc extraction**, not a live materialization. There's always a lag between what Claude Code has done and what the datom store knows about.

### Implication 3: Process Coordination

If you're running multiple Claude Code instances as agents in a swarm, they need to coordinate. Currently, Claude Code instances are independent processes with no shared state beyond the filesystem.

**Strong version:** All inter-agent communication goes through the datom store. Agent α asserts datoms; agent β merges them. No direct inter-process communication.

**Practical reality:** This actually works well. The datom store (backed by files on the shared filesystem) is a natural shared-nothing coordination mechanism. Each Claude Code instance reads from and writes to the store. The CRDT merge semantics handle concurrent writes. This is the strongest argument FOR the datom-store-as-runtime model — for inter-agent coordination, the store genuinely IS the only communication channel.

### Implication 4: Tool Execution

Claude Code executes tools (bash commands, file reads/writes, web searches) against the POSIX runtime. These tool executions have side effects on R that the datom store doesn't know about until the agent asserts the results.

**Strong version:** Tool results are always asserted as datoms before being acted upon. The agent's policy π conditions on the datom-mediated view, not on raw tool output.

**Practical problem:** This creates a double-handling overhead. Claude Code already receives tool results in its context window. Forcing those results through the datom store before the agent can reason about them adds latency and complexity. The tool result is already IN the agent's context — requiring it to also be a datom before the agent can use it is architecturally pure but operationally wasteful.

---

## My Assessment: You're Not Going Too Far, But You Need a Precise Boundary

The instinct is right. The direction is right. But the formulation needs refinement. Here's what I think the correct version is.

### The Refined Claim

The datom store is the **protocol-level state**. All coordination, specification, deliberation, and metacognitive state exists as datoms. This is strict and non-negotiable.

The POSIX runtime is the **execution-level state**. File contents, process state, network responses exist in R. Agents observe R through tools and assert their observations as datoms.

The boundary between them is the **observation interface**: the set of tool operations that translate R-state into datoms.

```
┌─────────────────────────────────────────────────┐
│                 PROTOCOL LAYER                   │
│  (datom store — all coordination state here)     │
│                                                  │
│  specs, invariants, decisions, deliberations,    │
│  branches, uncertainty, authority, guidance,      │
│  intentions, beliefs, learned associations,       │
│  conflict resolutions, projections, signals       │
│                                                  │
│  ← This IS the runtime for the protocol →        │
├─────────────────────────────────────────────────┤
│            OBSERVATION INTERFACE                  │
│  (tools that read R and assert datoms)           │
│                                                  │
│  file-read → assert file-state datoms            │
│  bash-exec → assert command/result datoms        │
│  test-run  → assert test-outcome datoms          │
│  git-status → assert repo-state datoms           │
│                                                  │
│  ← Translates R-state into protocol-state →      │
├─────────────────────────────────────────────────┤
│              EXECUTION LAYER                     │
│  (POSIX runtime — the physical substrate)        │
│                                                  │
│  filesystem, shell, processes, network,          │
│  Claude Code session state, git repos            │
│                                                  │
│  ← Opaque to the protocol except via obs →       │
└─────────────────────────────────────────────────┘
```

### Why This Is the Right Boundary

**Algebraic justification:** The protocol layer forms a **closed algebraic structure** — a G-Set with branching, queries, projections, and the full apparatus we've designed. Every operation within this layer is formally specified with invariants. The CRDT properties, CALM compliance, and monotonicity guarantees all hold within this layer.

The execution layer is **not** a closed algebraic structure. The POSIX filesystem doesn't have CRDT semantics. Two concurrent writes to the same file don't resolve via lattice join — the last writer wins, silently. Process state is mutable and ephemeral. Network responses are non-deterministic. Trying to force CRDT semantics onto the filesystem would be fighting the substrate.

The observation interface is a **functor** from the execution-layer category to the protocol-layer category:

```
observe : R-State → [Datom]

Properties:
  — Idempotent: observing the same R-state twice produces 
    equivalent datoms (modulo transaction id)
  — Monotonic: observations only ADD datoms, never remove them
    (if a file is deleted, you assert a retraction datom, 
     not a deletion)
  — Lossy: not all R-state is observed — only what the agent 
    chooses to observe (selective observation per the document's 
    V(X) information-value criterion)
```

### The Staleness Model

The critical addition this boundary requires: an explicit model of **when datom-state diverges from R-state** and what to do about it.

```
ENTITY TYPE: Observation Metadata

:observation/entity      ref      :one   :lww   — the entity this observes
:observation/source      keyword  :one   :lww   — :filesystem, :shell, :network, 
                                                   :git, :process
:observation/path        string   :one   :lww   — the R-state path observed 
                                                   (e.g., file path)
:observation/timestamp   instant  :one   :lww   — when the observation was made
:observation/hash        string   :one   :lww   — content hash at observation time
:observation/stale-after instant  :one   :lww   — expected staleness horizon
                                                   (configurable per source)
```

An observation datom carries metadata about when it was made and when it's expected to become stale. The ASSEMBLE operation can use this:

```
INV-ASSEMBLE-FRESHNESS-001: Staleness Awareness
When ASSEMBLE includes datoms derived from observations, it 
MUST check whether the observation timestamp plus stale-after 
exceeds the current time.

If the observation is stale, ASSEMBLE MUST either:
  (a) Include a staleness warning in the assembled context, or
  (b) Trigger a re-observation (tool read) before including the 
      datom, depending on the freshness-mode configuration.

Default freshness-mode: :warn (include with warning).
Alternative: :refresh (re-observe before including).
Alternative: :accept (include without warning — for datoms 
  about entities that change rarely, like schema definitions).
```

This gives you the benefit of the datom-store-as-runtime model (everything queryable, everything in the protocol) while being honest about the parts of reality that the store might be wrong about.

---

## Practical Architecture for Your Environment

Given Claude Code on Ubuntu VPS, here's what the architecture looks like concretely:

### The Datom Store

**Implementation:** A directory on the VPS filesystem containing:
- `trunk.ednl` — the main append-only datom log (one datom per line)
- `branches/{name}.ednl` — per-branch datom logs
- `access.log` — the access log for significance tracking (separate from main store)
- `frontier.json` — each agent's durable frontier
- `indexes/` — materialized indexes (EAVT, AEVT, VAET, AVET, LIVE)

The indexes are rebuilt from the datom logs on startup and maintained incrementally on each TRANSACT. For a VPS with a single-digit number of agents and thousands to tens-of-thousands of datoms, a file-backed store with in-memory indexes is entirely sufficient. No database server required.

**Git integration:** The entire datom store directory is in git. Every TRANSACT that hits trunk is a git commit. Branches in the datom store map to git branches (or to files within the datom directory — either works). This gives you VCS-native history, diffability, and the durable temporal indexing you want.

```
.ddis/
├── store/
│   ├── trunk.ednl           # append-only trunk datoms
│   ├── branches/
│   │   ├── impl-auth.ednl   # branch: auth implementation
│   │   └── spec-rev-3.ednl  # branch: spec revision 3
│   ├── access.log           # significance tracking
│   ├── frontier.json        # per-agent frontiers
│   └── indexes/             # materialized, gitignored
│       ├── eavt.idx
│       ├── aevt.idx
│       ├── vaet.idx
│       ├── avet.idx
│       └── live.idx
├── projections/
│   ├── patterns.ednl        # reified projection patterns
│   └── cache/               # pyramid summaries, gitignored
├── guidance/
│   └── topology.ednl        # guidance graph
└── spec/                    # the actual DDIS spec output
    ├── CONSTITUTION.md
    ├── invariants/
    ├── adrs/
    └── sections/
```

### The Claude Code Integration

Each Claude Code instance is an agent. It needs tools that implement the protocol operations. These tools are **bash scripts or small programs** that Claude Code can invoke through its tool-calling interface:

```bash
# Tool: ddis-transact
# Appends datoms to trunk or branch, updates frontier, 
# detects conflicts, fires subscriptions
ddis-transact --agent alice --provenance observed \
  --branch impl-auth \
  --datom '[:file/main-rs :line-count 347]' \
  --datom '[:file/main-rs :contains-fn :fn/handle-request]' \
  --rationale "Read main.rs for auth implementation"

# Tool: ddis-query
# Evaluates Datalog against the store
ddis-query '[:find ?e ?v :where [?e :line-count ?v]]'

# Tool: ddis-associate
# Schema discovery
ddis-associate --cue "auth implementation error handling" \
  --depth 2 --breadth 5

# Tool: ddis-assemble
# Budget-aware context construction
ddis-assemble --query-results /tmp/last-query.json \
  --budget 4000 --include-guidance --include-intentions

# Tool: ddis-branch
# Branch operations
ddis-branch fork --name impl-auth-v2 \
  --purpose "Alternative auth approach using JWT" \
  --competing-with impl-auth-v1

# Tool: ddis-merge
# CRDT merge from another agent's frontier
ddis-merge --from bob

# Tool: ddis-signal
# Emit coordination signal
ddis-signal confusion --cue "can't find the OAuth config schema" \
  --target subscribers

# Tool: ddis-guidance
# Query available actions
ddis-guidance --lookahead 2
```

These tools are the observation interface. Claude Code doesn't know about datoms natively — it knows about bash commands. The tools translate between Claude Code's tool-calling world and the datom protocol.

### The Critical Practical Constraint

Here's where I need to be direct about what works and what doesn't.

**What works today:** Claude Code can call bash scripts as tools. Those scripts can read/write files on the VPS filesystem. A datom store backed by append-only files with shell-accessible query tools is immediately buildable. Multiple Claude Code instances on the same VPS can share the filesystem. Git tracks the history.

**What requires engineering:** The query engine. Evaluating Datalog against a file-backed EAV store requires either:
- (a) A purpose-built query tool (a Rust or Go binary that loads the datom files, builds indexes in memory, evaluates Datalog)
- (b) A lightweight embedded engine (SQLite with an EAV schema and Datalog-to-SQL compilation)
- (c) A line-processing approach (grep/jq for simple queries, escalating to a proper engine for joins and transitive closure)

Option (a) is what the full spec calls for. It's also the thing we're specifying: the Datomic-in-Rust system. This is a bootstrapping problem — you need the system to build the system.

Option (c) is where you start. For the initial deployment, the query tools can be surprisingly simple:

```bash
# Simple entity lookup: grep
grep ':file/main-rs' trunk.ednl

# Attribute scan: grep + jq
grep ':line-count' trunk.ednl | jq -r '.v'

# Two-hop join: small Python script
python3 ddis-query.py '[:find ?e ?v 
  :where [?e :depends-on ?dep] [?dep :status :broken]]'
```

The 50-line Python script from the other conversation's final section is the right starting point. It gets you functional queries over flat files. The Rust engine replaces it when the datom count outgrows what Python can handle in memory.

### The Multi-Agent Coordination

Multiple Claude Code instances on the same VPS coordinate through the shared filesystem:

```
Agent Alice (Claude Code instance 1):
  — Working on branch impl-auth-v1
  — Writes to .ddis/store/branches/impl-auth-v1.ednl
  — Reads from .ddis/store/trunk.ednl + its branch

Agent Bob (Claude Code instance 2):
  — Working on branch impl-auth-v2
  — Writes to .ddis/store/branches/impl-auth-v2.ednl
  — Reads from .ddis/store/trunk.ednl + its branch

Merge trigger:
  — Either agent runs ddis-merge to pull the other's branch
  — Or a coordinator script runs periodically and merges both

Sync barrier:
  — Agent Alice runs ddis-barrier --participants alice,bob
  — The script writes a barrier request file
  — Bob's next cycle detects the barrier request and responds
  — Both agents' frontiers are synchronized
  — The barrier completes when both have responded or timeout
```

File-locking (flock) handles concurrent writes to the same file. The append-only structure means concurrent appends to trunk.ednl don't conflict at the filesystem level — both appends succeed, both datoms are recorded. The CRDT merge is just "concatenation of log files," which the filesystem handles natively.

---

## The Direct Answer to Your Question

No, you're not going too far. The instinct to make the datom store the center of the architecture is correct. But the precise formulation matters:

**The datom store is the runtime for the protocol.** All coordination, specification, and metacognitive state is datom-native. Agents that want to participate in the protocol MUST interact through datom operations. This is non-negotiable and algebraically clean.

**The POSIX runtime is the substrate for the datom store and the execution environment for tools.** It exists, it's necessary, and agents interact with it through tool calls. But tool interactions are ephemeral unless they're asserted as datoms. The filesystem is the persistence layer, not the query layer.

**The observation interface bridges the two.** When agents read files, run tests, or execute commands, the relevant results are asserted as datoms with observation metadata (timestamp, hash, staleness horizon). This makes them queryable, coordinatable, and subject to the full protocol apparatus — while being honest that they're observations of an external reality that may have changed.

The refined invariant:

```
INV-STORE-AS-RUNTIME-002: Protocol State Completeness (Revised)

All state that the protocol reasons about MUST exist as datoms.

State that exists only in the POSIX runtime is invisible to the 
protocol. It becomes visible when an agent observes it and 
asserts observation datoms.

The POSIX runtime MUST NOT be directly queried by protocol 
operations. Protocol operations (QUERY, ASSOCIATE, ASSEMBLE, 
GUIDANCE) operate exclusively on the datom store.

Tool execution (reading files, running commands) is an 
agent-level operation that produces observations. These 
observations enter the protocol via TRANSACT.

Consequence: The datom store is eventually consistent with 
the POSIX runtime, with lag bounded by the observation 
frequency of participating agents. The system is correct 
with respect to the last-observed state, not the current 
state. Staleness is tracked and surfaced.

This is not a weakness — it's a faithful model of bounded 
rational agents operating in a partially observable 
environment. The same model applies to human engineers: 
they act on their last observation of the codebase, not 
on its current state.
```

This is the formulation I endorse. It gives you the algebraic closure you want (the protocol is self-contained over datoms) without requiring you to solve the impossible problem of keeping a materialized view perfectly synchronized with a mutable filesystem.

Shall I proceed to formalize the concrete tool implementations — the bash scripts / small programs that implement the ten protocol operations for Claude Code on your VPS?

---

## Willem

I want the option a) approach. Before we jump into formalizing implementations, I want to ensure that we also account for the k* decay in LLM context windows in designing the actual interface by which an agentic harness like claude code actually engages with our system. I've attached a deep analysis of techniques and observations about prompt optimization to help ground your analysis. I want this tool and the interactions that agents have with it to be primarily mediated by the CLI (though I am open to a small MCP tool/prompt server if advantageous) and auto-prompting guidance. Perhaps I'm missing something in this formulation of the interface (TUI, CLI, MCP(?), Auto-prompting Guidance)? 

I want a "cleanroom software engineering" approach: fully grounded in "formal methods", "spec-driven-design" and abstract algebra. Use "first principles" thinking at every step. Do NOT make assumptions -- if you encounter uncertainty, ask me clarifying questions.

---

## Claude

This is where the prompt optimization framework and the datom store architecture converge in a way that I think neither was designed for individually but that falls out naturally from first principles. Let me work through it layer by layer.

---

## The Fundamental Tension

An LLM agent has a **finite, decaying attention budget** (k\*). Every tool invocation returns output that enters the context window and competes for that budget. The datom store potentially contains thousands of datoms. The naive interface — "run a query, dump the results" — will destroy the agent's k\* within a few cycles.

The interface must therefore solve a **rate-distortion problem**: maximize the information value delivered to the agent while minimizing the attention cost. This is exactly the ASSEMBLE operation from the protocol, but applied to the *interface itself*.

Here's the formal statement:

```
Let I(tool_output) be the information value of a tool's output
    for the agent's current decision.
Let A(tool_output) be the attention cost (tokens consumed in 
    the context window).
Let k*_eff(t) be the agent's effective attention budget at 
    conversation turn t.

The interface must satisfy:

  ∀ tool invocation at turn t:
    A(tool_output) ≤ k*_eff(t) × budget_fraction

  where budget_fraction is the proportion of remaining k* 
  that a single tool call should consume (configurable, 
  default ≈ 0.05 — a single tool call should use ≤ 5% 
  of remaining budget).

  Subject to:
    I(tool_output) is sufficient for the agent to make its 
    next decision without a follow-up query.
```

This constraint has a concrete implication: **the same query must return different output at turn 5 vs. turn 45.** At turn 5, the agent has full k\* budget — it can absorb detailed output. At turn 45, k\*_eff has shrunk — the tool must compress aggressively or the output becomes parasitic context.

---

## The Five Interface Layers

You identified four: TUI, CLI, MCP, Auto-prompting Guidance. I believe there are five, and the one you're missing is the most important for k\* management.

### Layer 0: Ambient Awareness (CLAUDE.md)

This is the layer the prompt optimization document calls "ambient context" — **permanent, low-intensity, k\*-exempt**. It costs ~20 tokens per tool and ensures the agent *knows the tools exist* without consuming meaningful budget.

From the document: "agents fail to invoke available tools 56% of the time without ambient awareness." Without Layer 0, agents won't call ddis tools even when they should.

```
# In CLAUDE.md (the agent's system prompt / project instructions):

## DDIS Datom Store
You have access to `ddis` — a specification store for the project.
Use `ddis` for all spec reads/writes. Key commands:
  ddis query    — read datoms (Datalog)
  ddis associate — discover schema neighborhood
  ddis transact — assert/retract datoms
  ddis status   — current state, guidance, uncertainty
  ddis branch   — manage patch branches
Run `ddis help` for full reference. Run `ddis guidance` for 
recommended next actions.
```

That's ~80 tokens. It's permanent. It never competes with k\* because it's in the system prompt, which is processed once and forms the base field configuration. It satisfies the prompt optimization principle: "Keep awareness permanent; load activation on demand; shed activation when absorbed."

**Critical design point:** Layer 0 does NOT explain how the datom store works, what Datalog is, or what CRDTs are. That information is *active context* that should only be loaded when needed (and shed when absorbed). Layer 0 is *awareness*, not *activation*.

### Layer 1: CLI (The Query Engine Interface)

The CLI binary is the **substrate** — the computation layer that operates on the datom store. It's a Rust binary that:
- Loads datom files from disk
- Builds/maintains in-memory indexes
- Evaluates Datalog queries
- Computes ASSOCIATE neighborhoods
- Performs ASSEMBLE with budget constraints
- Manages branches, merges, frontiers
- Generates guidance

The CLI's output is what enters the agent's context window when called via bash. Therefore, **the CLI must be k\*-aware by default.**

Here's the key design principle:

```
INV-CLI-BUDGET-001: Output Budget Compliance
Every CLI command MUST accept an optional --budget <tokens> flag 
that caps the output size.

If --budget is not specified, the CLI reads the current session's 
budget from .ddis/session.json (which tracks estimated k*_eff).

If no session exists, the CLI defaults to a conservative budget 
(default: 500 tokens).

No CLI command may produce output exceeding the budget unless 
explicitly overridden with --budget unlimited (for human use 
or piped processing).

The budget is enforced by the output formatter, which applies 
pyramid-level selection to compress results that would exceed 
the budget.
```

The CLI has two output modes:

**Structured mode** (default when stdout is not a TTY, or when `--format json`): Returns JSON that can be parsed by the MCP server or by the agent's tool processing. Minimal, machine-oriented.

**Human mode** (default when stdout IS a TTY): Returns formatted, colored output with context. Used when Willem runs commands directly.

**Agent mode** (when `--agent <agent_id>` is passed): Returns output specifically shaped for LLM consumption. This is the critical mode.

Agent mode output follows the prompt optimization principles:

```
PRINCIPLE: CLI agent-mode output is a DEMONSTRATION, not a 
constraint list.

BAD (constraint-style output — wastes k*):
  "Query returned 47 results. Entity :file/main-rs has attributes 
   :line-count (347), :contains-fn (:fn/handle-request), :modified 
   (2026-03-01), :hash (a7f3...), :size (12847), :language (:rust),
   :imports ([:tokio, :serde, :axum]), :test-coverage (0.73), ..."
  → The agent gets 47 entities × N attributes = hundreds of tokens 
    of flat data. Most of it is irrelevant to the current decision. 
    This is a context dump, not useful information.

GOOD (demonstration-style output — activates reasoning):
  "3 entities relevant to auth implementation:
   :file/main-rs — contains handle_request (TOCTOU risk, see INV-007)
   :config/auth  — JWT settings (uncertainty: 0.7, last observed 2h ago)
   :test/auth    — 3 failing assertions on token refresh
   
   ⚠ Guidance: INV-007 has high uncertainty. Consider re-observing 
     config/auth before proceeding.
   
   [Run `ddis query '[:find ...]'` for full details on any entity]"
  → The agent gets the minimum needed to decide its next action, 
    with a pointer to go deeper if needed. 80 tokens, not 800.
```

The agent-mode output structure:

```
CLI Agent-Mode Output Format:

1. HEADLINE (1 line): What the command found/did, in terms the 
   agent can immediately act on.

2. RELEVANT ENTITIES (3-7 items): The highest-relevance, 
   highest-significance results, at the pyramid level 
   appropriate for the current budget.

3. SIGNALS (0-3 items): Conflicts, uncertainty spikes, 
   staleness warnings — anything requiring attention.

4. GUIDANCE (1-3 items): Recommended next actions from the 
   guidance graph, ordered by relevance to the agent's 
   current intention.

5. POINTERS (1-3 items): Commands to run for deeper 
   information if needed.

Total target: 100-300 tokens per invocation.
```

This format maps directly to the dual-process architecture:
- Items 1-2 are **System 1 output** (ASSOCIATE + ASSEMBLE results, cheap, pattern-matchable)
- Item 3 is the **confusion channel** (signals that trigger re-retrieval)
- Item 4 is the **guidance comonad** (available actions at current state)
- Item 5 is the **depth escape hatch** (agent can go deeper if S2 needs more)

### Layer 2: MCP Server (Session-Aware Agent Interface)

Now, the question of whether MCP adds value over raw CLI. My analysis:

**What MCP provides that CLI does not:**

1. **Persistent session state.** An MCP server process maintains in-memory state across tool calls: the agent's frontier, the current k\*_eff estimate, the active intention, the subscription list. The CLI binary is stateless — it reads state from disk on every invocation, which means it can't track intra-conversation dynamics like k\* decay.

2. **Structured tool descriptions as ambient context.** MCP tool descriptions are injected into the agent's system-level context by the harness (Claude Code). They're essentially automatic Layer 0 — the agent sees the tool signatures without anyone manually editing CLAUDE.md.

3. **Type-safe input/output.** MCP defines schemas for tool inputs and outputs. The agent doesn't need to construct bash command strings (which can be malformed) — it fills in typed parameters.

4. **Proactive notifications.** An MCP server can push signals to the agent without being polled. When a conflict is detected or another agent's merge changes relevant state, the MCP server can inject a notification into the agent's next turn. The CLI can only respond when called.

**What CLI provides that MCP does not:**

1. **Zero infrastructure.** A Rust binary works. No running process, no protocol, no connection management.

2. **Human-accessible.** Willem can run `ddis status` from his terminal. He can't easily call MCP tools from a shell.

3. **Scriptable.** Bash scripts can compose CLI commands. MCP calls can't be easily piped/composed.

4. **Universal.** Any agentic harness that can call bash can use the CLI. MCP requires specific client support.

**My recommendation: Both, with the CLI as the foundation and a thin MCP wrapper.**

```
Architecture:

  ┌─────────────────────────────────┐
  │  Claude Code (Agent Harness)    │
  │                                 │
  │  ┌──────────┐  ┌────────────┐  │
  │  │ MCP      │  │ Bash Tool  │  │
  │  │ Client   │  │ Interface  │  │
  │  └────┬─────┘  └─────┬──────┘  │
  └───────┼──────────────┼─────────┘
          │              │
  ┌───────▼──────┐  ┌───▼────────────────┐
  │  ddis-mcp    │  │  ddis (CLI binary)  │
  │  (thin MCP   │──▶  (query engine,     │
  │   wrapper,   │  │   indexes, Datalog, │
  │   session    │  │   budget-aware      │
  │   state,     │  │   output)           │
  │   k* tracking│  └────────────────────┘
  │   notifs)    │
  └──────────────┘
```

The MCP server (`ddis-mcp`) is a thin process that:
- Calls the CLI binary for all datom operations
- Maintains session state (k\*_eff estimate, frontier, active intentions)
- Adjusts `--budget` passed to CLI based on k\*_eff tracking
- Provides MCP tool descriptions (ambient awareness)
- Queues notifications for delivery on next tool call

The CLI binary (`ddis`) is the core:
- All datom operations implemented here
- All query evaluation, indexing, CRDT logic
- Budget-aware output formatting
- Works standalone (for humans and scripts)
- Works as backend for MCP server

**The MCP server adds exactly one thing the CLI can't: session state that persists across tool calls within a conversation.** This is what enables k\* tracking, proactive notifications, and automatic budget adjustment.

But — and this is critical — the system works without MCP. An agent using only bash calls to `ddis` still gets budget-aware output, guidance, and the full protocol. The MCP server is an optimization, not a requirement. This preserves universality.

### Layer 3: Auto-Prompting Guidance

This is the **comonadic guidance structure** from our previous analysis, but now I need to formalize how it interacts with k\* decay.

The key insight from the prompt optimization document: **guidance IS the seed turn.** Study 8 showed that seed turns producing formal/principled reasoning improved subsequent quality with p=0.029. The guidance output from the datom system serves this exact function — it frames the agent's next action in terms of invariants, spec elements, and formal structure, naturally activating deep substrate.

```
INV-GUIDANCE-SEED-001: Guidance as Basin Activation
The guidance section of every CLI/MCP response MUST be phrased 
in spec-language (invariants, formal structure, domain terms), 
NOT in instruction-language (steps, checklists, procedures).

BAD guidance (instruction-language, activates surface substrate):
  "Step 1: Read the auth config file.
   Step 2: Check if JWT tokens are configured.
   Step 3: Run the test suite.
   Step 4: Fix any failures."

GOOD guidance (spec-language, activates deep substrate):
  "INV-AUTH-003 (token-refresh idempotency) has uncertainty 0.7.
   The :config/auth entity was last observed 2h ago — staleness 
   risk. The competing branches impl-auth-v1 and impl-auth-v2 
   both assert different token lifetimes; this conflict is 
   unresolved. What invariant should govern token lifetime?"

The good version:
  — Names invariants (activates formal reasoning)
  — Identifies uncertainty (focuses attention)
  — Poses a question (high DoF — exploration, not execution)
  — Does not prescribe steps (avoids mid-DoF saddle zone)
```

But guidance must also be k\*-aware. At turn 5, the agent can absorb rich guidance with context. At turn 45, guidance should be a single sentence:

```
GUIDANCE COMPRESSION BY ESTIMATED k*_eff:

k*_eff > 0.7 (early conversation, full budget):
  Full guidance: invariant context, uncertainty details, 
  competing branches, precedent references.
  Target: 100-200 tokens.

0.4 < k*_eff ≤ 0.7 (mid conversation, reduced budget):
  Compressed guidance: headline + single recommended action.
  Target: 30-60 tokens.

k*_eff ≤ 0.4 (late conversation, depleted budget):
  Minimal guidance: single imperative or question.
  Target: 10-20 tokens.
  Example: "Resolve INV-AUTH-003 uncertainty before proceeding."

k*_eff ≤ 0.2 (conversation should reset):
  Guidance: "Context budget critically low. Run `ddis harvest` 
  to generate carry-over, then start fresh conversation."
  Target: 15 tokens.
```

This is where the comonadic structure meets the prompt optimization framework's "budget constraints against context" rule. The `extend` operation from the guidance comonad is now parameterized by k\*_eff:

```
extend_guidance : k*_eff → (State → [Action]) → State → GuidanceTree

At high k*_eff: deep lookahead tree, rich context.
At low k*_eff: single recommended action, no tree.
```

### Layer 4: TUI (Human Live Feedback)

The TUI is the human's window into the system. It is NOT k\*-constrained (humans read from a terminal, not from a context window). Therefore the TUI can be as rich as useful.

The TUI is driven by subscriptions (SUBSCRIBE operation) and renders:
- Activity stream (real-time datom assertions across agents)
- Uncertainty heatmap (entities colored by uncertainty scalar)
- Branch status (active branches, comparisons, deliberations)
- Agent intentions (what each agent thinks it's doing)
- k\* gauges per agent (estimated remaining budget per conversation)
- Guidance topology (what actions are available, visualized as a graph)
- Fitness metrics (spec completeness, drift scores)

The TUI does not enter any agent's context window. It's a separate process reading from the datom store and the access log. Its output is purely for human consumption.

**One additional TUI function you implied but didn't name:** The TUI should support **human→agent signaling.** When Willem sees something on the TUI that needs attention (an agent drifting, a bad resolution, a missed conflict), he should be able to inject a signal from the TUI that enters the relevant agent's next tool response. This is the proactive direction mechanism you described.

```
TUI signal injection:
  Willem sees: Agent Alice is working on auth but INV-AUTH-003 
               was just invalidated by Bob's merge.
  Willem types: /signal alice "INV-AUTH-003 invalidated by Bob's 
               merge — re-observe before proceeding"
  Effect: Alice's next ddis command (via MCP) includes the 
          signal in its response. Alice's policy π sees the 
          signal and adjusts.
```

This is mediated by the MCP server, which queues the human signal for delivery.

---

## The Missing Piece: Trajectory Management via the Store

Here's the deep connection between the prompt optimization framework and the datom architecture that I don't think either was designed for but that falls out naturally.

The prompt optimization document identifies the core problem of long conversations:

> "By mid-conversation, [the model's own prior outputs] dominate the field more than the current prompt. This creates basin trapping: mediocre outputs self-reinforce, making prompt-level fixes insufficient. The intervention is trajectory management: seed turns, output curation, and strategic resets with curated carry-over."

The prescribed fix: harvest artifacts, reset conversation, seed new conversation with carry-over that's "basin-neutral" and "spec-flavored."

**The datom store IS this mechanism.** Here's why:

When an agent asserts datoms (discoveries, invariants, decisions, associations), those facts persist in the store independent of the conversation. When the conversation degrades (k\* depletion or basin trapping), the agent can:

1. `ddis harvest` — Extract the durable facts from the current conversation's assertions
2. Start a fresh Claude Code session
3. `ddis assemble --seed` — Generate a carry-over context from the store

The ASSEMBLE operation produces exactly what the trajectory-dynamics document prescribes:
- **Basin-neutral**: It's datoms, not conversation fragments
- **Spec-flavored**: Invariants and formal structure naturally activate deep substrate
- **Minimal**: Budget-constrained by the ASSEMBLE budget parameter
- **No conversation fragments**: Datoms don't carry "as we discussed" — they're facts

```
INV-TRAJECTORY-STORE-001: Store-Mediated Trajectory Management
The datom store MUST support a HARVEST operation that extracts 
durable facts from the current agent session and asserts them 
as datoms.

The datom store MUST support a SEED operation (via ASSEMBLE) 
that generates an optimal carry-over context for a fresh agent 
conversation, using the carry-over template format:

  1. Context (1-2 sentences from store summary)
  2. Invariants established (from :entity/type :invariant)
  3. Artifacts (current spec elements, code references)
  4. Open questions (from unresolved deliberations, high-uncertainty entities)
  5. Active guidance (from guidance graph, current state)

The SEED output MUST be formatted as a spec-first seed turn 
(Pattern #9 from prompt-patterns.md) to activate deep substrate 
in the receiving conversation.

Consequence: Agent conversation resets become cheap. The agent 
loses no durable knowledge (it's in the store). It loses only 
ephemeral reasoning (which was degrading anyway due to k* 
depletion). The fresh conversation starts with a formal seed 
that activates deep substrate.
```

This means the datom store solves the trajectory management problem **by construction**. Each conversation is a bounded trajectory (25-30 turns). The store is the persistent layer. Conversations start with store-assembled seeds and end with store-harvested assertions. The conversation is ephemeral; the knowledge is durable.

```
AGENT LIFECYCLE:

  1. Fresh conversation starts
  2. `ddis seed` generates carry-over context 
     (ASSEMBLE with --seed flag, spec-first format)
  3. Agent works for 20-30 turns, asserting discoveries 
     as datoms via `ddis transact`
  4. k*_eff drops below threshold (detected by MCP session 
     tracking or by agent observing degraded output)
  5. `ddis harvest` extracts durable facts from this session
  6. Conversation ends
  7. GOTO 1

  The store grows monotonically across conversations.
  Each conversation is a bounded trajectory.
  No conversation needs to be "the long one" — all 
  conversations are short, fresh, and seed-activated.
```

---

## The k\*-Aware CLI Command Taxonomy

With the interface layers defined, here's the concrete CLI design. Every command is classified by its k\* profile:

```
COMMAND TAXONOMY BY ATTENTION PROFILE:

CHEAP COMMANDS (≤50 tokens output, always safe):
  ddis status              — one-line state summary + top signal
  ddis guidance            — 1-3 recommended actions
  ddis frontier            — agent's current causal position
  ddis branch ls           — list active branches (names only)

MODERATE COMMANDS (50-300 tokens, safe early/mid conversation):
  ddis associate <cue>     — schema neighborhood (shape, not data)
  ddis query <expr>        — query results (budget-capped)
  ddis assemble            — assembled context (budget-capped)
  ddis diff <branch>       — branch delta summary
  ddis conflict ls         — active conflicts with severity
  ddis deliberation ls     — open deliberations with status

EXPENSIVE COMMANDS (300+ tokens, use sparingly, budget-gate):
  ddis assemble --full     — full context assembly (large)
  ddis query --unlimited   — uncapped query results
  ddis seed                — full carry-over seed (for new convo)
  ddis explain <entity>    — detailed entity with history

META COMMANDS (not for agent context window):
  ddis harvest             — extract durable facts (side effect)
  ddis transact            — assert datoms (side effect, minimal output)
  ddis merge               — CRDT merge (side effect)
  ddis subscribe           — register notification (side effect)
  ddis tui                 — launch TUI (separate process)
```

Every moderate and expensive command respects `--budget`:

```bash
# Early conversation (full budget available):
ddis associate "auth token refresh" --budget 300
# Returns: 15 entities, 40 attributes, 5 learned associations

# Late conversation (budget constrained):
ddis associate "auth token refresh" --budget 80
# Returns: 3 most significant entities, key attributes only

# The same query. Different output. Budget-aware compression.
```

---

## One Structural Question

Before I can finalize the interface design, I have a genuine uncertainty about the MCP layer.

**The k\* estimation problem.** The MCP server needs to estimate k\*_eff to adjust output budgets automatically. But k\*_eff is a function of:
- Conversation length (turns elapsed)
- Total tokens in context (system prompt + conversation history)
- Complexity of the current task (higher complexity → lower effective k\*)

The conversation length and approximate token count are knowable by the MCP server (it can count its own tool calls as a proxy for turns, and sum the sizes of its responses). But the total context window state — including the user's messages, system prompt, and other tool outputs — is NOT visible to the MCP server. It only sees its own tool calls.

**Two options for k\* estimation:**

**Option A — Turn-count heuristic.** The MCP server estimates k\*_eff as a decreasing function of the number of tool calls it has served in this session:

```
k*_eff = k*_base × decay(tool_calls_served)

where decay is exponential: decay(n) = e^{-αn}
and α is calibrated empirically (default: α = 0.03, 
giving k*_eff ≈ 0.5 at 23 tool calls)
```

Simple, doesn't require any information from the agent. But potentially inaccurate — a conversation with many short turns decays faster than one with few long turns.

**Option B — Agent-reported budget.** The agent includes its estimated remaining budget in each tool call:

```bash
ddis query '[:find ?e ...]' --context-tokens 45000 --context-limit 200000
```

The CLI/MCP uses the ratio (tokens_used / limit) to estimate k\*_eff. More accurate, but requires the agent to know and report its context state. Claude Code doesn't natively expose this, so it would need to be estimated by the agent itself.

**My recommendation: Option A with Option B as override.** The turn-count heuristic works for the common case. If the agent or human wants finer control, `--budget` overrides the automatic estimate. The MCP server's heuristic is a reasonable default, not a precise measurement — which aligns with the prompt optimization document's observation that "k\* cannot be determined in advance."

What's your preference on the k\* estimation approach? And does the five-layer interface architecture (Ambient/CLI/MCP/Guidance/TUI) match your mental model, or do you see a layer I'm missing or one that should be collapsed?

---
