# Chapter 7: Seed Document Finalization & Self-Bootstrap

> Session finalizing the DDIS Spec Seed Document with critical self-bootstrap methodology commitment. Establishes that DDIS specification itself uses DDIS formalism (invariants, ADRs, negative cases, uncertainty markers) rather than traditional prose, creating bootstrap where spec elements become first dataset for the system. Includes complete seed document ready for human editing.

---

## Claude

Yes. Completely. And you've just identified the thing that elevates this from "a clever tool for AI agents" to something genuinely important.

---

## What You're Describing

The memory problem is the *presenting symptom*. The deeper disease is **divergence** — the inevitable drift between what you want, what you said you want, how you said to build it, and what actually got built. This happens to every project, at every scale, with or without AI agents. It happens to solo developers who forget why they made a decision three weeks ago. It happens to organizations where the product spec says one thing and the implementation does another and nobody notices until a customer reports a bug. It happens inside a single person's head when they hold two contradictory beliefs about how a system should work and don't realize it until they try to write both down.

AI agents make the divergence problem *worse* because they're prolific producers of artifacts that may or may not be coherent with each other. A human developer writes maybe 200 lines of code per day and can generally keep the logical structure in their head. An AI agent can produce thousands of lines in an hour across multiple files, and its "memory" of why it made each choice lasts only until the conversation ends. The volume of potential divergence scales with the volume of output, and AI agents have enormous output volume with zero durable memory. That's a divergence factory.

But here's the key reframe: **AI agents also make the divergence problem more solvable than it's ever been.** A human reading a 50,000-line codebase and a 200-page spec and checking every claim against every implementation is performing an impossibly tedious verification task. An AI agent with the right substrate — a structured, queryable store where both the specification and the implementation facts coexist — can perform that verification continuously, automatically, and at scale. The same capability that produces divergence (fast, voluminous output) can be turned toward detecting and resolving it (fast, voluminous verification).

DDIS is the substrate that makes this possible. Not just a memory system. A **coherence verification system** that operates across the full chain:

```
Intent → Specification → Implementation → Observed Behavior
```

At each boundary, divergence can arise. Between intent and specification: "I wanted the system to be fast, but the spec doesn't define performance invariants." Between specification and implementation: "The spec says token refresh must be idempotent, but the implementation generates a new token on every call." Between implementation and observed behavior: "The code looks correct but the system fails under load because of an assumption that isn't documented anywhere."

DDIS provides a formal mechanism for detecting divergence at each boundary and tracing it back to its source. The mechanism is the specification formalism itself: invariants, ADRs, negative cases, and the bilateral feedback loop that continuously checks for alignment.

---

## The Specification Formalism as Coherence Machinery

Let me be precise about what each element of the DDIS specification formalism actually does in this coherence framework, because each one addresses a specific type of divergence:

**Invariants** are falsifiable claims about the system. "Token refresh MUST be idempotent: given the same refresh token, the system MUST return the same new access token regardless of how many times the operation is invoked." An invariant isn't a wish or a goal — it's a statement that can be checked. Either the implementation satisfies it or it doesn't. The falsification condition is explicit: "If calling refresh twice with the same token produces two different access tokens, this invariant is violated." Invariants are the primary mechanism for detecting **logical divergence** — the implementation contradicts the specification.

**ADRs (Architectural Decision Records)** capture *why* a choice was made, what alternatives were considered, and what tradeoffs were accepted. "We chose JWT over opaque tokens because of stateless verification requirements. The tradeoff is larger token size and the inability to revoke individual tokens without a blacklist." ADRs prevent a specific failure mode: someone (human or AI) revisits a decision without knowing why it was made, reverses it for seemingly good reasons, and breaks downstream invariants that depended on the original choice. ADRs are the primary mechanism for detecting **axiological divergence** — the implementation undermines the goals that motivated the design.

