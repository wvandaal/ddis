# The Trilateral Coherence Model

> **Date**: 2026-03-03
> **Scope**: Formal algebraic treatment of the three-state coherence architecture
> **Purpose**: Formalize the process by which work moves between Intent, Specification,
> and Implementation — with bilateral morphisms, a universal convergence cycle, and
> a Lyapunov stability argument for why DDIS drives toward coherence.
>
> **Traces to**: SEED.md §1–§6, spec/10-bilateral.md, spec/05-harvest.md,
> AGENTIC_SYSTEMS_FORMAL_ANALYSIS.md, HARVEST_SEED.md §5–§9
>
> **Reading prerequisite**: SEED.md (the foundational document), spec/10-bilateral.md
> (bilateral adjunction), spec/05-harvest.md (harvest pipeline). This document extends
> and unifies those formalisms.

---

## Table of Contents

1. [Motivation: The Triangle of Coherence](#1-motivation)
2. [Ground Truths: Three Primary Representations](#2-ground-truths)
3. [The Category of Coherence States](#3-category-of-coherence-states)
4. [Bilateral Functors: The Six Morphisms](#4-bilateral-functors)
5. [Adjunctions: Three Convergence Pairs](#5-adjunctions)
6. [The Universal Convergence Cycle](#6-universal-convergence-cycle)
7. [Lyapunov Stability: The Energy Argument](#7-lyapunov-stability)
8. [The Mediation Theorem: Why Specification is Central](#8-mediation-theorem)
9. [Divergence Dynamics: Work as Energy Storage](#9-divergence-dynamics)
10. [Scaling Properties: The Triangle at Scale](#10-scaling-properties)
11. [Grounding in Existing Design](#11-grounding)
12. [Open Questions and Uncertainties](#12-open-questions)
13. [Implications for Implementation](#13-implications)
14. [Topology-Mediated Conflict Resolution](#14-topology)
15. [The Signal-to-Noise Problem in Intent](#15-signal-noise)
16. [Structural Inevitability: Why the System "Can't Help But" Converge](#16-inevitability)
17. [The Unification: Three Models, One Substrate](#17-unification)
18. [Critical Assessment: Strengths, Weaknesses, and the Path to Adoption](#18-critical-assessment)
19. [The Radical Addition: Unified Store, Three LIVE Views](#19-live-views)
20. [Practical Implementation](PRACTICAL_IMPLEMENTATION.md) *(separate document)*

---

## 1. Motivation: The Triangle of Coherence

<a name="1-motivation"></a>

SEED.md §2 identifies four divergence boundaries forming a chain:

```
Intent → Specification → Implementation → Observed Behavior
```

This chain is linear: each boundary separates adjacent states. But the actual dynamics
are not linear — they form a **triangle**. An agent can (and routinely does) jump directly
from intent to implementation, bypassing specification. A test failure feeds directly back
to intent, bypassing the spec. The existing DDIS design (spec/10-bilateral.md) formalizes
the Specification ↔ Implementation boundary as an adjunction but leaves the other two
boundaries less structured.

This document formalizes all three boundaries simultaneously.

### The Three States

| State | Symbol | Ground Truth | Algebraic Structure |
|-------|--------|-------------|---------------------|
| **Intent/Ideation** | **I** | JSONL session logs, conversations, mental models | Free monoid E* with prefix order |
| **Specification** | **S** | DDIS spec elements (INV, ADR, NEG, UNC with IDs) | Labeled directed graph with coherence constraints |
| **Implementation** | **P** | Code files, tests, build artifacts | Abstract syntax forest + test oracle |

Each state has a **primary source document** — the lossless representation of that state
as it actually exists. The primary source documents are:

- **I**: The append-only event logs (Claude Code JSONL, conversation transcripts, meeting
  notes). Every intent that was ever expressed lives here, in full context.
- **S**: The DDIS specification (structured elements with IDs, cross-references,
  falsification conditions). Every formal claim about the system lives here.
- **P**: The codebase (source files, test files, build configuration). Every line of
  executable behavior lives here.

### The Triangle

```
           I
          ╱ ╲
    F_IS ╱   ╲ B_PI
        ╱     ╲
       ╱       ╲
      S ——————— P
         F_SP
         B_PS
```

Six morphisms connect the three states in bilateral pairs. Each pair forms an adjunction
(§5). The triangle **does not commute** in general — going I → S → P and going I → P
directly produce different results. The non-commutativity is precisely the divergence
that DDIS exists to detect and resolve.

---

## 2. Ground Truths: Three Primary Representations

<a name="2-ground-truths"></a>

Each state's ground truth has a distinct algebraic structure. Understanding these structures
is prerequisite to formalizing the morphisms between them.

### 2.1 Intent: The Free Monoid E*

The ground truth of intent is the **complete history of all conversations**, decisions,
and reasoning that produced the current understanding of what the system should do.

**Structure**: Let `E` be the type of conversational events (user messages, assistant
responses, tool calls, tool results, system prompts, compaction summaries). The intent
ground truth is an element of `E*`, the free monoid over `E`:

```
(E*, ·, ε)

where:
  ·  : E* × E* → E*     (concatenation)
  ε  : E*                (empty log)

Properties:
  Associativity: (a · b) · c = a · (b · c)
  Identity: ε · a = a · ε = a
  Monotonicity: ∀t₁ < t₂: log(t₁) ≤ log(t₂)   (prefix order)
```

**Key properties**:
- **Append-only**: The log grows monotonically. No event is ever deleted or modified.
  (This is the same C1 constraint as the datom store.)
- **Ordered**: Events have a total order by position. Causality is captured by ordering.
- **Multi-source**: Intent may span many conversations, many agents, many humans.
  The log is the **union** of all event streams (itself a monoid homomorphism).
- **Ephemeral context**: Any single agent sees only a **projection** φ : E* → E*|w of
  the full log, where w is the context window size. The full log is accessible through
  the runtime (Channel 2 in the AGENTIC_SYSTEMS analysis), but the agent's "working
  memory" is always a lossy compression.

**Compaction as quotient** (from AGENTIC_SYSTEMS_FORMAL_ANALYSIS.md):

```
compact : E* → Summary
inject  : Summary → E

Effective log after compaction:
  l' = [inject(compact(l_old))] · l_new

Behavioral equivalence invariant:
  ∀ op : π(l_old · l_new)(op) ≈ π([compact(l_old)] · l_new)(op)
```

### 2.2 Specification: The Coherence Graph

The ground truth of specification is the **structured set of formal elements** — each
individually addressable, cross-referenced, and equipped with falsification conditions.

**Structure**: Let `Elem` be the type of specification elements. The specification is a
labeled directed graph:

```
S = (V, E, τ, ρ, χ)

where:
  V : Set<Elem>              -- vertices (spec elements: INV, ADR, NEG, UNC, etc.)
  E : Set<(Elem, Label, Elem)>  -- edges (cross-references, dependencies, traces)
  τ : Elem → ElemType        -- typing function (invariant, ADR, negative case, etc.)
  ρ : Elem → [0,1]           -- confidence/certainty for each element
  χ : Elem → Set<Elem>       -- contradiction adjacency (elements that may conflict)
```

**Key properties**:
- **Self-consistency** (C1 from five-point coherence): ¬∃ (e₁, e₂) ∈ V² such that
  e₁ contradicts e₂ and both have confidence > threshold.
- **Completeness**: Every goal traces to invariants and back (coverage).
- **Falsifiability** (C6): Every invariant has an explicit violation condition.
- **Structured identity**: Every element has a unique ID (`INV-NS-NNN`, `ADR-NS-NNN`).
- **Schema-as-data**: When the datom store exists, the specification becomes data IN
  the store — the graph structure is itself queryable (C7 self-bootstrap).

**The specification is NOT a monoid.** Unlike the log, it does not have a natural
concatenation operation. It is closer to an **algebra over a signature** — the set of
elements satisfies constraints (no contradictions, complete traceability, falsifiability)
that can be checked mechanically. Adding an element may violate constraints, making the
operation partial: `add : S × Elem → S + Error`.

### 2.3 Implementation: The Executable Forest

The ground truth of implementation is the **codebase**: source files, test files, build
configuration, and the test oracle (the function that determines whether a test passes).

**Structure**: Let `File` be the type of source files, `Test` be the type of test cases,
and `Oracle : Test → {pass, fail}` be the test oracle.

```
P = (F, T, ω, σ)

where:
  F : Set<File>           -- source files (abstract syntax trees)
  T : Set<Test>           -- test cases
  ω : T → {pass, fail}   -- test oracle (execution)
  σ : F → Set<Behavior>  -- semantic function (what the code does)
```

**Key properties**:
- **Executability**: The codebase compiles and the test oracle can be evaluated.
- **Behavioral observability**: The implementation's meaning is determined by what it
  *does*, not what it *says*. σ is the denotational semantics; ω provides empirical
  evidence about σ.
- **Mutation**: Unlike the log and the store, the implementation IS mutable. Files are
  overwritten, tests are modified, code is refactored. History is maintained externally
  (git), not intrinsically.

**Note on mutability**: The implementation's mutability creates an asymmetry with the
other two states. I and S (in DDIS's formulation) are append-only; P is not. This
asymmetry is load-bearing: the implementation is the state that *changes*, and the
other two states exist to *govern* those changes.

### 2.4 Structural Summary

| Property | Intent (I) | Specification (S) | Implementation (P) |
|----------|-----------|-------------------|-------------------|
| Algebraic structure | Free monoid | Constrained graph | Mutable forest + oracle |
| Growth mode | Append-only | Append + retract | Mutable (overwrite) |
| Identity | By position | By ID (INV-NS-NNN) | By path (file:line) |
| Consistency check | None (raw) | Contradiction detection | Compilation + tests |
| Persistence | JSONL files | Datom store / markdown | Filesystem + git |
| Intrinsic ordering | Total (temporal) | Partial (dependency) | None (set of files) |

---

## 3. The Category of Coherence States

<a name="3-category-of-coherence-states"></a>

### 3.1 Definition

Define the category **Coh** (Coherence) with:

- **Objects**: Three coherence states {I, S, P}
- **Morphisms**: Bilateral functors between each pair (§4)
- **Composition**: Sequential application of morphisms
- **Identity**: The identity morphism on each state is the trivial "no transformation"

This is a small category (three objects), but the morphisms carry rich structure.
Each morphism is not merely a function but a **functor** between categories of
states — it maps objects (states) to objects and morphisms (state transitions) to
morphisms.

### 3.2 States as Categories

More precisely, each coherence state is itself a category:

**Cat_I** (Intent category):
- Objects: Conversation states (elements of E*, i.e., prefixes of the event log)
- Morphisms: Conversation extensions (append operations: l → l · e)
- Composition: Sequential extension (l · e₁ · e₂)
- This is the free category generated by E (each event is a generator)

**Cat_S** (Specification category):
- Objects: Specification states (valid graphs satisfying coherence constraints)
- Morphisms: Spec modifications (element additions, revisions, retractions)
- Composition: Sequential modification
- NOT free — constrained by coherence (some compositions are invalid)

**Cat_P** (Implementation category):
- Objects: Codebase states (compilable source trees)
- Morphisms: Code changes (commits, patches, refactors)
- Composition: Sequential change (patch application)
- NOT free — constrained by compilability and test passage

### 3.3 The Enrichment

The morphisms between states are enriched over **Set × [0,1]**: each morphism carries
both a transformation (the set component) and a **confidence/loss metric** (the [0,1]
component) measuring how much information is preserved or lost in the transformation.

A morphism m : A → B with confidence 1.0 is lossless (an embedding). A morphism with
confidence < 1.0 is lossy (a projection). The confidence metric threads through
composition: `conf(g ∘ f) ≤ min(conf(f), conf(g))` — confidence can only decrease
through composition.

This enrichment captures the fundamental reality that transformations between states
are inherently lossy. Intent contains nuance that specification cannot capture. Specification
contains rationale that code cannot express. The divergence at each boundary is precisely
the information lost by the morphism.

---

## 4. Bilateral Functors: The Six Morphisms

<a name="4-bilateral-functors"></a>

Each pair of states has two morphisms — a **forward functor** (the natural direction of
work flow) and a **backward functor** (the verification/feedback direction). Together
they form a **bilateral pair**.

### 4.1 The I ↔ S Pair: Harvest and Seed

**F_IS : Cat_I → Cat_S (Harvest / Formalize)**

Takes conversational intent and crystallizes it into formal specification elements.

```
F_IS(conversation) = {
  1. DETECT: Scan conversation for implicit claims, decisions, dependencies
  2. EXTRACT: Generate spec element candidates (INV, ADR, NEG, UNC)
  3. FORMALIZE: Assign IDs, write falsification conditions, establish cross-references
  4. VALIDATE: Check new elements against existing spec for contradictions
  5. COMMIT: Add validated elements to the specification
}
```

**Properties**:
- **Lossy**: Not everything in a conversation is spec-worthy. Context, reasoning,
  false starts, exploration — all are discarded. The specification retains only
  the crystallized commitments.
- **Creative**: Formalization requires judgment. The same conversation may yield
  different spec elements depending on the formalizer. This is NOT a defect — it
  reflects the genuine underdetermination of the intent→spec boundary.
- **Monotonic** (within S): F_IS only adds elements to S (possibly with retractions,
  which are themselves additions in DDIS's append-only model).
- **Quality-measurable**: Harvest quality = false positive rate (committed then
  retracted) + false negative rate (rejected then re-discovered). See spec/05-harvest.md.

**B_SI : Cat_S → Cat_I (Seed / Contextualize)**

Takes formal specification elements and projects them into conversation context.

```
B_SI(spec, agent, task) = {
  1. ASSOCIATE: Find spec elements relevant to the agent's current task
  2. ASSEMBLE: Compress to fit the agent's attention budget (k*)
  3. FORMAT: Generate dynamic CLAUDE.md or seed document
  4. INJECT: Present to the agent at session start
}
```

**Properties**:
- **Lossy**: The full spec exceeds any agent's context window. ASSEMBLE must
  compress, prioritize, and drop. The projection is budget-aware (spec/06-seed.md).
- **Selective**: Relevance-weighted. Not all elements are equally important for
  every task. ASSOCIATE uses the knowledge graph topology to find the relevant
  neighborhood.
- **Periodic**: Seed is a session-boundary operation, not continuous. Knowledge
  is injected at session start and through guidance footers during the session.
- **Bilateral to harvest**: B_SI(F_IS(conversation)) ≈ conversation (the roundtrip
  preserves essential intent — the adjunction condition, §5.1).

### 4.2 The S ↔ P Pair: Implement and Verify

**F_SP : Cat_S → Cat_P (Implement)**

Takes specification elements and produces code that satisfies them.

```
F_SP(spec_element) = {
  1. INTERPRET: Understand the invariant's statement and falsification condition
  2. DESIGN: Choose an implementation strategy (one of potentially many)
  3. CODE: Write source files that realize the strategy
  4. TEST: Write test cases that check the falsification condition
  5. VERIFY: Compile, run tests, confirm the invariant holds
}
```

**Properties**:
- **Highly creative**: Many implementations satisfy the same spec. The specification
  constrains but does not determine the implementation. The implementation choices
  not captured in the spec become the "structural divergence" that B_PS detects.
- **Non-unique**: Two agents implementing the same spec independently will produce
  different code. Both may be correct. This is the aleatory divergence type.
- **Traceable**: Every implementation artifact traces to at least one spec element (C5).
  Untraceable code is a spec gap.

**B_PS : Cat_P → Cat_S (Drift / Verify)**

Takes the implementation and checks it against the specification in both directions.

```
B_PS(impl, spec) = {
  FORWARD:  For each spec element, does the impl satisfy it?
            Unimplemented elements → structural gaps
  BACKWARD: For each impl artifact, does it trace to a spec element?
            Untraceable code → spec gaps (potential absorption candidates)
}
```

**Properties**:
- **Conservative**: No false negatives (CR-001). Every genuine divergence is detected.
  False positives are acceptable (and resolved by documentation as residuals).
- **Bilateral**: Checks in BOTH directions. Forward finds unimplemented spec. Backward
  finds unspecified code. This is the key insight from spec/10-bilateral.md.
- **Non-constructive**: B_PS detects divergence but does not resolve it. Resolution
  is a separate operation (either implement the missing spec or absorb the unspecified
  code into the spec).

### 4.3 The I ↔ P Pair: Direct Coding and Learning

**F_IP : Cat_I → Cat_P (Direct Coding)**

Takes intent directly to implementation without formal specification.

```
F_IP(intent) = {
  1. INTERPRET: Understand what the user/agent wants
  2. CODE: Write code that (hopefully) satisfies the intent
  3. TEST: Write tests based on informal understanding
}
```

**Properties**:
- **Highest loss**: No formal specification to mediate. The gap between intent and
  code is bridged entirely by the implementer's understanding.
- **Fastest**: No specification overhead. This is how most software is written in
  practice — and why most software has divergence problems.
- **Unverifiable**: Without a formal spec, there is no mechanical way to check
  whether the code matches the intent. Verification is purely subjective.
- **DDIS discourages this morphism**: The entire point of DDIS is to route work
  through S. The existence of F_IP is an empirical fact, not a design goal.

**B_PI : Cat_P → Cat_I (Learn / Debug)**

Takes implementation behavior and feeds it back to intent, updating understanding.

```
B_PI(impl_behavior) = {
  1. OBSERVE: Run the code, observe its behavior
  2. COMPARE: Does the behavior match what was intended?
  3. UPDATE: Revise understanding based on observed behavior
  4. RECORD: Add observations to the conversation log
}
```

**Properties**:
- **Empirical**: Driven by observation, not formal analysis.
- **Corrective**: Often triggered by failures (test failures, bugs, unexpected
  behavior). Success cases are less informative.
- **Feeds the triangle**: Observations from B_PI inform both direct intent revision
  and specification revision (through F_IS applied to the updated intent).

### 4.4 The Six Morphisms — Summary Table

| Morphism | Direction | Operation | Lossy? | Creative? | DDIS Support |
|----------|-----------|-----------|--------|-----------|-------------|
| F_IS | I → S | Harvest / Formalize | Yes | Yes | spec/05-harvest.md |
| B_SI | S → I | Seed / Contextualize | Yes | No | spec/06-seed.md |
| F_SP | S → P | Implement | Yes | Highly | spec/10-bilateral.md |
| B_PS | P → S | Drift / Verify | No (conservative) | No | spec/10-bilateral.md |
| F_IP | I → P | Direct Coding | Very high | Highly | Discouraged |
| B_PI | P → I | Learn / Debug | Moderate | No | Implicit |

---

## 5. Adjunctions: Three Convergence Pairs

<a name="5-adjunctions"></a>

Each bilateral pair forms an **adjunction** — a pair of functors with a natural relationship
between them. The adjunction captures the precise sense in which the forward and backward
morphisms are "best possible approximations" of each other.

### 5.1 The Harvest/Seed Adjunction: F_IS ⊣ B_SI

**Claim**: The harvest functor F_IS is left adjoint to the seed functor B_SI.

```
F_IS ⊣ B_SI

Unit:    η_I : Id_I → B_SI ∘ F_IS
Counit:  ε_S : F_IS ∘ B_SI → Id_S
```

**The unit η_I**: For any conversation state c ∈ Cat_I:

```
η_I(c) : c → B_SI(F_IS(c))
```

This says: "Take a conversation, harvest it into spec elements, then seed those spec
elements back into a new conversation. The result is a *refinement* of the original
conversation — it contains the essential intent, stripped of ephemera."

η_I is NOT an isomorphism. The composition B_SI(F_IS(c)) loses context, false starts,
exploratory reasoning. What survives is the crystallized knowledge. The unit measures
the **epistemic gap** — the difference between raw intent and formalized knowledge.

**The counit ε_S**: For any spec state s ∈ Cat_S:

```
ε_S(s) : F_IS(B_SI(s)) → s
```

This says: "Take a spec, seed it into a conversation, then harvest back. The result
should be (at most) the original spec."

ε_S is closer to an isomorphism — a well-written spec, when seeded and re-harvested,
should reproduce itself. The gap (ε_S not being an isomorphism) represents:
- Spec elements so implicit they aren't re-extracted from the seeded context
- New insights discovered during the re-engagement that extend the spec

**Triangle identities** (adjunction laws):

```
F_IS ∘ η_I = ε_S ∘ F_IS     (left triangle)
η_I ∘ B_SI = B_SI ∘ ε_S     (right triangle)
```

These say: "Harvesting a seeded conversation, then harvesting again, is the same as
just harvesting once and applying the counit." In operational terms: you don't get
new spec elements by harvesting the same knowledge twice.

### 5.2 The Implement/Verify Adjunction: F_SP ⊣ B_PS

**Claim**: The implement functor F_SP is left adjoint to the verify functor B_PS.

```
F_SP ⊣ B_PS

Unit:    η_S : Id_S → B_PS ∘ F_SP
Counit:  ε_P : F_SP ∘ B_PS → Id_P
```

**The unit η_S**: For any spec state s ∈ Cat_S:

```
η_S(s) : s → B_PS(F_SP(s))
```

"Implement the spec, then verify the implementation against the spec. The result is the
spec as reflected through the implementation — enriched with implementation-specific
details that reveal spec gaps."

This is the **bilateral loop** of spec/10-bilateral.md. The gap between s and η_S(s)
is the structural divergence: spec elements that the implementation doesn't satisfy
(forward gaps) and implementation behavior that the spec doesn't cover (backward gaps).

**The counit ε_P**: For any impl state p ∈ Cat_P:

```
ε_P(p) : F_SP(B_PS(p)) → p
```

"Verify the implementation against the spec, then re-implement from the verified spec.
The result should approximate the original implementation."

ε_P captures **implementation stability**: code that is well-aligned with the spec is
unchanged by a verify-then-reimplement cycle. Code that is poorly aligned would be
substantially rewritten.

**This is the most developed adjunction in the current DDIS design.** The bilateral
loop (spec/10-bilateral.md) IS the iteration of (B_PS ∘ F_SP) to fixpoint, with the
fitness function F(S) measuring progress.

### 5.3 The Direct/Learn Adjunction: F_IP ⊣ B_PI

**Claim**: The direct coding functor F_IP is left adjoint to the learning functor B_PI.

```
F_IP ⊣ B_PI

Unit:    η_I' : Id_I → B_PI ∘ F_IP
Counit:  ε_P' : F_IP ∘ B_PI → Id_P
```

**The unit η_I'**: "Code directly from intent, then observe what the code does and update
intent. The result is intent *informed by implementation reality*."

This is what happens when you "try something and see if it works." The gap is large — many
aspects of intent are not testable through direct observation. But the insights from
B_PI(F_IP(intent)) can be profound: the code reveals constraints, edge cases, and
interactions that the original intent did not anticipate.

**The counit ε_P'**: "Observe what the code does, update understanding, then recode from
the updated understanding."

This is **debugging**: observe behavior, understand what went wrong, fix the code. The
gap between the original code and the recoded version is the bug.

**This adjunction is the weakest of the three.** It has the highest loss in both
directions. DDIS's central design thesis is that routing through S transforms this
weak adjunction into the composition of two stronger ones.

### 5.4 The Factorization Theorem

**Theorem (Mediation)**: The direct adjunction (F_IP ⊣ B_PI) factors through the
specification:

```
F_IP ≅ F_SP ∘ F_IS     (implement ∘ harvest)
B_PI ≅ B_SI ∘ B_PS     (seed ∘ verify)
```

with the factored version having strictly better convergence properties.

**Proof sketch**: The factored path I → S → P goes through a state (S) that has:
1. **Falsification conditions**: Every claim in S can be mechanically checked
2. **Contradiction detection**: Internal consistency of S is verifiable
3. **Traceability**: Every element in S traces to both I and P
4. **Convergence guarantees**: The bilateral loops on each adjunction are monotonic

The direct path I → P lacks all four. The information lost by F_IP is strictly greater
than the information lost by F_SP ∘ F_IS, because the specification preserves the
*formal structure* of the intent that raw code discards.

**This is the central theorem of DDIS**: the specification is not overhead — it is the
**universal mediating object** that makes coherence verification possible. Every
coherent I → P path factors through S. Paths that bypass S are unverifiable.

---

## 6. The Universal Convergence Cycle

<a name="6-universal-convergence-cycle"></a>

The five-step convergence cycle is the **universal pattern** for computing adjunction
fixpoints between any pair of coherence states.

### 6.1 The General Pattern

For any bilateral pair (F : A → B, B : B → A) with adjunction F ⊣ B:

```
CONVERGE(A, B):

  Step 1 — LIFT (Observation)
    Compute the divergence since last convergence.
    Δ_A = changes to A since last cycle
    Δ_B = changes to B since last cycle

  Step 2 — CONVERT (Provisional Transformation)
    Apply the forward and backward functors to the deltas:
    Δ_B' = F(Δ_A)    -- A-changes projected into B-space
    Δ_A' = B(Δ_B)    -- B-changes projected into A-space
    These are provisional: they may conflict with each other or with existing state.

  Step 3 — ASSESS (Bilateral Integration)
    Check for conflicts:
    conflicts_AB = Δ_B' ∩ conflicts(B ∪ Δ_B)    -- A-derived changes conflicting with B
    conflicts_BA = Δ_A' ∩ conflicts(A ∪ Δ_A)    -- B-derived changes conflicting with A
    Check for consistency:
    contradictions = detect_contradictions(A ∪ Δ_A', B ∪ Δ_B')

  Step 4 — RESOLVE (Conflict Resolution)
    For each conflict, invoke the appropriate resolution mechanism:
    - Automated: lattice resolution for lattice-typed attributes
    - Deliberation: structured argument for semantic conflicts
    - Escalation: human decision for axiological conflicts
    Topology determines who resolves: self-review, peer review, swarm, hierarchical.

  Step 5 — APPLY (Commit to Ground Truth)
    Apply the resolved changes to both ground truth documents:
    A' = A ∪ resolved(Δ_A')
    B' = B ∪ resolved(Δ_B')
    Record the convergence event (provenance, participants, conflicts resolved).

POST: D(A', B') ≤ D(A, B)    -- divergence never increases (Lyapunov condition, §7)
```

### 6.2 Instantiation: I ↔ S (Harvest/Seed Cycle)

```
CONVERGE(I, S) — The Harvest Cycle:

  Step 1 — LIFT: Scan recent conversations for un-transacted knowledge.
    Identify observations, decisions, dependencies, uncertainties that
    the agent has in context but that the store does not have as datoms.
    Δ_I = new conversation events since last harvest
    Δ_S = spec elements added/modified since last seed

  Step 2 — CONVERT:
    F_IS(Δ_I) = harvest candidates (proposed spec elements)
    B_SI(Δ_S) = new context to inject (elements the agent should know about)

  Step 3 — ASSESS:
    Do any harvest candidates contradict existing spec elements?
    Do any new spec elements invalidate conversation-level assumptions?
    Quality check: confidence ≥ threshold, stability guard, commitment weight.

  Step 4 — RESOLVE:
    Low-confidence candidates → mark with UNC, don't commit
    Contradictions → trigger deliberation (spec/11-deliberation.md)
    Novel insights → commit with provenance

  Step 5 — APPLY:
    Commit validated candidates to the spec (datom store)
    Inject new spec context into the next seed
    Record harvest session entity with quality metrics
```

### 6.3 Instantiation: S ↔ P (Bilateral Loop)

```
CONVERGE(S, P) — The Bilateral Cycle:

  Step 1 — LIFT: Detect all changes since last bilateral cycle.
    Δ_S = spec elements added/modified since last cycle
    Δ_P = code changes (commits) since last cycle

  Step 2 — CONVERT:
    F_SP(Δ_S) = implementation requirements (what needs to be coded)
    B_PS(Δ_P) = verification results (what the code reveals about spec alignment)

  Step 3 — ASSESS:
    Forward scan: which spec elements are unimplemented?
    Backward scan: which implementation artifacts are unspecified?
    Compute fitness F(S) (spec/10-bilateral.md §10.1)

  Step 4 — RESOLVE:
    Unimplemented spec → create implementation tasks
    Unspecified code → either absorb into spec or flag as divergence
    Contradictions → trigger deliberation

  Step 5 — APPLY:
    Update divergence map
    Document residuals (persistent gaps with rationale)
    Emit signals for unresolved gaps
```

### 6.4 Instantiation: I ↔ P (Direct Feedback)

```
CONVERGE(I, P) — The Empirical Cycle:

  Step 1 — LIFT: Observe implementation behavior.
    Δ_I = new intent/goals expressed since last cycle
    Δ_P = new code behavior observed (test results, runtime behavior)

  Step 2 — CONVERT:
    F_IP(Δ_I) = expected behavior (informal test expectations)
    B_PI(Δ_P) = updated understanding (what the code actually does)

  Step 3 — ASSESS:
    Does the code behavior match the intent?
    Are there behaviors not anticipated by any intent?
    Are there intents not addressed by any behavior?

  Step 4 — RESOLVE:
    Mismatches → either fix code or revise intent
    Unanticipated behavior → decide if it's a bug or a feature
    Unaddressed intent → decide if it's important

  Step 5 — APPLY:
    Update conversation context with empirical observations
    Update implementation to match revised intent
    (Ideally, ALSO update specification — routing through S)
```

### 6.5 Cycle Composition

The three cycles are not independent. They compose:

```
FULL_CONVERGENCE = CONVERGE(I,S) ; CONVERGE(S,P) ; CONVERGE(I,P)
```

A full convergence pass runs all three bilateral cycles. The order matters:
I↔S first (get intent into spec), then S↔P (get spec into impl), then I↔P
(verify intent matches impl end-to-end).

The I↔P cycle acts as a **checksum**: if the first two cycles worked perfectly,
I↔P should find zero divergence. Any divergence found by I↔P that was not found
by I↔S or S↔P indicates a gap in the specification's coverage — the spec failed
to mediate some aspect of the intent-to-implementation translation.

---

## 7. Lyapunov Stability: The Energy Argument

<a name="7-lyapunov-stability"></a>

The user's energy metaphor formalizes as a **Lyapunov stability argument**: the system
has a potential function that decreases monotonically through convergence cycles, guaranteeing
convergence to an equilibrium.

### 7.1 The Potential Function

Define the **total divergence potential**:

```
Φ(I, S, P) = w₁·D(I,S) + w₂·D(S,P) + w₃·D(I,P)

where:
  D(A,B) = divergence between states A and B (sum of gaps × severity)
  w₁, w₂, w₃ = boundary weights (from spec/10-bilateral.md: the wᵢ in the
                four-boundary divergence measure)
```

Φ is a Lyapunov function candidate. We need to show:

1. **Φ ≥ 0**: Divergence is always non-negative. ✓ (by construction)
2. **Φ = 0 iff coherent**: Zero divergence iff all three pairs are aligned. ✓
3. **Φ decreases through cycles**: Each convergence cycle reduces total divergence.

### 7.2 Monotonic Decrease (The Key Property)

**Theorem (Lyapunov Convergence)**: For each convergence cycle,

```
Φ(I', S', P') ≤ Φ(I, S, P)
```

with equality iff the cycle found no unresolved divergence.

**Proof sketch**:

Each convergence cycle (§6.1) has five steps. Steps 1-3 (Lift, Convert, Assess) do not
modify state — they only observe. Steps 4-5 (Resolve, Apply) modify state in one of
two ways:

(a) **Resolution**: A divergence is resolved. The gap is eliminated.
    D(A', B') < D(A, B). Φ decreases strictly.

(b) **Documentation**: A divergence is documented as a residual with explicit rationale
    and uncertainty marker. The gap persists but is now *acknowledged*.
    This does not decrease D directly, but it prevents the gap from growing
    (the residual has a stability guard) and it increases the fitness function F(S)
    (acknowledged residuals contribute positively to the coherence measure).

Neither operation increases divergence. Resolution decreases it; documentation
stabilizes it. Therefore Φ is non-increasing. ∎

### 7.3 The Energy Metaphor, Formalized

| Informal Concept | Formal Counterpart |
|-------------------|-------------------|
| **Potential energy** | Φ(I, S, P) — total divergence across all boundaries |
| **Kinetic energy** | The work done during Steps 1-4 of CONVERGE — observation, conversion, assessment, resolution |
| **Equilibrium** | Φ = 0 (or Φ = Φ_residual with all residuals documented) |
| **Work creates divergence** | Forward progress (new code, new ideas, new spec elements) increases Φ in the short term |
| **Harvesting converts potential to kinetic** | The convergence cycle converts divergence (potential) into resolution work (kinetic) |
| **Equilibrium absorbs kinetic energy** | Resolved changes are committed to ground truth documents, reducing Φ back toward 0 |

### 7.4 The Work-Divergence Tradeoff

A fundamental tension exists: **forward progress always increases divergence temporarily**.

```
When an agent writes code (I → P), D(S,P) increases (code without spec backing).
When a human expresses intent (I grows), D(I,S) increases (intent without spec coverage).
When a spec element is added (S grows), D(S,P) increases (spec without implementation).
```

The convergence cycle absorbs this divergence. The rate of absorption determines the
sustainable rate of forward progress:

```
Sustainable if: rate(CONVERGE) ≥ rate(divergence_creation)
Unstable if:    rate(CONVERGE) < rate(divergence_creation)
```

When divergence accumulates faster than convergence can absorb it, the system enters
an **incoherent regime** — the spec doesn't match the code, the code doesn't match the
intent, and increasing effort goes to untangling inconsistencies rather than making progress.

DDIS's mechanisms (guidance injection, harvest warnings, dynamic CLAUDE.md, fitness
function monitoring) are designed to **keep the system in the convergent regime** by:
1. Making convergence cheap (one CLI call to harvest, automated verification)
2. Making divergence visible (fitness function, drift metrics, signal system)
3. Preventing divergence accumulation (proactive harvest warnings at 70%/85%/95% context)

### 7.5 Convergence Rate

The convergence rate depends on the structure of the adjunctions:

```
For the I↔S adjunction:
  Rate limited by: harvest quality (FP/FN rates), agent attention budget
  Typical cycle: session-boundary (every 20-30 turns)
  Bottleneck: formalization creativity (turning informal intent into formal spec)

For the S↔P adjunction:
  Rate limited by: implementation speed, test coverage, verification completeness
  Typical cycle: commit-boundary (every meaningful code change)
  Bottleneck: implementation correctness (ensuring code satisfies spec)

For the I↔P adjunction:
  Rate limited by: observation bandwidth, test expressiveness
  Typical cycle: test-run-boundary (every test execution)
  Bottleneck: the behavioral gap (intent is richer than tests can express)
```

---

## 8. The Mediation Theorem: Why Specification is Central

<a name="8-mediation-theorem"></a>

### 8.1 The Universal Property of S

**Claim**: The specification state S is the **universal mediating object** in the
coherence triangle. Every coherent morphism I → P factors through S.

More precisely: for any morphism f : I → P that produces a coherent implementation
(one that can be verified against intent), there exists a unique pair (f₁ : I → S, f₂ : S → P)
such that f ≈ f₂ ∘ f₁ and the factored path is verifiable.

```
     I ----f---→ P        Every coherent f factors as:
      \        ↗
   f₁  \    / f₂          I ---f₁--→ S ---f₂--→ P
        ↘  /
         S                 where f₁ = F_IS (harvest) and f₂ = F_SP (implement)
```

### 8.2 Why the Factorization is Strictly Better

The direct path f : I → P has divergence potential:

```
D_direct(I, P) = D(I, P)      -- single, large gap
```

The factored path has divergence potential:

```
D_factored(I, S, P) = D(I, S) + D(S, P)
```

Crucially, **D(I, S) + D(S, P) ≤ D(I, P)** is NOT guaranteed by the triangle inequality
alone — divergence is not a metric in the strict mathematical sense. Instead, the factored
path is better because of three structural properties:

1. **Verifiability**: D(I, S) can be measured (harvest quality metrics).
   D(S, P) can be measured (bilateral loop, fitness function).
   D(I, P) CANNOT be measured mechanically — it requires subjective judgment.

2. **Locality**: Fixing D(I, S) requires only spec work. Fixing D(S, P) requires
   only implementation work. Fixing D(I, P) requires both simultaneously.

3. **Monotonic convergence**: Both sub-adjunctions (I↔S, S↔P) have convergence
   guarantees (§7). The direct adjunction (I↔P) does not.

### 8.3 The Role of S in Multi-Agent Settings

In a multi-agent setting, S becomes even more critical. Multiple agents can independently:
- Harvest intent into S (F_IS applied by different agents)
- Implement from S (F_SP applied by different agents)
- Verify against S (B_PS applied by different agents)

S is the **shared reference point**. Without it, agents working on the same project
have no way to verify that their work is coherent with each other's work. With S,
coherence is checkable: do all implementations satisfy the same spec?

The CRDT property of the datom store (G-Set under set union) ensures that independent
additions to S merge cleanly. Conflicts are surfaced at merge time and resolved through
deliberation. The specification IS the coordination substrate.

---

## 9. Divergence Dynamics: Work as Energy Storage

<a name="9-divergence-dynamics"></a>

### 9.1 The Phase Space

The system evolves in a phase space where each axis represents divergence at a boundary:

```
Phase point: (D_IS, D_SP, D_IP) ∈ ℝ³≥₀

Equilibrium: (0, 0, 0)    -- or (r₁, r₂, r₃) with documented residuals
```

Forward progress (work) moves the phase point away from equilibrium. Convergence cycles
move it back. The trajectory through phase space characterizes the project's coherence
dynamics.

### 9.2 Typical Trajectories

**Healthy trajectory** (DDIS-governed):

```
Phase 0: (0, 0, 0)        -- project start, trivially coherent
Phase 1: (δ, 0, δ)        -- new intent, no spec yet. D_IS and D_IP increase.
Phase 2: (0, δ, δ)        -- harvest. Intent formalized. D_IS → 0, D_SP increases.
Phase 3: (0, 0, 0)        -- implement + verify. D_SP → 0. Full coherence restored.
```

Each phase alternates between divergence (work) and convergence (verification). The
amplitude of the oscillation depends on how much work is done between convergence cycles.

**Unhealthy trajectory** (no DDIS):

```
Phase 0: (0, 0, 0)        -- project start
Phase 1: (δ₁, 0, δ₁)     -- new intent
Phase 2: (δ₁, δ₂, δ₁+δ₂) -- code written without spec update. Divergence accumulates.
Phase 3: (2δ₁, δ₂+δ₃, 3δ) -- more intent, more code, no convergence. Exponential growth.
...
Phase N: (Nδ, Nδ, Nδ)     -- project is incoherent. Most effort goes to untangling.
```

Without convergence cycles, divergence accumulates monotonically. This is the "entropy"
of software projects — the inevitable decay of coherence under work.

### 9.3 The Convergence Budget

Every convergence cycle has a cost (agent time, human attention, compute). The budget
for convergence bounds the sustainable rate of forward progress:

```
Budget_convergence ≥ ∫ rate(divergence_creation) dt

If violated: divergence accumulates → incoherent regime
If satisfied: divergence bounded → convergent regime
```

DDIS minimizes convergence cost by:
- **Automation**: Bilateral scans, contradiction detection, fitness computation are
  automated Datalog queries
- **Incrementalism**: Each cycle only processes deltas since last cycle
- **Prioritization**: Signal system routes the most important divergences first
- **Amortization**: Harvest/seed at session boundaries amortizes cost over many turns

---

## 10. Scaling Properties: The Triangle at Scale

<a name="10-scaling-properties"></a>

### 10.1 Separation of Concerns

The three states have different scaling characteristics:

| State | Scales with | Bottleneck | Parallelizable? |
|-------|------------|------------|-----------------|
| I (Intent) | Number of stakeholders × conversation volume | Human attention | Yes (many conversations) |
| S (Spec) | System complexity (elements × cross-refs) | Formalization effort | Partially (by namespace) |
| P (Impl) | Codebase size (files × lines) | Implementation effort | Highly (by module) |

The convergence cycles between pairs can operate **independently and in parallel**:
- I↔S cycle runs at session boundaries for each agent
- S↔P cycle runs at commit boundaries for each module
- I↔P cycle runs at test boundaries for each test suite

### 10.2 The Triangle at Multi-Agent Scale

With N agents working on the same project:

```
Each agent αᵢ maintains:
  - I_αᵢ : Their own conversation history (private)
  - S    : Shared specification (via CRDT merge)
  - P_αᵢ : Their working copy of the implementation (merge via git)

Convergence cycles:
  - I_αᵢ ↔ S : Each agent harvests independently (F_IS applied by each)
  - S ↔ P   : Implementation verified against shared spec
  - I_αᵢ ↔ P_αᵢ : Each agent verifies their own intent-code alignment
```

The specification S acts as the **consensus substrate**: even though agents have
different intents (different conversations), they share the same spec. The spec IS
the formal agreement about what the system should do.

Conflicts between agents surface at two points:
1. **Merge of specifications**: When two agents harvest contradictory spec elements,
   the contradiction is detected and triggers deliberation
2. **Merge of implementations**: When two agents implement differently, the bilateral
   loop detects divergence against the shared spec

Both conflict points are mediated by S. Without S, agent coordination reduces to
"whoever pushes last wins" — which is exactly the problem DDIS exists to solve.

### 10.3 Human-AI Collaboration

The triangle model naturally accommodates mixed human-AI teams:

- **Humans** primarily operate in I (expressing intent, making decisions)
  and review S (approving spec elements, resolving deliberations)
- **AI agents** primarily operate across all three states (harvesting I→S,
  implementing S→P, verifying P→S, learning P→I)
- **The spec S is the shared language** between humans and AI

This separation is powerful: humans don't need to read code (P) to verify
that the system is coherent — they verify S (which is in natural language
with formal structure), and the bilateral loop verifies S ↔ P automatically.

---

## 11. Grounding in Existing Design

<a name="11-grounding"></a>

### 11.1 What Already Exists

| This Document | Existing Formalization | Source |
|--------------|----------------------|--------|
| S ↔ P adjunction | Bilateral loop as (F, B) adjunction with fixpoint | spec/10-bilateral.md §10.1 |
| D(S, P) measure | Four-boundary divergence formula | spec/10-bilateral.md §10.1 |
| Fitness function F(S) | 7-component weighted formula | spec/10-bilateral.md §10.1, ADRS CO-009 |
| Five-point coherence | C1–C5 | spec/10-bilateral.md §10.4 INV-BILATERAL-002 |
| Harvest pipeline | 5-stage DETECT→PROPOSE→REVIEW→COMMIT→RECORD | spec/05-harvest.md §5.2 |
| Seed assembly | ASSOCIATE→ASSEMBLE→FORMAT→INJECT | spec/06-seed.md |
| Crystallization guard | 6-condition stability check | spec/05-harvest.md §5.2 |
| Agent system model | (E, Op, Obs, S, π, δ, φ, compact) with 3 laws | AGENTIC_SYSTEMS_FORMAL_ANALYSIS.md |
| Reconciliation taxonomy | 8 divergence types | SEED.md §6 |
| Deliberation entities | Deliberation, Position, Decision | spec/11-deliberation.md |
| G-Set CRDT merge | (P(D), ∪) under set union | spec/07-merge.md |
| Resolution modes | Per-attribute lattice/LWW/multi-value | spec/04-resolution.md |
| Signal dispatch | 8 signal types → resolution mechanisms | spec/09-signal.md |
| Sync barriers | Consistent cuts, post-barrier determinism | spec/08-sync.md |
| Guidance comonad | (W, extract, extend) where W(A) = (StoreState, A) | spec/12-guidance.md |

### 11.2 What This Document Adds

| New Contribution | Extends |
|-----------------|---------|
| **Trilateral model**: Three states with bilateral pairs | Extends bilateral loop from S↔P to all three pairs |
| **I as first-class state**: Intent with algebraic structure (free monoid) | Previously implicit; now formalized |
| **Six morphisms**: Complete set of forward/backward functors | Only F_SP/B_PS were fully specified |
| **Three adjunctions**: F_IS⊣B_SI, F_SP⊣B_PS, F_IP⊣B_PI | Only F_SP⊣B_PS was formalized |
| **Universal convergence cycle**: 5-step pattern for all adjunctions | Previously specific to harvest and bilateral |
| **Lyapunov stability**: Formal proof of convergence | Previously stated as INV-BILATERAL-001, now proved for entire triangle |
| **Mediation theorem**: S as universal mediating object | Previously informal insight |
| **Phase space dynamics**: Trajectories through divergence space | New |
| **Scaling analysis**: How the triangle works at N-agent scale | New |
| **I↔P morphism**: Direct coding/learning formalized | Previously implicit and discouraged |

### 11.3 What This Resolves

Several open tensions from HARVEST_SEED.md (the 25 tensions found during extraction)
are clarified by this model:

- **T-02 (Harvest confidence vs. store invariants)**: The adjunction formalism clarifies
  that harvest operates at the I↔S boundary where confidence thresholds are intrinsic
  (unit η_I is lossy by construction; the crystallization guard is the ε-ball condition).

- **T-09 (Conservative detection vs. practical scalability)**: The Lyapunov argument
  shows that conservative detection (no false negatives) is necessary for monotonic
  convergence but can be implemented incrementally (delta-only verification).

- **T-17 (Bilateral symmetry vs. asymmetric boundaries)**: The trilateral model makes
  the asymmetry explicit: I→S is creative (formalization), S→P is constructive
  (implementation), but both have the same abstract structure (forward functor of an
  adjunction). Symmetry is at the categorical level, not the operational level.

---

## 12. Open Questions and Uncertainties

<a name="12-open-questions"></a>

### OQ-1: Is F_IS truly left adjoint to B_SI?

**Confidence**: 0.7
**Uncertainty type**: Algebraic

The harvest/seed pair has the *flavor* of an adjunction (universal mapping property,
unit/counit), but a rigorous proof requires showing that the natural transformations
satisfy the triangle identities. The practical challenge: harvest is creative (non-deterministic),
which means F_IS is not a functor in the strict sense — it's a **profunctor** or a
**non-deterministic functor**. The adjunction may need to be stated in a category enriched
over distributions or powersets rather than sets.

**Resolution criteria**: Either (a) show that non-determinism can be factored out
(e.g., by fixing a harvest strategy), or (b) weaken the adjunction to a Galois connection
or lax adjunction that accommodates non-determinism.

### OQ-2: What is the correct divergence metric?

**Confidence**: 0.6
**Uncertainty type**: Technical

The Lyapunov argument requires a well-defined divergence function D(A, B). The current
spec (10-bilateral.md) defines D for the S↔P boundary but not for I↔S or I↔P. For I↔S,
the "divergence" is the epistemic gap (un-harvested knowledge). For I↔P, it's the
behavioral gap (intent not matched by behavior). These are fundamentally different types
of measurement. Can they be unified into a single divergence metric, or do we need
three separate Lyapunov functions?

**Resolution criteria**: Define D(I,S) and D(I,P) as formal metrics and verify that
Φ = w₁D_IS + w₂D_SP + w₃D_IP satisfies the Lyapunov conditions.

### OQ-3: How should the I↔P morphisms be governed?

**Confidence**: 0.5
**Uncertainty type**: Design

DDIS discourages direct I→P coding because it bypasses verification. But B_PI (learning
from implementation) is valuable and hard to suppress. Should the system:
(a) Forbid F_IP entirely and route all work through S?
(b) Allow F_IP but mandate immediate convergence (F_IS of the B_PI observations)?
(c) Track F_IP usage as a divergence signal and apply guidance to redirect?

Option (c) seems most aligned with DDIS philosophy (detect divergence, don't forbid it),
but options (a) and (b) have simpler convergence proofs.

### OQ-4: The observed behavior boundary

**Confidence**: 0.8
**Uncertainty type**: Scope

SEED.md §2 identifies FOUR divergence boundaries:
Intent → Spec → Impl → **Observed Behavior**

This document treats only three states (I, S, P). Observed behavior is folded into P
(via the test oracle ω). Should observed behavior be a fourth state **O** with its own
ground truth (runtime logs, user reports, monitoring data)?

A four-state model would add three more bilateral pairs (I↔O, S↔O, P↔O), for a total
of 12 morphisms. This may be more accurate but significantly more complex. The current
formulation assumes ω (the test oracle) adequately represents observed behavior.

### OQ-5: Composition order of convergence cycles

**Confidence**: 0.6
**Uncertainty type**: Operational

§6.5 states that FULL_CONVERGENCE runs I↔S, then S↔P, then I↔P. But is this the optimal
order? Alternative orderings:
- S↔P first (ground in implementation reality before harvesting intent)
- All three in parallel (possible if cycles are independent)
- Adaptive (run the cycle with highest divergence first)

The optimal order may depend on the project phase (early: I↔S dominant; late: S↔P dominant).

---

## 13. Implications for Implementation

<a name="13-implications"></a>

### 13.1 What Needs to Be Built

The trilateral model identifies three convergence cycles. The S↔P cycle is well-designed
(spec/10-bilateral.md). The I↔S cycle has the harvest/seed pipeline (spec/05-harvest.md,
spec/06-seed.md). The I↔P cycle is currently implicit.

**Stage 0 requirements** (from this model):

1. **Divergence metrics**: Define D(I,S) formally. Currently the epistemic gap Δ(t) from
   spec/05-harvest.md serves this role. Verify it satisfies the Lyapunov condition.

2. **Convergence cycle orchestration**: A mechanism to run the three cycles in sequence
   (or parallel) and track total divergence Φ across cycles.

3. **I↔P cycle implementation**: At minimum, a mechanism to detect when agents are
   coding directly from intent without spec mediation, and a guidance signal to redirect
   them through S.

4. **Fitness function extension**: Extend F(S) from spec/10-bilateral.md to cover all
   three boundaries, not just S↔P.

### 13.2 What the Property Vocabulary Enables

The 109-property closed ontology (PROPERTY_VOCABULARY.md) provides the shared language
for all three convergence cycles:

- **I↔S**: Properties extracted during harvest. Each intent claim maps to typed
  properties that constrain what spec elements it can produce.
- **S↔P**: Properties checked during bilateral verification. Each spec element has
  properties that implementation must satisfy.
- **I↔P**: Properties checked during direct verification. Each intent claim has
  properties that implementation behavior should exhibit.

The property vocabulary is the **type system of the convergence cycle** — it ensures
that morphisms between states are well-typed.

### 13.3 Relationship to the Coherence Engine

The coherence engine (HARVEST_SEED.md) is the **runtime** of the trilateral model.
It executes the convergence cycles, maintains the divergence metrics, and drives the
system toward equilibrium. The 10 techniques from LP, ATP, and CL
(QUERY_ENGINE_ENHANCEMENTS.md) are the **query language** of the convergence cycles —
they provide the computational machinery for detecting contradictions, measuring
divergence, and verifying coherence.

### 13.4 The Self-Bootstrap Implication

The trilateral model is itself a specification. It should be managed by the system it
describes. When Braid exists, this document becomes data in the datom store — each
definition, theorem, and open question becomes a queryable entity. The system can then
verify its own convergence properties, detect contradictions in its own formalization,
and drive its own specification toward coherence.

This is C7 (self-bootstrap) applied to the methodology itself: the methodology specifies
how to verify coherence, and the methodology's own coherence is verified by the same
mechanisms it specifies.

---

## 14. Topology-Mediated Conflict Resolution

<a name="14-topology"></a>

The convergence cycle's Step 4 (RESOLVE) is where conflicts are **surfaced and bubbled up
through the topology of humans and agents**. This section formalizes the conflict resolution
topology and its relationship to the trilateral model.

### 14.1 The Resolution Topology

Conflicts detected during bilateral assessment (Step 3) require resolution. The resolution
topology determines WHO resolves each conflict and THROUGH what mechanism.

From the design transcripts (transcript 01: spectral authority; transcript 03: dual-process;
transcript 04: delegation classes) and formal spec (CR-002, UA-003, UA-005):

```
Resolution topology T = (Agents, Authority, Routes, Escalation)

where:
  Agents     : Set<Agent>                     -- participants
  Authority  : Agent × Attribute → [0,1]      -- spectral authority score
  Routes     : ConflictType → ResolutionPath   -- routing function
  Escalation : ResolutionPath → ResolutionPath  -- escalation chain

Resolution paths (from CR-002):
  Tier 1: Automated lattice resolution
    Condition: attribute has declared lattice with ⊔ (join)
    Mechanism: compute least upper bound
    No human involvement. Monotonic. Always succeeds.

  Tier 2: Structured deliberation
    Condition: semantic or pragmatic conflict
    Mechanism: Deliberation entity created, positions argued, decision recorded
    May involve multiple agents. Produces ADR.

  Tier 3: Escalated human decision
    Condition: axiological conflict (values/goals misalignment)
    Mechanism: Human reviews positions, makes authoritative decision
    Produces high-authority ADR.
```

### 14.2 Authority as Spectral Decomposition

Authority is NOT a binary permission — it is a **continuous score** computed from the
agent-attribute interaction matrix (UA-003, transcript 01):

```
Given interaction matrix M ∈ ℝ^{|Agents| × |Attributes|}
  where M[α, a] = count of agent α's successful transactions on attribute a

Authority scores = singular values of M via SVD:
  M = U Σ V^T

Agent α's authority on attribute a:
  auth(α, a) = Σ_k σ_k × U[α,k] × V[a,k]
```

This spectral decomposition means authority emerges from **demonstrated competence**,
not from role assignment. An agent that has successfully modified many invariants has
high authority on invariant-related attributes; an agent that has primarily worked on
implementation has high authority on code-related attributes.

### 14.3 Delegation Classes and the Triangle

The four delegation classes (UA-005) map to the trilateral model:

| Class | Typical State | Resolution Authority | Confirmation Required |
|-------|--------------|---------------------|----------------------|
| Autonomous | S↔P (routine) | Self-resolve | None |
| Supervised | I↔S (harvest) | Self-resolve with recording | Post-hoc review |
| Collaborative | Cross-boundary | Multi-agent deliberation | Consensus |
| Advisory | I→S (novel) | Human decision | Explicit approval |

The delegation class is determined by:
1. The **boundary** being crossed (I↔S, S↔P, I↔P)
2. The **commitment weight** of the change (how much depends on it)
3. The **spectral authority** of the agent on the affected attributes
4. The **confidence** of the divergence detection

High authority + low weight + high confidence → autonomous.
Low authority + high weight + low confidence → advisory.

### 14.4 Topologies at Scale

The resolution topology is NOT fixed — it adapts to the number of participants
and the structure of the conflict (transcript 01: topology-agnostic protocol;
PD-005: topology independence):

```
Solo agent:     Self-review (all conflicts resolve by agent judgment)
Two agents:     Bilateral peer review (each reviews the other's harvests)
Small team:     Hierarchical (specialist delegation based on authority)
Large team:     Market topology (broadcast conflicts, bid to resolve)
Mixed human/AI: Hybrid (AI resolves Tier 1-2, humans resolve Tier 3)
```

The key property: **the convergence cycle is topology-agnostic**. Steps 1-3 and 5
are identical regardless of topology. Only Step 4 (RESOLVE) changes. This means
the formal convergence guarantees (Lyapunov stability, monotonic convergence) hold
across all topologies — the resolution mechanism changes but the convergence property
does not.

---

## 15. The Signal-to-Noise Problem in Intent

<a name="15-signal-noise"></a>

The unification thesis (UNIFICATION_THESIS_PROMPT.md) flags an important subtlety:

> "This particular example is slightly leaky, since non-substantive coordination,
> i.e. procedural instructions, still append to the JSONL conversation log, without
> contributing to divergence."

This observation has formal implications for the harvest functor F_IS.

### 15.1 Substantive vs. Procedural Events

Not all events in the log E* contribute to intent divergence. Events decompose into:

```
E = E_sub ⊔ E_proc ⊔ E_meta

where:
  E_sub   — substantive events (new requirements, design decisions, bug reports)
  E_proc  — procedural events (tool coordination, file reads, status checks)
  E_meta  — meta-events (compaction summaries, harvest records, seed injections)
```

The harvest functor should operate primarily on E_sub:

```
F_IS(log) ≈ F_IS(π_sub(log))

where π_sub : E* → E_sub* is the substantive projection
```

This projection is itself non-trivial — distinguishing substantive from procedural
content requires understanding the conversation's semantic structure. An event like
"read file X" is procedural, but "after reading file X, I realize we need invariant Y"
is substantive. The boundary is context-dependent.

### 15.2 Implications for Divergence Measurement

If D(I, S) is computed over the full log E*, it will be inflated by procedural events
that don't represent genuine intent divergence. The corrected divergence measure:

```
D_corrected(I, S) = D(π_sub(I), S)
```

This is equivalent to saying: the divergence between intent and spec is measured only
over the **substantive content** of the conversation, not over the coordination overhead.

The practical implementation: the harvest pipeline's DETECT step (spec/05-harvest.md §5.2)
already filters for "observations made but not transacted" and "decisions made but not
recorded." This filtering IS the substantive projection π_sub. The trilateral model
makes the role of this filtering explicit: it is the kernel of the harvest functor.

---

## 16. Structural Inevitability: Why the System "Can't Help But" Converge

<a name="16-inevitability"></a>

The unification thesis asks for a system that "can't help but bridge the gap." This is
not rhetoric — it is a formal property. The trilateral model achieves structural
inevitability through three mechanisms.

### 16.1 The Ratchet Property

Each convergence cycle is a **ratchet**: it can decrease divergence or hold it constant,
but it cannot increase it (Lyapunov condition, §7). This means:

```
∀ n: Φ(n+1) ≤ Φ(n)

where Φ(n) = total divergence after n convergence cycles
```

Once a divergence is resolved, it stays resolved (modulo new work that creates new
divergence). The system never "unlearns" coherence.

### 16.2 The Pressure Mechanism

Divergence creates **pressure** that triggers convergence:

```
For I↔S: Proactive harvest warnings at 70%/85%/95% context consumption (IB-012)
         → the agent cannot ignore divergence indefinitely
For S↔P: Fitness function F(S) monotonically reported
         → declining fitness is visible and actionable
For I↔P: Direct coding without spec mediation emits guidance signals (GU-005)
         → the agent is continuously reminded to route through S
```

The pressure mechanisms ensure convergence cycles are **triggered**, not merely available.
The system doesn't wait for someone to decide to converge — it detects divergence and
initiates convergence proactively.

### 16.3 The Structural Argument

The "can't help but" property follows from the conjunction of:

1. **Append-only store** (C1): Every fact asserted is permanent. Once an agent harvests
   knowledge into the store, it cannot be lost. Once a bilateral scan detects divergence,
   the detection is itself a permanent fact.

2. **Monotonic convergence** (INV-BILATERAL-001): Each cycle reduces or maintains Φ.
   Combined with C1, this means the system's coherence state is non-volatile and
   non-decreasing.

3. **Proactive triggers**: The system detects divergence and initiates convergence without
   requiring human discipline. This eliminates the "process obligation decay" failure mode
   that SEED.md §2 identifies as the fundamental weakness of traditional approaches.

4. **G-Set merge** (C4): Multi-agent convergence is conflict-free at the data level.
   Conflicts surface at the semantic level and are resolved through deliberation. The
   merge operation itself never fails, never loses data, never creates inconsistency.

5. **Self-improvement** (SEED.md §7): The convergence mechanisms themselves improve with
   use. Dynamic CLAUDE.md adapts to observed drift patterns. Significance weights sharpen
   retrieval. Harvest quality calibrates over time.

**Together**: The system accumulates coherence monotonically (1+2), is driven toward
convergence proactively (3), handles multi-agent composition cleanly (4), and improves
its own convergence efficiency over time (5). A system with these five properties
converges toward coherence by construction — not because participants choose to
converge, but because the substrate makes divergence visible, convergence cheap, and
regression impossible.

This is what it means for coherence to be a **structural property** rather than a
**process obligation**. Process obligations require discipline and decay under pressure.
Structural properties hold by construction and cannot be circumvented without breaking
the substrate.

### 16.4 Connection to the Agent System Model

The agentic systems formal analysis (AGENTIC_SYSTEMS_FORMAL_ANALYSIS.md) established
that every agent system is an instance of:

```
AgentSystem = (E, Op, Obs, S, π, δ, φ, compact)
```

The trilateral model maps onto this as follows:

```
E    = the event type (covering all three states)
Op   = operations across all three states (harvest, implement, verify, seed, etc.)
Obs  = observations from all three boundaries
S    = the runtime state (encompassing I, S, P as substates)
π    = the agent policy (which convergence cycle to run next)
δ    = the state transition function (the convergence cycle itself)
φ    = the context projection (seed assembly from store)
compact = the compaction function (harvest as quotient on intent)
```

The harvest/seed cycle IS the compact/φ pair from the agent system model, lifted from
a single-agent property to a multi-state convergence mechanism. The trilateral model
reveals that the agent system model's compact function has THREE instances (one per
boundary), each with different loss characteristics.

---

## 17. The Unification: Three Models, One Substrate

<a name="17-unification"></a>

This section synthesizes the three formalisms — the agent system model (AGENTIC_SYSTEMS),
the coherence engine (HARVEST_SEED), and the trilateral model (this document) — into a
single unified picture.

### 17.1 The Three Layers

```
Layer 3: Trilateral Model (this document)
  What it describes: The macro-level dynamics of coherence across three states
  Key concept: Bilateral adjunctions with convergence cycles
  Operating timescale: Session-boundary to project-lifetime

Layer 2: Coherence Engine (HARVEST_SEED.md, QUERY_ENGINE_ENHANCEMENTS.md)
  What it describes: The query and detection machinery that powers convergence
  Key concept: Property vocabulary, contradiction detection, typed coherence checking
  Operating timescale: Per-query to per-transaction

Layer 1: Agent System Model (AGENTIC_SYSTEMS_FORMAL_ANALYSIS.md)
  What it describes: The fundamental structure of any agent interacting with a runtime
  Key concept: Free monoid event log, Mealy machine runtime, policy function
  Operating timescale: Per-turn (single operation)
```

### 17.2 How They Compose

```
An agent turn (Layer 1):
  Agent reads log → chooses operation → runtime executes → observation appended

A coherence check (Layer 2):
  Query engine evaluates properties of current state
  → detects tensions, contradictions, divergence
  → reports results as datoms in the store

A convergence cycle (Layer 3):
  Lift changes across a boundary → convert provisionally → assess bilaterally
  → resolve conflicts through topology → apply to ground truth
```

Each layer provides the substrate for the one above:
- Layer 1 enables individual operations that Layer 2's queries are built from
- Layer 2's detection machinery powers Layer 3's bilateral assessment
- Layer 3's convergence guarantees ensure the system-level property that
  Layer 1's individual operations compose into coherent behavior

### 17.3 The Unified Invariant

Across all three layers, one invariant holds:

```
UNIFIED INVARIANT: At every timescale, the system's coherence state is
monotonically non-decreasing.

Layer 1: Each operation appends to the log (monotonic growth of E*)
Layer 2: Each coherence check adds detection results (monotonic growth of knowledge)
Layer 3: Each convergence cycle reduces or maintains Φ (monotonic convergence)

The conjunction: the system knows more, detects more, and converges more
with every action taken. It cannot regress because:
  - C1 prevents deletion (append-only store)
  - The Lyapunov condition prevents divergence increase (per cycle)
  - Spectral authority accumulates competence (non-decreasing)
  - Dynamic CLAUDE.md accumulates drift corrections (self-improving)
```

This is the formal expression of the original vision from SEED.md §1:

> "It is true not because of human discipline or process compliance, but because
> of the structure of the system itself."

The trilateral model, the coherence engine, and the agent system model are three
views of the same structural property: **monotonic convergence toward coherence
as an algebraic invariant of the substrate itself.**

---

## 18. Critical Assessment: Strengths, Weaknesses, and the Path to Adoption

<a name="18-critical-assessment"></a>

This section is a candid evaluation of the trilateral model — what is genuinely strong,
what is concerning, and what concrete changes would make it practically useful rather
than merely theoretically elegant. Written after the formal treatment specifically to
provide "fresh eyes" evaluation material for future sessions.

### 18.1 What Is Genuinely Strong

**The core insight is correct and important.** Every nontrivial software project has
these three states, they DO drift apart, and the drift IS the primary source of waste.
The formalization of *why* it happens and *what types of drift exist* (the reconciliation
taxonomy's 8 divergence types) is a real contribution. Most teams can't even NAME the
type of drift they're experiencing, let alone systematically address it.

**S as universal mediator is the strongest claim.** The Mediation Theorem (§8) — that
every coherent I→P path factors through S — is essentially the formal argument for *why
specifications exist at all*. Without S, verification is subjective ("does this code feel
like what I wanted?"). With S, verification is mechanical ("does this code satisfy these
invariants?"). This justification holds for projects above a certain complexity threshold
and is the single most important result in this document.

**The harvest/seed cycle is exceptionally well-designed for AI agents.** AI agents have
zero durable memory, enormous output volume, and a systematic tendency to bypass
specifications. The harvest/seed cycle directly addresses the LLM context window problem:
conversations die but knowledge persists. This is a real solution to a real problem that
every multi-session AI workflow encounters.

**The energy metaphor is the most intuitive part.** Work creates divergence (potential),
convergence absorbs it (kinetic → equilibrium). This gives a mental model for *when* to
converge: when potential energy is high enough that continuing to work costs more than
pausing to reconcile. The proactive harvest warnings (70%/85%/95% context) implement
this principle well.

### 18.2 What Is Concerning

**The formalism may obscure the practical message.** Adjunctions, Lyapunov functions,
profunctors, enriched categories — intellectually satisfying but are they doing real
work? The core idea is simple: "keep your conversations, specs, and code in sync, and
here are automated tools to help." The category theory adds precision but may be
**over-specifying the conceptual framework** while the implementation framework remains
underspecified. A working prototype with rough edges would be more valuable than a
perfect algebraic treatment of a system that doesn't exist yet.

**The I↔S boundary is the weakest link and the hardest to automate.** Harvesting intent
from conversations into formal spec elements requires *creative judgment*. It's not like
S↔P (where you can mechanically check "does this code match this invariant?"). The I→S
morphism requires understanding context, distinguishing substantive from procedural
content, deciding what level of formality is appropriate, and making judgment calls about
what's spec-worthy. Current LLMs can do this, but not reliably. This boundary is where
the system will succeed or fail in practice, and it is the least developed part of the
design.

**The overhead concern is real.** Most software teams already struggle to maintain
documentation, let alone a formal specification with individually-addressed elements,
falsification conditions, cross-references, and contradiction detection. The trilateral
model asks people to maintain THREE synchronized artifacts instead of the usual two
(code + maybe some docs). The "can't help but converge" claim only holds if the tools
make convergence **cheaper than not converging**. That is a high bar.

**The direct I→P path is how 99% of software is actually written.** The model treats
F_IP as an inferior morphism that DDIS should redirect through S. But for small changes,
bug fixes, quick experiments, and prototyping, going directly from intent to code is
the right thing to do. The overhead of formalizing every change as a spec element before
implementing it would be crippling for small-to-medium work. The system needs a principled
answer to "when is the spec overhead worth it?" and the current design doesn't have one
beyond "always route through S," which is impractical.

**The Lyapunov argument has a gap.** It proves that convergence cycles don't increase
divergence, but it doesn't prove that convergence cycles *happen*. If the system relies
on agents choosing to run harvest/seed cycles, the convergence property is only as strong
as the agents' compliance — which is exactly the "process obligation" failure mode that
SEED.md §2 critiques. The proactive harvest warnings help, but they are still
notifications that can be ignored. True structural inevitability requires convergence to
be a *side effect* of normal work, not a separate action.

### 18.3 Six Design Improvements

These are concrete changes that would close the gap between the formal model and a tool
that people actually use.

#### DI-1: Make Convergence Invisible

The harvest should happen automatically as a side effect of every tool call, not as a
separate "run harvest" step. Every time the agent reads a file, writes code, or runs a
test, the system should silently update its divergence measurements. Every time a session
ends (for any reason — context limit, user disconnect, agent crash), the harvest should
fire automatically. The user/agent should never have to think about convergence. It
should be like garbage collection: it happens, you benefit, you don't manage it.

**Implementation implication**: The harvest pipeline (spec/05-harvest.md) should be
embedded in the CLI's tool-response path, not exposed as a separate command. `braid
harvest` should exist for manual invocation but the default should be continuous
background harvesting triggered by observable thresholds.

#### DI-2: Introduce a Formality Gradient

Not every change needs a formal invariant with a falsification condition. The system
should support a spectrum:

```
Level 0: Implicit     — no spec backing (small fixes, experiments)
Level 1: Noted        — one-line intent annotation in a commit message
Level 2: Structured   — spec element with ID, type, and statement
Level 3: Formal       — full invariant with falsification condition, verification, traces
Level 4: Verified     — formal + witnessed + challenged + bilateral-checked
```

The system should automatically escalate formality based on **commitment weight** — how
much downstream work depends on this decision. A one-line CSS fix stays at Level 0. A
new auth architecture gets escalated to Level 3-4. The current model treats all spec
elements as Level 3+, which creates friction that prevents adoption.

**Implementation implication**: The property vocabulary should support lightweight
annotations (Level 1-2) alongside full spec elements (Level 3-4). The coherence
engine should track divergence at all levels, with different sensitivity thresholds.

#### DI-3: Visible Divergence Dashboard

The single most compelling feature would be a concrete display:

> "Here are the 7 places where your code doesn't match your spec, and here are the
> 3 decisions from yesterday's conversation that nobody wrote down."

Make divergence **visible** and **specific**. People fix drift when they can see it.
Abstract fitness scores (F(S) = 0.73) are meaningless to most users. Concrete, actionable
divergence reports are immediately valuable.

**Implementation implication**: The bilateral loop's output should be a structured
report with file paths, line numbers, specific elements, and suggested actions — not
a numeric score. The TUI (spec/14-interface.md) should make this the default view.

#### DI-4: Start with S↔P, Add I↔S Later

The bilateral loop between spec and implementation is the most mature, most automatable,
and most immediately valuable part of the design. Build this first, prove it works, then
add the harvest/seed cycle (I↔S) once the core is solid. The I↔P morphism can be
deferred indefinitely — it's the weakest and least useful of the three.

**Implementation implication**: Stage 0 should focus on: (1) datom store, (2) Datalog
query engine, (3) bilateral spec↔impl checking, (4) divergence reporting. Harvest/seed
can be Stage 0.5 or Stage 1.

#### DI-5: Spec Generation from Code (Inverse Harvest)

The backward morphism B_PS is currently a **checker**: "does the code match the spec?"
The most powerful practical feature would be a **generator**: "here is your codebase,
here is the specification the system infers from it."

This inverts the adoption flow. Instead of "write specs then implement," you implement
and the system extracts specs. This dramatically lowers the adoption barrier because you
don't need a spec to start using the tool. The generated spec is a draft that humans
refine — much easier than writing from scratch.

**Implementation implication**: B_PS should have two modes: `verify` (check code against
existing spec) and `infer` (generate spec elements from code). The inferred spec
elements start at Level 2 (structured) and humans escalate to Level 3-4 as needed.

#### DI-6: Natural Language Spec Format

The biggest adoption barrier is the spec format. If writing a spec element requires
learning a syntax (`INV-NS-NNN`, falsification conditions, verification tags,
cross-references), 90% of potential users are lost. The spec should read like
**structured English with machine-parseable annotations**. The property vocabulary (109
typed properties) helps here because it's a fixed set of labels, not a language to learn.

**Implementation implication**: The CLI should accept natural-language invariant
descriptions and automatically assign IDs, extract falsification conditions, and propose
cross-references. The formal structure is generated, not authored.

### 18.4 What a Practical Implementation Looks Like (Layman's Terms)

Three things exist in every software project:

- Your **conversations** (Slack, pair programming, Claude Code sessions) — where you
  decide what to build
- Your **rules** (the spec) — the written-down agreements about how the system works
- Your **code** — what's actually been built

**The problem**: These three constantly fall out of sync. You decide something in a
conversation but forget to write it down. You write a rule but nobody implements it.
Someone implements something that contradicts a rule. Someone implements something
nobody ever asked for.

**What this system does**: It watches all three continuously and tells you **exactly
where they disagree and why**. Not "your project has issues" — specifically:

> "The auth timeout was changed to 30 seconds in conversation #47 but the spec
> still says 60 seconds and the code uses 45 seconds."

It also **helps you fix it**:
- Scans your conversations and proposes rule updates ("you decided X — should I
  add it to the spec?")
- Checks your code against the rules automatically after every change ("this
  function violates rule AUTH-003")
- Flags when you're coding without rule backing ("this new module has no rules —
  want me to draft some based on what you built?")

**The "can't help but" part**: The tools are woven into the normal workflow so that
staying in sync is *easier* than drifting apart. Like how git makes tracking changes
easier than not tracking them. You don't "do version control" as a separate activity —
it's just part of how you work. This system aims for the same: convergence isn't a
process you follow, it's a property of the tools you're already using.

**For AI agents specifically**: Every time a coding session ends, the system
automatically extracts important decisions and discoveries into the spec. When a new
session starts, it automatically loads the relevant spec into the agent's context. The
agent never loses knowledge, the human never re-explains, and the spec accumulates the
team's understanding over time — regardless of how many agents or sessions are involved.

### 18.5 The Bottom Line

The trilateral model is a genuinely strong formalization of a real problem. The algebraic
treatment is rigorous. But the gap between the theory and a practical tool that people
actually use is significant.

**The path to adoption runs through**: invisible convergence (DI-1), visible divergence
(DI-3), lightweight specs (DI-2, DI-6), spec generation from code (DI-5), and immediate
value (DI-4) — not through adjunctions and Lyapunov functions.

**The formalism should be the hidden engine, not the user interface.** Category theory
should power the convergence guarantees the way linear algebra powers Google Search —
essential to correctness, invisible to users. What users see is: "here's where your
project is inconsistent, here's what to fix, and the fix was applied automatically
where possible."

**The key risk**: Over-engineering the theory while under-engineering the experience.
The 1,600 lines of formalization above are valuable as a foundation, but they're
worthless without a 100-line CLI that makes a developer say "I can't believe I ever
worked without this."

---

## 19. The Radical Addition: Unified Store, Three LIVE Views

<a name="19-live-views"></a>

> **Status**: Proposal — the single highest-impact addition to the trilateral model
> **Traces to**: spec/01-store.md §1.3 (LIVE Index), §7 (Lyapunov), §6 (Universal
> Convergence Cycle), DI-1 (Invisible Convergence)
> **Key insight**: Eliminate the boundary between I, S, and P by making them LIVE
> materialized views of a single datom store, not separate artifact stores that
> require periodic synchronization.

### 19.1 The Problem with "Three Stores, Periodic Sync"

The trilateral model as formalized in §1–§18 treats Intent, Specification, and
Implementation as three distinct representations with their own ground truths
(JSONL logs, spec files, code files) connected by six functors and synchronized
through periodic convergence cycles. This architecture has a structural weakness:

**The convergence cycle is a batch operation.** You work, divergence accumulates,
then at some threshold you stop working and run the 5-step cycle (Lift → Convert →
Assess → Resolve → Apply). Between cycles, you are flying blind — divergence grows
unobserved, and the cost of reconciliation grows superlinearly with the divergence
that has accumulated.

This is the same architectural mistake that led to "integration hell" in pre-CI
software development, to data warehouses that are always stale, and to every
workflow where "sync" is a verb that means "stop what you're doing and reconcile."

The LIVE index from `spec/01-store.md` already solves this exact problem for the
store/query boundary:

```
LIVE(S) = fold(causal_sort(S), apply_resolution)

Store layer:  append-only datoms (the "truth")
LIVE layer:   continuously materialized current-state view (the "useful truth")

The LIVE index is not checked periodically — it is updated incrementally with
every transaction. Current state is always available at O(1).
```

### 19.2 The Proposal: Three LIVE Projections of One Store

Extend the LIVE index concept to the entire trilateral model. Instead of three
separate stores with periodic sync, there is **one datom store** with **three
LIVE materialized views** — one for each vertex of the triangle.

```
                        ┌─────────────────────┐
                        │   Unified Datom      │
                        │       Store          │
                        │   (P(D), ∪)          │
                        └──────┬──────┬────────┘
                               │      │
              ┌────────────────┤      ├────────────────┐
              │                │      │                │
              ▼                ▼      ▼                ▼
     ┌────────────────┐  ┌──────────────┐  ┌────────────────┐
     │  LIVE_I(S)     │  │  LIVE_S(S)   │  │  LIVE_P(S)     │
     │  Intent View   │  │  Spec View   │  │  Impl View     │
     │                │  │              │  │                │
     │ - Decisions    │  │ - Invariants │  │ - Modules      │
     │ - Constraints  │  │ - ADRs       │  │ - Functions    │
     │ - Goals        │  │ - Neg cases  │  │ - Tests        │
     │ - Preferences  │  │ - Uncertainty│  │ - Coverage     │
     └────────────────┘  └──────────────┘  └────────────────┘

Each view is a monotone function from the store semilattice:

LIVE_I(S) = project(S, {a | a.namespace ∈ INTENT_ATTRS})
LIVE_S(S) = project(S, {a | a.namespace ∈ SPEC_ATTRS})
LIVE_P(S) = project(S, {a | a.namespace ∈ IMPL_ATTRS})
```

Every input — a conversation message, a spec element, a code assertion — is a
datom transacted into the same store. The views update incrementally, automatically,
continuously.

**What changes**:

| Before (§1–§18)                       | After (Unified Store)                   |
|---------------------------------------|-----------------------------------------|
| Three ground-truth representations    | One ground-truth store                  |
| Six functors mapping between them     | Three projection functions from store   |
| Periodic 5-step convergence cycle     | Continuous incremental updates          |
| Divergence accumulates unobserved     | Divergence is a live metric             |
| Harvest is a batch extraction         | Harvest is transact + recompute views   |
| Seed is a batch injection             | Seed is a parameterized query           |
| "Sync" is a verb (action you take)    | Coherence is a property (always true)   |

### 19.3 How Inputs Become Datoms

The critical innovation is that **every input channel feeds the same store**:

```
CONVERSATION INPUT (agent session):
  User says: "Authentication should use JWT, not sessions"
  ──►  Datom: (intent:auth-method, :intent/decision, "JWT over sessions",
               tx-47, Assert)
  ──►  Datom: (intent:auth-method, :intent/rationale, "Stateless, scales
               horizontally", tx-47, Assert)
  ──►  Datom: (intent:auth-method, :intent/source, "session:2026-03-03/msg-12",
               tx-47, Assert)

SPEC INPUT (spec edit):
  Agent writes: INV-AUTH-001: All endpoints require valid JWT
  ──►  Datom: (spec:inv-auth-001, :spec/type, :invariant, tx-48, Assert)
  ──►  Datom: (spec:inv-auth-001, :spec/statement, "All endpoints require
               valid JWT", tx-48, Assert)
  ──►  Datom: (spec:inv-auth-001, :spec/traces-to, intent:auth-method,
               tx-48, Assert)

CODE INPUT (implementation):
  Agent writes: fn validate_jwt(token: &str) -> Result<Claims>
  ──►  Datom: (impl:auth/validate_jwt, :impl/signature, "fn validate_jwt(&str)
               -> Result<Claims>", tx-49, Assert)
  ──►  Datom: (impl:auth/validate_jwt, :impl/implements, spec:inv-auth-001,
               tx-49, Assert)
  ──►  Datom: (impl:auth/validate_jwt, :impl/file, "src/auth.rs:42", tx-49,
               Assert)
```

Each transaction carries full provenance. The `traces-to` and `implements` links
create the trilateral traceability structure **as a natural byproduct of doing
work**, not as a separate compliance activity.

### 19.4 Divergence as a Live Metric

With all three states in one store, divergence is computable in real time:

```
D_IS(S) = |{e ∈ LIVE_I(S) | ¬∃ link ∈ S: link.a = :traces-to ∧
            link.e ∈ LIVE_S(S) ∧ link.v = e}|
         + |{e ∈ LIVE_S(S) | ¬∃ link ∈ S: link.a = :traces-to ∧
            link.v = e ∧ link.e ∈ LIVE_I(S)}|

D_SP(S) = |{e ∈ LIVE_S(S) | ¬∃ link ∈ S: link.a = :implements ∧
            link.v = e ∧ link.e ∈ LIVE_P(S)}|
         + |{e ∈ LIVE_P(S) | ¬∃ link ∈ S: link.a = :implements ∧
            link.e = e ∧ link.v ∈ LIVE_S(S)}|

D_IP(S) = D_IS(S) + D_SP(S) - |{e | e has complete I→S→P chain}|

Φ(S) = w₁·D_IS(S) + w₂·D_SP(S) + w₃·D_IP(S)
```

Because `D_IS`, `D_SP`, `D_IP` are computable from the store at any instant,
**Φ is a live counter, not a periodic measurement**. Every transaction that adds
a `traces-to` or `implements` link decreases Φ. Every transaction that adds an
unlinked intent decision or unlinked code function increases Φ.

The LIVE divergence metric can be:
- Displayed in the IDE as a status bar number
- Injected into the agent's context window as guidance
- Used to trigger convergence actions automatically (e.g., "Φ exceeded threshold,
  running I→S extraction")
- Tracked over time to show convergence trends

### 19.5 What This Does to the Convergence Cycle

The 5-step convergence cycle (§6) does **not disappear** — it becomes the
implementation strategy for reducing Φ when the live metric indicates drift.
But the critical difference is:

```
BEFORE:  Work → accumulate unknown divergence → notice → stop → run cycle → resume
AFTER:   Work → see Φ update in real time → cycle triggers when ΔΦ > threshold
```

The cycle itself also changes character:

**Step 1 (Lift)**: No longer "scan everything since last convergence." Instead,
query the LIVE views for unlinked datoms since the last convergence transaction:

```datalog
[:find ?intent ?statement
 :where
 [?intent :intent/decision ?statement]
 (not [_ :traces-to ?intent])]
```

This is O(unlinked datoms), not O(all datoms since last sync).

**Step 2 (Convert)**: Generate candidate links (proposed `traces-to` or
`implements` datoms) using the same store's own content as context. The conversion
functor has access to the full history via the store.

**Step 3 (Assess)**: Bilateral coherence check is a Datalog query over the store:
does the proposed link create a contradiction with existing invariants? Does the
linked spec element already have a different implementation?

**Step 4 (Resolve)**: Conflicts are datoms too — assert them as `:conflict/type`,
`:conflict/between`, `:conflict/proposed-resolution` and route through the
topology-mediated resolution from §14.

**Step 5 (Apply)**: Transact the resolved links. Φ decreases. LIVE views update.

The entire cycle is expressible as a Datalog program over the store, which means
it is:
- **Monotone** (adding links never removes existing valid links)
- **Convergent** (running the cycle is idempotent when there's nothing to link)
- **Incrementalizable** (each step operates on the delta, not the full store)

### 19.6 Why This Is the Single Smartest Addition

**It dissolves the meta-problem.** The trilateral model as presented in §1–§18
adds a layer of complexity: six functors, three adjunctions, periodic cycles. The
unified-store version removes the functors entirely — they become projections from
a single source. There are no "mappings between representations" because there is
only one representation. The adjunctions become an implementation detail of how the
projection functions compose, not a user-facing concept.

**It makes DI-1 (Invisible Convergence) structurally inevitable.** If every input
is a datom and every output is a LIVE view, convergence isn't an activity you
perform — it's a property of the data model. You can't *not* converge, because
there's nothing to sync. The only question is whether the links between datoms
exist yet.

**It aligns with what already works.** The LIVE index is already specified and
designed (INV-STORE-012). Extending it to cross-boundary views is a natural
generalization, not a new concept. The self-bootstrap commitment (C7) already
requires spec elements to live in the store. This proposal simply extends that
commitment to intent and implementation facts as well.

**It enables the "formality gradient" (DI-2) naturally.** A Level 0 assertion
(implicit) is a datom with no links. A Level 4 assertion (verified) is the same
datom with a full chain of `traces-to`, `implements`, `witnessed-by`, and
`challenged-by` links. The formality level isn't a property you set — it's
computed from the link structure in real time:

```
formality_level(e, S) =
  0  if  e has no outgoing links
  1  if  e has :intent/noted
  2  if  e has :spec/id ∧ :spec/type ∧ :spec/statement
  3  if  e has L2 + :spec/falsification ∧ :spec/traces-to
  4  if  e has L3 + :spec/witnessed ∧ :spec/challenged
```

**It turns the overhead problem (§18.2) inside out.** The overhead in §1–§18 comes
from having three separate representations that need active synchronization. In
the unified-store model, the overhead disappears because there's nothing to
synchronize. The *only* cost is transacting datoms — which the system already
does for every operation anyway. Adding a `:traces-to` link to a transaction
costs essentially nothing on top of the transaction itself.

**It gives you something no other system has: a queryable, time-traveling,
cross-boundary knowledge graph.** Because all three states live in the same
append-only store with full provenance, you can ask questions that no existing
tool can answer:

```
"When did this intent decision first appear, when was it formalized into a spec
element, and when was it first implemented?"

"Which code modules have no spec backing? Which spec elements have no intent
backing? Which intent decisions were never formalized?"

"Show me the state of all three views as of last Tuesday."

"What is the convergence trend over the last 20 sessions?"
```

These are not hypothetical queries — they are Datalog programs over the existing
store design.

### 19.7 The Risk and the Mitigation

**The risk**: Requiring every input to be "datomized" adds friction. A developer
doesn't want to annotate every conversation message or tag every function with its
spec link.

**The mitigation**: Most datomization can be automated:

- **Conversation → datoms**: An LLM extracts decisions, constraints, and goals
  from conversation transcript. This is the harvest functor (§5), but running
  incrementally after each message turn rather than as a batch at session end.
  The human sees: nothing (it happens in the background). The LIVE_I view
  simply... updates.

- **Spec → datoms**: The spec is already structured (INV/ADR/NEG elements with
  IDs). Parsing it into datoms is mechanical. The existing `ddis parse` command
  does exactly this. The extension: run it incrementally on every spec file save.

- **Code → datoms**: Static analysis extracts module structure, function
  signatures, test coverage. `implements` links can be inferred from annotations
  (existing `// ddis:maintains INV-001` annotations) or from naming conventions
  or from LLM-assisted inference. The developer sees: nothing (a file watcher
  extracts code facts on save).

The formality gradient (DI-2) handles the rest: not everything needs full links.
Most code datoms start at Level 0 (no links) and only get linked when they become
important enough to warrant it. The live Φ metric tells you which unlinked datoms
are worth investing in.

### 19.8 Formal Relationship to §1–§18

This proposal does not invalidate the trilateral model — it provides its most
natural implementation:

```
§1–§18 formalism          │ §19 implementation
───────────────────────────┼─────────────────────────────────
Cat_I (free monoid E*)     │ LIVE_I(S): project over intent attrs
Cat_S (constrained graph)  │ LIVE_S(S): project over spec attrs
Cat_P (executable forest)  │ LIVE_P(S): project over impl attrs
F_IS (Harvest)             │ Extract datoms from conversation
B_SI (Seed)                │ Query LIVE_I for relevant decisions
F_SP (Implement)           │ Write code datoms with :implements links
B_PS (Verify)              │ Query coverage of LIVE_S by LIVE_P
F_IP (Direct Code)         │ Write code datoms with no spec link
B_PI (Learn)               │ Infer spec elements from code patterns
Φ(I,S,P)                  │ LIVE_Φ(S): count of unlinked datoms
Convergence cycle          │ Datalog program reducing LIVE_Φ(S)
```

The adjunctions become **composites of store operations** rather than abstract
mathematical structures. The Lyapunov argument still holds — Φ still decreases
monotonically under convergence operations — but now Φ is observable in real time
rather than measured periodically.

### 19.9 What This Means in Layman's Terms

Imagine you have a single notebook where you write everything: your ideas, your
plans, and your code. Every entry is timestamped and tagged. The notebook
automatically maintains three "views" — filtered views that show you just the
ideas, just the plans, or just the code. It also maintains a fourth view: the
"connection gaps" — places where an idea exists without a plan, or a plan exists
without code, or code exists without a plan.

That fourth view is a live counter. As you work, you naturally create connections
(your code implements your plan, your plan reflects your idea). The counter goes
down. When you create something new without connecting it, the counter goes up.
The system gently nudges you when the counter gets too high.

You never "sync." You never "reconcile." You never "do process." You just work,
and the system keeps score.

That's the unified store with three LIVE views.

---

*This document extends the bilateral loop formalization from spec/10-bilateral.md to
a full trilateral model covering all three coherence states. The key contributions are:
the universal convergence cycle as a pattern for all adjunction pairs, the Lyapunov
stability argument for convergence, the mediation theorem for why specification is
central, the phase space analysis of divergence dynamics, topology-mediated conflict
resolution, the structural inevitability argument, the three-layer unification,
the critical assessment with six concrete design improvements (DI-1 through DI-6),
and the radical proposal (§19) to dissolve the three-store architecture entirely
in favor of three LIVE materialized views of a single unified datom store — making
coherence a continuously-observed property rather than a periodically-enforced
discipline. Open questions OQ-1 through OQ-5 should be resolved through formal
analysis or empirical observation during Stage 0 implementation.*