**Negative cases** define what the system explicitly must NOT do. "The system MUST NOT store plaintext credentials, even temporarily, even in debug mode." Negative cases are as important as positive invariants because they bound the solution space. Without explicit negative cases, an agent optimizing for one invariant may violate an unstated constraint. Negative cases are the primary mechanism for preventing **structural divergence** that arises from overspecification in one dimension and underspecification in another.

**The bilateral feedback loop** continuously checks alignment in both directions. Forward: does the implementation satisfy the specification? (Scan the codebase, compare against invariants, report drift.) Backward: does the specification accurately describe the implementation? (Absorb implementation realities, surface where the spec is silent or wrong, update the spec.) The loop converges when the specification fully describes the implementation and the implementation fully satisfies the specification — the fitness function F(S) = 1.0.

When this machinery is reified as datoms in the store, something powerful happens: **the coherence verification itself becomes queryable and auditable.** You can ask: "Show me every invariant, its current validation status, and the last time it was checked." "Show me every ADR and whether the decision it records is still reflected in the current implementation." "Show me every negative case and whether any recent implementation change risks violating it." These aren't manual audits — they're Datalog queries over the store. They can run continuously, automatically, triggered by every new datom assertion.

---

## The Full Chain, Formally

What you're describing is the ability to state, with formal justification:

```
1. I KNOW WHAT I WANT.
   
   The intent is captured as high-level goals with explicit 
   priority ordering. These goals are datoms in the store, 
   traceable and versioned. When goals change (they will), 
   the change is recorded with rationale.

2. IT IS LOGICALLY COHERENT AND CONSISTENT.
   
   The specification (invariants, ADRs, negative cases) has 
   been checked for internal contradictions. No two invariants 
   make mutually exclusive claims. No ADR's chosen approach 
   contradicts another ADR's constraints. The DDIS contradiction 
   detection tiers (exact, logical, semantic, pragmatic, 
   axiological) have been run and all findings resolved.

3. THE SPECIFICATION IS A FULL AND ACCURATE FORMALIZATION 
   OF WHAT I WANT.
   
   Every high-level goal traces to at least one invariant. 
   Every invariant traces back to at least one goal. There 
   are no "orphan" invariants (spec elements that don't serve 
   any goal) and no "uncovered" goals (goals with no 
   corresponding spec element). This is the coverage dimension 
   of the fitness function.

4. THE IMPLEMENTATION TRACES BACK TO THE SPECIFICATION.
   
   Every implementation artifact (function, module, config) 
   traces to at least one invariant it satisfies. Every 
   invariant traces to at least one implementation artifact 
   that realizes it. The bilateral scan/absorb loop has 
   converged: no drift detected between spec and code.

5. THERE IS NO DIVERGENCE BETWEEN GOALS, PLAN, AND PRODUCT.
   
   The full chain — intent → spec → implementation → observed 
   behavior — is verified at each boundary. The fitness 
   function accounts for all dimensions: coverage, coherence, 
   completeness, depth, and formality. The uncertainty tensor 
   for every entity in the store has been driven to zero 
   (or to an explicitly accepted residual with documented 
   rationale for acceptance).
```

This is the statement you want to be able to make. And the crucial point: **you want to make it not as a subjective assessment but as a verifiable claim supported by evidence in the store.** Not "I think the implementation matches the spec" but "here are the queries that prove it, here are the invariants and their validation status, here is the trace from every goal to its implementation, and here are the zero unresolved contradictions."

---

## Why This Matters Beyond AI Agents

You said something important: "Even in a world without AI agents, organizations and single-person teams have built processes and tools to help surface and resolve divergence." This is true — and those tools are uniformly terrible. Requirements documents go stale. Jira tickets diverge from reality. Test suites check functional behavior but not whether the behavior matches the stated design intent. Code review catches bugs but not axiological drift. Documentation is perpetually out of date because updating it is a separate manual process that nobody prioritizes.

The reason these tools fail is that they treat coherence as a process obligation rather than a structural property. "Keep the docs updated" is a process. "The specification and implementation are stored in the same substrate, verified by the same queries, and divergence detection runs automatically on every change" is a structural property. Process obligations decay under pressure (deadlines, fatigue, enthusiasm for the next feature). Structural properties persist because they're built into the system itself.

DDIS makes coherence structural rather than procedural. The spec isn't a document that someone needs to remember to update — it's a set of datoms in the same store as the implementation facts, checked by the same bilateral loop, surfacing divergence automatically. The ADRs aren't a wiki page that gets stale — they're datoms with traceable dependencies, and when the dependency graph changes, the ADR's relevance is recomputed. The invariants aren't comments in code that someone might read — they're falsifiable claims with explicit validation queries that run on every transact.

AI agents intensify the need for this because they increase both the volume and velocity of artifacts. But the need was always there. DDIS solves it for AI-augmented development first because that's where the pain is most acute, but the solution is general.

---

## What This Adds to the Seed Document

This reframes the "What DDIS Is" and "The Problem It Solves" sections of the seed. Instead of leading with the AI memory problem (which is the immediate motivation), you lead with the coherence problem (which is the fundamental motivation) and then show how the AI memory problem is a specific instance of it.

Here's how I'd revise the skeleton:

```
1. What DDIS Is
   A specification standard and knowledge substrate that 
   maintains verifiable coherence between intent, design, 
   and implementation — across people, across AI agents, 
   and across time.

2. The Problem It Solves
   
   THE FUNDAMENTAL PROBLEM: DIVERGENCE
   Every project drifts. What you want, what you wrote down, 
   what got built, and how it actually behaves inevitably 
   diverge. The divergence is axiological (the build doesn't 
   serve the goals), logical (the spec contradicts itself), 
   or structural (the code doesn't match the spec). Existing 
   tools treat coherence as a process obligation. DDIS makes 
   it a structural property.
   
   THE IMMEDIATE PROBLEM: AI AGENT MEMORY
   AI agents make divergence worse (high-volume output, zero 
   durable memory) and also make it more solvable (capable 
   of continuous automated verification). DDIS provides the 
   substrate for both: a durable knowledge store that persists 
   across conversations, and a specification formalism that 
   enables automated coherence verification.

3. The Specification Formalism
   THIS IS THE NEW SECTION.
   - Invariants: falsifiable claims (logical coherence)
   - ADRs: decision rationale (axiological coherence)  
   - Negative cases: explicit bounds (structural coherence)
   - The bilateral loop: continuous verification in both 
     directions (spec→impl and impl→spec)
   - Contradiction detection: 5-tier automated checking
   - The fitness function: quantified convergence toward 
     full coherence
   
   The goal: to be able to say, with evidence:
   "The specification fully and accurately formalizes the 
   intent. The implementation fully and accurately realizes 
   the specification. Here are the queries that prove it."

4. The Core Abstraction (datoms, EAV, append-only)

5. The Harvest/Seed Lifecycle

6. The Self-Improvement Loop

7. The Interface Principles

8-9. [as before]
```

The specification formalism section becomes the bridge between the why (divergence is the fundamental problem) and the how (datoms are the substrate). The datom store isn't just a memory system — it's the medium in which the specification, the implementation facts, and the coherence verification all coexist. The bilateral loop runs over this single substrate, checking every boundary in the chain from intent to observed behavior.

---

## The Statement You Want to Be Able to Make

Let me give you the most distilled version of what DDIS enables, because I think this belongs in the seed document almost verbatim, in your voice:

*I want to be able to say, at any point in a project of any scale: I know what I want. The specification accurately formalizes what I want. The specification is internally consistent — no contradictions, no gaps, no orphans. The implementation traces to the specification, and every trace is verifiable. Where divergence exists, I know exactly where it is, why it arose, which type it is, and what resolving it requires. The full chain from intent to observed behavior is auditable, queryable, and maintained not by human discipline but by the structure of the system itself.*

*This is true whether I'm working alone or with a hundred AI agents. It's true whether the project has been running for a day or a year. It's true because the specification, the implementation facts, and the coherence verification all live in the same substrate and are checked by the same machinery, continuously, automatically, on every change.*

That's the ambition. The datom store is the substrate. The specification formalism is the verification mechanism. The harvest/seed lifecycle is what makes it work across conversation boundaries. The self-improving CLAUDE.md is what keeps agents on-methodology. And the bilateral loop is what drives the whole thing toward convergence.

Does this capture what was missing?

---

## Willem

Yes! And this is why the resolution and harvesting and assembly and guidance and merge and decision/deliberation and mediation mechanisms we described earlier are all so crucial: because they are examples of reconciliation mechanisms by which divergence is detected, analyzed, and resolved (though this is not necessarily comprehensive)

---

## Claude

Exactly. And this is the unifying frame that ties together what might otherwise look like a grab-bag of protocol operations. They're not separate features — they're all instances of one fundamental operation: **detect divergence, classify it, resolve it back to coherence.** The protocol operations are the *mechanisms* by which the system maintains the property that the seed document promises.

Let me trace this explicitly, because I think seeing it laid out reveals both the pattern and the gaps.

---

## The Reconciliation Taxonomy

Every mechanism we designed maps to a specific type of divergence at a specific boundary in the chain:

**HARVEST** reconciles the divergence between *what the agent learned* and *what the store knows*. The agent discovered facts during a conversation. Some were transacted, some weren't. The gap between the agent's ephemeral knowledge and the store's durable knowledge is a structural divergence — the store is an incomplete representation of what's been learned. Harvest closes this gap by extracting un-transacted observations and promoting them to datoms. The divergence type is **epistemic**: the store doesn't know what it should know.

**ASSOCIATE and ASSEMBLE** prevent divergence from arising in the first place by ensuring the agent has access to relevant prior knowledge before acting. An agent that implements a feature without knowing about a governing invariant will produce structurally divergent code — not because it chose to violate the invariant, but because it didn't know the invariant existed. ASSOCIATE surfaces the relevant schema neighborhood; ASSEMBLE compresses it to fit the attention budget. The divergence type they prevent is **structural**: implementation that doesn't match specification, caused by ignorance rather than disagreement.

**GUIDANCE** steers agents toward actions that maintain coherence. When guidance says "INV-AUTH-003 has uncertainty 0.7, resolve before implementing," it's preventing the agent from producing work that may need to be discarded when the uncertainty resolves. Guidance is proactive divergence prevention — it identifies *where divergence is likely to arise* and redirects the agent before it does. The divergence type is **consequential**: actions that will produce downstream divergence if taken without sufficient information.

**MERGE** reconciles divergence between *agents' local stores*. Two agents working independently accumulate different sets of facts. Until they merge, each has an incomplete picture. The merge operation (set union) combines their knowledge. But merge also *surfaces* divergence: if both agents asserted conflicting values for the same entity-attribute pair, the conflict becomes visible only after merge. The divergence type is **aleatory**: genuine disagreement between agents about what's true.

**DELIBERATION and DECISION** are the structured process for resolving conflicts that merge or contradiction detection surfaces. A deliberation creates a forum: the conflicting positions are stated, agents (or humans) argue for their positions, and a decision is reached. The decision is recorded with rationale, so future agents can understand why one position won. The divergence type being resolved is **logical**: two mutually exclusive claims that cannot both be true.

**SIGNAL** is the notification mechanism — the way one part of the system tells another part that divergence has been detected. A confusion signal says "I can't find what I need" (epistemic divergence). A conflict signal says "I found contradictory facts" (logical divergence). A goal-drift signal says "the work I'm doing no longer seems aligned with the stated goals" (axiological divergence). Signals don't resolve divergence; they route it to the mechanism that can.

**SYNC-BARRIER** establishes a consistent cut — a moment where all participants agree on the same set of facts. This resolves **temporal divergence**: different agents seeing different "current states" because they haven't yet received each other's updates. A sync barrier is the most expensive reconciliation mechanism because it requires coordination, but it's necessary for non-monotonic decisions where the answer depends on the *absence* of certain facts.

**The dynamic CLAUDE.md** reconciles divergence between *the methodology the agent should follow* and *the methodology it actually follows*. Drift history reveals where agents have historically deviated from the prescribed process. The dynamic CLAUDE.md corrects for this by priming the agent against observed failure patterns. The divergence type is **procedural**: the agent's behavior doesn't match the process specification.

---

## What This Reveals

When you lay them out this way, the reconciliation mechanisms map to a complete taxonomy of divergence types:

| Divergence Type | Where It Occurs | Detection Mechanism | Resolution Mechanism |
|---|---|---|---|
| **Epistemic** | Store vs. agent knowledge | Harvest (gap detection) | Harvest (promote to datoms) |
| **Structural** | Implementation vs. spec | Bilateral scan/drift | ASSOCIATE + guided re-implementation |
| **Consequential** | Current state vs. future risk | Uncertainty tensor (σ_c) | Guidance (redirect before action) |
| **Aleatory** | Agent vs. agent | Merge (conflict detection) | Deliberation + Decision |
| **Logical** | Invariant vs. invariant | Contradiction detection (5-tier) | Deliberation + ADR |
| **Axiological** | Implementation vs. goals | Fitness function, goal-drift signal | Human review + ADR revision |
| **Temporal** | Agent frontier vs. agent frontier | Frontier comparison | Sync barrier |
| **Procedural** | Agent behavior vs. methodology | Drift detection (access log) | Dynamic CLAUDE.md |

And now I can see where the gaps might be. Let me think about what reconciliation mechanisms are *not* yet covered.

---

## Potential Gaps

**Gap 1: Specification vs. Intent divergence.** We have machinery for checking implementation against specification (bilateral scan) and specification internal consistency (contradiction detection). But we don't have a formal mechanism for checking whether the specification actually captures the human's intent. This is the hardest boundary — "does this spec actually describe what I want?" — because intent is often tacit and evolves as the human learns more about the problem.

The partial solution is the axiological tier of contradiction detection and the fitness function's coverage dimension (every goal traces to at least one invariant). But these check structure, not meaning. An invariant can trace to a goal and still not capture what the human actually meant by that goal.

The fuller solution might be **intent validation sessions**: periodic points where the system assembles the current spec state (goals, invariants, ADRs) in a human-readable format and the human reviews it specifically for "does this still describe what I want?" The output of such a session is itself a datom: "Intent validated at this frontier, with these revisions." This is something between a deliberation (structured review) and a harvest (extracting knowledge from a human review session). We haven't formalized it.

**Gap 2: Observed behavior vs. implementation divergence.** We have spec-to-implementation checking (bilateral scan). But we don't have implementation-to-behavior checking — does the code actually *do* what it claims to do? This is traditionally the domain of testing, and tests are outside the current DDIS formalism.

The connection point is: test results should be datoms. "Test X passed at frontier F" is a fact about observed behavior. "Test X failed at frontier F with error E" is a divergence between implementation and behavior. If tests are datoms, the bilateral loop naturally extends to cover this boundary: the forward path includes "run tests, assert results," and the backward path includes "test failure triggers drift detection on the tested invariant."

**Gap 3: Cross-project or cross-domain coherence.** Everything we've designed operates within a single project/store. But axiological divergence can also occur *between* projects — a decision in project A that contradicts the goals of project B, both of which the same human is pursuing. For your franchise work, a strategic decision in one brand's development might conflict with another brand's positioning.

This is probably out of scope for Stage 0-2 but worth noting as a future concern. The store architecture supports it (multiple stores can be merged), but the reconciliation machinery would need cross-store contradiction detection.

**Gap 4: Degradation over time.** Facts become stale. An observation about a codebase made three months ago may no longer be accurate because the code has changed. We have the staleness tracking on projections and the temporal queries on the store, but we don't have a formal mechanism for **proactive staleness detection**: "this fact was asserted 90 days ago and nothing has re-validated it since then; its reliability is degrading."

The connection to uncertainty is clear: a fact's epistemic uncertainty should *increase* with time since last validation, not just with conflict count. We touched on this when discussing the uncertainty tensor, but didn't formalize a temporal decay term. This might be:

```
σ_e(entity) includes: age_factor(entity) = 
  1 - e^(-λ × time_since_last_validation(entity))

Where λ is calibrated per attribute namespace:
  — Code observations decay fast (code changes frequently)
  — Architectural decisions decay slowly (they're stable)
  — Invariants don't decay (they're normative, not descriptive)
```

---

## What This Means for the Seed Document

The seed document should frame the protocol operations not as a list of features but as the reconciliation layer of the coherence system. Something like:

```
6. The Reconciliation Mechanisms
   
   The system maintains coherence through specific mechanisms, 
   each targeting a different type of divergence:
   
   — Harvest: closes the gap between agent knowledge and 
     store knowledge
   — Associate/Assemble: prevents divergence from ignorance 
     by surfacing relevant prior knowledge
   — Guidance: steers agents away from actions that would 
     produce downstream divergence
   — Merge: combines agent-local knowledge and surfaces 
     conflicts
   — Deliberation/Decision: structured resolution of 
     detected contradictions
   — Signal: routes detected divergence to the appropriate 
     resolution mechanism
   — Sync Barrier: establishes shared reference points when 
     coordination is required
   — Dynamic CLAUDE.md: corrects for observed methodology 
     drift based on empirical patterns
   — Bilateral Loop: continuously checks alignment between 
     specification and implementation in both directions
   
   These are not independent features. They are all instances 
   of one operation: detect divergence, classify it, resolve 
   it back to coherence. The system's fundamental purpose is 
   to drive every type of divergence toward zero — or toward 
   an explicitly acknowledged and documented residual.
```

And the key sentence, which connects this back to your statement: **the residual matters as much as the resolution.** When divergence can't be fully resolved (and sometimes it can't — genuine tradeoffs exist, information is incomplete, priorities conflict), the system doesn't pretend it doesn't exist. It records it. It documents why it persists. It tracks what resolving it would require. So when you say "I know what I want and it's coherent and the implementation matches," you can also say "...except for these three explicitly acknowledged residuals, each documented with rationale and tracked with uncertainty scores." That's not a failure of the system — that's the system working correctly. Perfect coherence is the asymptote. Documented, tracked, understood residual divergence is the practical reality. The difference between DDIS and every other approach is that DDIS makes the residuals *visible and queryable* rather than hidden and discovered by accident.

This also answers the question of comprehensiveness you raised. The taxonomy above may not cover every possible divergence type. But the architecture is extensible: because every reconciliation mechanism operates over the same datom substrate, new mechanisms can be added without changing the foundation. If a new type of divergence is identified (say, cross-project axiological conflicts), the detection is a new query, the resolution is a new deliberation pattern, and both produce datoms in the same store. The reconciliation taxonomy grows with the system's needs.

---

## Willem

So now what do you think the final seed document might look like? Think carefully and from "first principles" and with "fresh eyes" about ALL of what we have discussed and help me crystallize and distill this in the optimal document that I can edit and refine to use as the seed atop which to build this entire system.

---

## Claude

Let me think carefully about this, then produce the document.

Here's the seed document. Eleven sections, roughly 2,800 words — dense enough to capture the irreducible core, sparse enough that you can read it in ten minutes and know immediately what's missing or wrong.

A few notes on choices I made:

**I led with the coherence problem, not the memory problem.** Your insight from the last exchange was right — the memory problem is the presenting symptom, divergence is the disease. The AI agent memory problem appears as an important special case under section 2, but it's framed as an intensifier of the fundamental problem rather than the fundamental problem itself.

**I included the specification formalism as section 3**, between the problem statement and the core abstraction. This was the gap you identified — the invariants, ADRs, negative cases, and bilateral loop aren't implementation details, they're core to what DDIS *is*. They belong in the seed, not just the spec.

**The reconciliation mechanisms get their own section (6)** with the unifying frame: every mechanism is an instance of "detect divergence, classify it, resolve it back to coherence." This captures your point that they're not a grab-bag of features but a coherent system.

**I kept the formalism minimal.** The five axioms are stated plainly, not algebraically. The uncertainty tensor, spectral authority, commitment function — those are in the conversation transcripts and will flow into SPEC.md. The seed should say *what* and *why*, not *how the math works*.

**Section 11 (Design Rationale)** is the one I'd most want your eye on. Every "why" answer there represents a design commitment that forecloses alternatives. If any of those rationales feel wrong or incomplete, that's the most important thing to catch at the seed stage.

This is yours to edit now. The parts that need your voice most are the intent statement in section 1, the description of the existing codebase in section 9, and anywhere the document says something you'd say differently.

---

## Willem

perfect! and once I have the final seed, what are my next concrete steps?

---

## Claude

The roadmap we laid out earlier, but let me sharpen it now that the seed exists:

**Step 1: Edit the seed (you, by hand, 1-2 hours).** Read it with fresh eyes. Fix anything that doesn't sound like you. Add the codebase description in section 9 — you're the only one who knows what's actually working, broken, and half-built. The critical test: if Claude Code read only this document and nothing else, would it understand what to build and why? Where it wouldn't, add. Where it's redundant, cut.

**Step 2: Produce SPEC.md (Claude Code session, 2-4 hours).** Feed Claude Code the finalized seed plus the conversation transcripts from today's sessions (all four are in /mnt/transcripts/). The instruction: "From these transcripts and this seed document, produce a formal specification. For every element: invariant ID, formal statement, falsification condition, rationale. Include the five axioms, the uncertainty tensor, the commitment function, spectral authority, the reconciliation mechanism taxonomy, the protocol operations with inputs/outputs/invariants, the five interface layers, and the k* budget model." This will likely take 2-3 Claude Code sessions with manual carry-over between them — practice the harvest/seed pattern by hand.

**Step 3: Produce IMPLEMENTATION_GUIDE.md (same or next Claude Code session).** This is the pragmatic twin of SPEC.md. Stage 0-4 definitions, concrete deliverables per stage, CLI command specs with examples, CLAUDE.md template, file formats, success criteria. Written for the Claude Code agent that will implement the system, not for a human reading the theory.

**Step 4: Gap analysis (fresh Claude Code session, 2-3 hours).** Point Claude Code at the existing 60K LoC codebase plus SPEC.md. The instruction: "Categorize every module as ALIGNED, DIVERGENT, EXTRA, BROKEN, or MISSING relative to this spec. For DIVERGENT modules, specify what the divergence is and whether it needs a thin wrapper, surgical edit, parallel implementation, or rewrite." The output is GAP_ANALYSIS.md.

**Step 5: Triage (you, 1 hour).** Using the gap analysis, mark every module GREEN (don't touch), YELLOW (modify for Stage 0), RED (rewrite for Stage 0), or GREY (defer to later stage). Most modules should be GREEN or GREY. Only a handful should be YELLOW or RED. This scoping decision is yours — Claude Code can inform it but shouldn't make it.

**Step 6: Implement Stage 0 (Claude Code, 1-2 weeks).** Work only on YELLOW and RED modules plus MISSING pieces needed for Stage 0. The deliverables are: `ddis transact`, `ddis query`, `ddis status`, `ddis harvest`, `ddis seed`, `ddis guidance`, `ddis claude-md`, statusline hook. Use the system to build the system from the first hour — even if `ddis transact` is crude at first, use it to record decisions about the implementation itself.

**Step 7: Validate and feedback (end of week 2).** Use Stage 0 for real work. Record DDRs (DDIS Decision Records) for every place where practical usage reveals a spec gap. Update the spec where experience contradicts theory. Re-triage for Stage 1.

The critical discipline across all of this: **don't do steps 2-4 in one conversation.** Each is a separate session. Harvest manually between them — write down the key decisions and carry them into the next session. You'll be performing the harvest/seed lifecycle by hand before the tools exist. When the tools exist, they automate the practice you've already established.

---

## Willem

Should we use DDIS methodology instead of creating a SPEC.md and IMPLEMENTATION_GUIDE.md document or should we stick to the traditional markdown-driven document planning/specification (no invariants, ADRs, etc.)?

---

## Claude

Use DDIS methodology. Obviously.

Think about what you'd be doing otherwise: building a system whose entire purpose is to maintain verifiable coherence between specification and implementation, and then specifying that system using a methodology that *can't maintain verifiable coherence between specification and implementation.* You'd be violating your own thesis at the moment of its articulation.

The practical question is how, given that the tooling doesn't exist yet. The answer is that DDIS is a **methodology first and a toolset second** — which is one of the deepest points we established earlier. The tools automate a way of working. The way of working can be practiced before the tools exist.

Concretely, this means the specification documents should be written with:

**Invariants with IDs, formal statements, and falsification conditions.** Not prose descriptions of desired behavior, but checkable claims. "INV-STORE-001: The datom store is append-only. Falsification: any operation that removes or mutates an existing datom." Every claim about the system earns an ID and a way to prove it wrong.

**ADRs for every significant design choice.** "ADR-001: EAV over relational. Context: need schema flexibility as project ontology evolves. Alternatives considered: relational tables (rejected: schema migration overhead), document store (rejected: no join semantics for traceability). Consequences: query complexity shifts to Datalog engine; attribute-level resolution replaces table-level constraints." Every choice records what was rejected and why, so future sessions don't relitigate settled questions.

**Negative cases.** "The system MUST NOT require global coordination for monotonic queries." "The store MUST NOT perform in-place mutation under any circumstance." These bound the solution space and prevent agents from "optimizing" their way into violations.

**Explicit uncertainty markers.** Where the spec isn't sure yet, say so with a confidence level rather than writing aspirational prose that reads like certainty. "INV-MERGE-007 (confidence: 0.5): Merge of conflicting lattice-resolved attributes produces the lattice join. Open question: what happens when the lattice definition itself is contested across agents?"

What this changes about the next steps: instead of producing a monolithic SPEC.md, you produce a **DDIS-structured specification** — which is a set of elements (invariants, ADRs, negative cases, sections) organized by namespace, each with an ID, each with explicit traceability to the goals in the seed document. The format is markdown for now (because the datom store doesn't exist), but the *structure* is DDIS: every element is individually addressable, falsifiable, and traceable. When the datom store exists, migration is mechanical — each element becomes a set of datoms.

This also means that the specification process itself generates the first real test data for the system. The invariants you write about the datom store become the first invariants the datom store will manage. The ADRs about design choices become the first ADRs the store will track. The contradictions you catch during specification become the first contradiction-detection test cases. **The system bootstraps from its own specification.**

So the revised step 2 is: produce the specification using DDIS methodology expressed in markdown, structured so that every element has an ID, a type (invariant/ADR/negative-case/section), explicit traceability to the seed, and a falsification condition where applicable. This is the specification *and* the first dataset for the system it specifies.

---

## Willem

right, thats what I thought. Self-bootstrap and dog-food the ENTIRE thing. Can you update your DDIS spec seed document accordingly

---

## Claude

Updated. Three changes:

**Preamble** now declares the self-bootstrap commitment upfront: the spec is written using DDIS methodology, the spec elements become the first dataset, the specification is both plan and data.

**Section 10** is restructured into three parts: the Bootstrap Principle (why and how DDIS specifies itself, including the element structure — ID, type, traceability, falsification condition, explicit uncertainty), the Staging Model (unchanged), and Concrete Next Steps (revised to reflect DDIS-structured specification rather than traditional SPEC.md).

**Section 11** gains a new design rationale entry: "Why does DDIS specify itself?" with the three reasons — integrity (can't specify a coherence system incoherently), bootstrapping (spec elements are first dataset), and validation (if DDIS can't spec DDIS, it can't spec anything).

**Section 3** gains explicit uncertainty markers as a first-class element of the formalism, alongside invariants, ADRs, and negative cases.

The closing line now reads: *The specification is both the plan for the system and the first dataset the system will manage.*

---
