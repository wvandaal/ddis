---
module: element-specifications
domain: core
maintains: [INV-020]
interfaces: [INV-001, INV-002, INV-003, INV-004, INV-005, INV-006, INV-007, INV-008, INV-009, INV-010, INV-017, INV-018, INV-019]
implements: [ADR-008, ADR-009, ADR-010]
negative_specs: 3
---

# Module: Element Specifications

The element-by-element reference for DDIS authors. Each section specifies one structural element: what it must contain, quality criteria, how it relates to other elements, and what good versus bad looks like. Each includes woven LLM-specific provisions (ADR-008: LLM provisions integrated into each element specification).

**Invariants this module maintains (INV-018 compliance):**
- INV-020: Every element specification chapter includes a structured verification prompt block

**Key invariants referenced from other modules (INV-018 compliance):**
- INV-001: Every implementation section traces to at least one ADR or invariant, which traces to the formal model
- INV-002: Every design choice where a reasonable alternative exists is captured in an ADR
- INV-003: Every invariant can be violated by a concrete scenario and detected by a named test
- INV-005: Every performance claim is tied to a specific benchmark, design point, and measurement methodology
- INV-006: The specification contains a cross-reference web where no section is an island
- INV-008: The specification is self-contained
- INV-009: Every domain-specific term used in the specification is defined in the glossary
- INV-010: Every state machine defines all states, all transitions, all guards, and behavior for invalid transitions
- INV-017: Every implementation chapter includes explicit "DO NOT" constraints preventing likely hallucination patterns
- INV-018: Every implementation chapter restates the invariants it must preserve
- INV-019: The spec provides an explicit dependency chain for implementation ordering

---

# PART II: CORE STANDARD — ELEMENT SPECIFICATIONS

The heart of DDIS. Each section specifies one structural element: what it must contain, quality criteria, how it relates to other elements, and what good versus bad looks like. Each includes woven LLM-specific provisions (ADR-008).

## Chapter 2: Preamble Elements

### 2.1 Design Goal

**What it is**: A single sentence (<=30 words) that states the system's reason for existing.

**Required properties**:
- States the core value proposition, not the implementation
- Uses bold for emphasis on the 3-5 key properties
- Readable by a non-technical stakeholder

**Quality criteria**: A reader who sees only the design goal should be able to decide whether this system is relevant to them.

**DO NOT** state the design goal in terms of implementation technology ("Build a Rust-based event-sourced system"). State it in terms of value ("scrollback-native, zero-flicker terminal apps"). An LLM reading an implementation-focused design goal will over-constrain its solution space. (Validates INV-017.)

**Anti-pattern**: "Design goal: Build a distributed task coordination system using event sourcing and advisory reservations." -- This describes implementation, not value.

**Good example** (FrankenTUI): "Design goal: **scrollback-native, zero-flicker, agent-ergonomic, and high-performance** Rust terminal apps."

**Cross-references**: The design goal establishes vocabulary used throughout. Each bolded property should correspond to at least one invariant and one quality gate.

---

### 2.2 Core Promise

**What it is**: A single sentence (<=40 words) that describes what the system makes possible, from the user's perspective.

**Required properties**:
- Written from the user's viewpoint, not the architect's
- States concrete capabilities, not abstract properties
- Uses "without" clauses to highlight what would normally be sacrificed

**Quality criteria**: If you showed only this sentence to a potential user, they should understand what the system gives them and what it doesn't cost them.

**DO NOT** use abstract qualities without concrete meaning ("robust", "scalable", "enterprise-grade"). An LLM encountering these terms will generate generic boilerplate instead of domain-specific implementation. (Validates INV-017.)

**Anti-pattern**: "The system provides robust, scalable, enterprise-grade coordination." -- Meaningless buzzwords.

**Good example** (FrankenTUI): "ftui is designed so you can build a Claude Code / Codex-class agent harness UI without flicker, without cursor corruption, and without sacrificing native scrollback."

---

### 2.3 Document Note

**What it is**: A short disclaimer (2-4 sentences) about code blocks and where correctness lives.

**Why it exists**: Without this note, implementers treat code blocks as copy-paste targets. The document note redirects trust from code to invariants and tests. LLMs will reproduce code blocks verbatim unless explicitly told otherwise.

**DO NOT** omit this note even if it seems obvious. (Validates INV-017.)

**Template**:
> Code blocks in this plan are **design sketches** for API shape, invariants, and responsibilities.
> They are intentionally "close to [language]," but not guaranteed to compile verbatim.
> The correctness contract lives in the invariants, tests, and ADRs — not in pseudo-code syntax.

---

### 2.4 How to Use This Plan

**What it is**: A numbered list (4-6 items) giving practical reading and execution guidance.

**Required properties**:
- Starts with "Read PART 0 end-to-end"
- Identifies the churn-magnets to lock via ADRs
- Points to the Master TODO as the execution tracker
- Identifies at least one non-negotiable process requirement
- For LLM implementers: includes a step about reading negative specifications and verification prompts

**Quality criteria**: A new team member reading only this section knows exactly how to engage with the document.

### Verification Prompt for Chapter 2 (Preamble Elements)

After writing your spec's preamble, verify:
1. [ ] Design goal is <=30 words and states value, not implementation technology (INV-017: every implementation chapter includes explicit "DO NOT" constraints — applied here as the negative spec against implementation-focused design goals)
2. [ ] Core promise uses "without" clauses and contains no abstract buzzwords (INV-017)
3. [ ] Document note explicitly states code blocks are design sketches, not copy-paste targets (INV-008: the spec is self-contained — this note prevents misinterpretation)
4. [ ] How-to-use list starts with "Read PART 0" and includes LLM-specific step for negative specs and verification prompts
5. [ ] Your preamble does NOT use marketing language ("enterprise-grade", "cutting-edge") — these cause LLMs to generate generic boilerplate

---

## Chapter 3: PART 0 Elements

### 3.1 Non-Negotiables (Engineering Contract)

**What it is**: 5-10 properties defining what the system IS. Stronger than invariants (which are formal and testable) — these are philosophical commitments that must never be compromised, even under pressure.

**Required format**:
```
- **[Property name in bold]**
  [One sentence explaining what this means concretely]
```

**Quality criteria for each non-negotiable**:
- An implementer could imagine a situation where violating it would be tempting (e.g., "just skip replay validation in dev mode — it's slow")
- The non-negotiable clearly says: no, even then
- It is not a restatement of a technical invariant; it is a commitment

**DO NOT** restate invariants as non-negotiables — they serve different purposes. Non-negotiables are philosophical commitments ("deterministic replay is non-negotiable"); invariants are testable properties ("same event sequence -> identical state"). (Validates INV-017.)

**Relationship to invariants**: Non-negotiables are the "why" that justifies groups of invariants. "Deterministic replay is real" (non-negotiable) justifies INV-003: "Same event log -> identical state" (invariant). The non-negotiable is the commitment; the invariant is the testable manifestation.

---

### 3.2 Non-Goals

**What it is**: A list of 5-10 things the system explicitly does NOT attempt.

**Why it exists**: Scope creep is the most common spec failure. Non-goals give implementers permission to say "out of scope." For LLMs, non-goals prevent adding "helpful" features not in the spec.

**Quality criteria for each non-goal**:
- Someone has actually asked for this (or will), making the exclusion non-obvious
- The non-goal explains briefly why it's excluded (not just "not in scope" but why not)

**DO NOT** list absurd non-goals that nobody would request. Non-goals should exclude things that are tempting, not impossible. (Validates INV-017.)

**Anti-pattern**: "Non-goal: Building a quantum computer." -- Nobody asked for this. Non-goals should exclude things that are tempting, not absurd.

---

### 3.3 First-Principles Derivation

**What it is**: The formal model from which the entire architecture derives. Makes every section feel *inevitable* rather than *asserted*.

**Required components**:

1. **"What IS a [System]?"** — A mathematical or pseudo-mathematical definition:
   ```
   System: (State, Input) -> (State', Output)
   where:
     State = { ... }
     Input = { ... }
     Output = { ... }
   ```
   This establishes the system as a formally defined state machine or function.

2. **Consequences** — 3-5 bullet points explaining what this formal definition implies for the architecture. Each consequence should feel like a discovery, not an assertion.

3. **Fundamental Operations Table** — Every primitive operation with its mathematical model and complexity target.

**Quality criteria**: After reading this section, an implementer should be able to derive the system's architecture independently. If the architecture is a surprise after reading the first principles, the derivation is incomplete.

**DO NOT** assert the architecture without deriving it from the formal model. An LLM given an asserted architecture will not understand the constraints behind it and will make downstream decisions that violate the model. (Validates INV-001: every implementation section traces to at least one ADR or invariant, which traces to the formal model; INV-017.)

**Relationship to other elements**: The formal model is referenced by every invariant (which constrains states the model can reach), every ADR (which decides between alternatives within the model), and every algorithm (which implements transitions in the model).

---

### 3.4 Invariants

**What it is**: A numbered list of properties that must hold at all times during system operation.

**Required format per invariant**:

```
**INV-NNN: [Descriptive Name]**

*[Plain-language statement in one sentence]*

  [Semi-formal expression: predicate logic, pseudocode, or precise English]

Violation scenario: [Concrete description of how this could break]

Validation: [Named test strategy or specific test]

// WHY THIS MATTERS: [One sentence on consequences of violation]
```

**Quality criteria for each invariant**:
- **Falsifiable**: You can construct a concrete counterexample (a state or event sequence that would violate it)
- **Consequential**: Violating it causes observable, bad behavior (not just theoretical impurity)
- **Non-trivial**: It's not a tautology or a restatement of a type constraint the compiler already enforces
- **Testable**: The validation method is specific enough to implement

**Quantity guidance**: A medium-complexity system typically has 10-25 invariants. Fewer suggests under-specification. More suggests the invariants are too granular (consider grouping related invariants under a non-negotiable).

**DO NOT** write invariants without violation scenarios — an invariant without a counterexample is unfalsifiable and violates INV-003 (every invariant can be violated by a concrete scenario and detected by a named test). **DO NOT** write invariants that merely restate type system guarantees (e.g., "TaskId values are unique" when using a newtype with auto-increment). **DO NOT** write aspirational invariants without measurable criteria. (Validates INV-003, INV-017.)

**Anti-patterns**:
```
X BAD: "INV-001: The system shall be performant."
  - Not falsifiable (what is "performant"?)
  - No violation scenario (everything is or isn't "performant")
  - Not testable

X BAD: "INV-002: TaskId values are unique."
  - Trivially enforced by the type system (use a newtype with a counter)
  - Not worth an invariant unless uniqueness has subtle cross-boundary implications

GOOD: "INV-003: Event Log Determinism
  Same event sequence applied to same initial state produces identical final state.
  forall events, forall state_0: reduce(state_0, events) = reduce(state_0, events)
  Violation: A reducer reads wall-clock time, causing different states on replay.
  Validation: Replay test -- process 10K events, snapshot, replay from scratch, byte-compare.
  // WHY THIS MATTERS: If replay diverges, we lose auditability and debugging via replay."
```

---

### 3.5 Architecture Decision Records (ADRs)

**What it is**: A record of each significant design decision, including the alternatives that were rejected and why.

**Required format per ADR**:

```
### ADR-NNN: [Descriptive Title]

#### Problem
[1-3 sentences describing the decision that needs to be made]

#### Options
A) **[Option name]**
- Pros: [concrete advantages]
- Cons: [concrete disadvantages]

B) **[Option name]**
- Pros: [concrete advantages]
- Cons: [concrete disadvantages]

[At least 2 options, at most 4]

#### Decision
**[Chosen option]**: [Rationale in 2-5 sentences]

// WHY NOT [rejected option]? [Brief explanation]

#### Consequences
[2-4 bullet points on what this decision implies for the rest of the system]

#### Tests
[How we will know this decision was correct or needs revisiting]
```

**Quality criteria for each ADR**:
- **Genuine alternatives**: Each option must have a real advocate. If Option B is a strawman nobody would choose, it is not a genuine alternative. The test: would a competent engineer in a different context reasonably choose Option B?
- **Concrete tradeoffs**: Pros and cons cite specific, measurable properties — not vague qualities like "simpler" or "more robust."
- **Consequential decision**: The choice materially affects the system. If swapping Option A for Option B would require < 1 day of refactoring, it's not an ADR — it's a local implementation choice.

**DO NOT** include decisions that predate the spec's scope (e.g., language choice if already decided). **DO NOT** create strawman ADRs where one option is obviously superior. **DO NOT** omit WHY NOT annotations for rejected options — these are the most valuable part for LLM implementers who might otherwise re-explore rejected paths. (Validates INV-002, INV-017.)

**Anti-pattern**:
```
X BAD:
  ADR-001: Use Rust
  Options: A) Rust B) C++ C) Go
  Decision: Rust because it's safe.
  -- Not a genuine decision within the spec's scope.
    Language choice predates the spec.
    No concrete tradeoff analysis.
```

**Churn-magnets**: After all ADRs are written, add a brief section identifying which decisions cause the most downstream rework if changed. These are the decisions to lock first and spike earliest (see §6.1.1, Phase -1).

---

### 3.6 Quality Gates

**What it is**: 4-8 stop-ship criteria, ordered by priority.

**Required properties per gate**:
- A gate is a **predicate**, not a task. It is either passing or failing at any point in time.
- Each gate references specific invariants or test suites.
- Gates are ordered such that a failing Gate N makes Gate N+1 irrelevant.
- At least one gate specifically validates LLM implementation readiness (see Gate 7 in §0.7).

**Quality criteria**: A project manager should be able to assess gate status in < 30 minutes using the referenced tests.

**DO NOT** define gates without concrete measurement procedures. "Code quality is high" is not a gate. "All invariants have passing tests" is a gate. (Validates INV-003, INV-017.)

---

### 3.7 Performance Budgets and Design Point

**What it is**: A table of performance targets anchored to a specific design point (hardware, workload, scale).

**Required components**:

1. **Design point**: The specific scenario these budgets apply to. E.g., "M1 Max, 300 concurrent agents, 10K tasks, 60Hz TUI refresh."

2. **Budget table**: Operation -> target -> measurement method.

3. **Measurement harness description**: How to run the benchmarks (at minimum, benchmark names and what they simulate).

4. **Adjustment guidance**: "These are validated against the design point. If your design point differs, adjust with reasoning — but document the new targets and re-validate."

**Quality criteria**: An implementer can run the benchmarks and get a pass/fail signal without asking anyone.

**DO NOT** include performance claims without numbers, design points, or measurement methods. "The system should be fast enough for real-time use" is not a budget. (Validates INV-005, INV-017.)

**Anti-pattern**: "The system should be fast enough for real-time use." -- No number, no design point, no measurement method.

---

### 3.8 Negative Specifications

**What it is**: Explicit constraints on what the system (or subsystem) must NOT do, organized per implementation chapter.

**Why it exists**: LLMs fill specification gaps with plausible but unauthorized behaviors (§0.2.2). Negative specifications are co-located with the subsystem they constrain and use imperative language LLMs follow — more effective than distant anti-patterns. (Locked by ADR-009, validates INV-017.)

**Required format per implementation chapter**:
```
### Negative Specifications for [Subsystem]

- **DO NOT** [specific prohibited behavior]. [Brief reason or invariant reference.]
- **DO NOT** [specific prohibited behavior]. [Brief reason or invariant reference.]
- **DO NOT** [specific prohibited behavior]. [Brief reason or invariant reference.]
```

**Quality criteria for each negative specification**:
- Addresses a plausible action an implementer (especially an LLM) might take
- Is falsifiable: you can mechanically check whether the implementation violates it
- References the invariant or ADR it protects
- Is specific to the subsystem, not a generic platitude ("DO NOT write bugs" is not useful)

**Quantity guidance**: 3-8 negative specs per subsystem. Fewer suggests under-specification of boundaries. More suggests the subsystem's positive spec is unclear (if you need 15 "DO NOT" constraints, the "DO" section is probably ambiguous).

**DO NOT** write generic negative specs that apply to all subsystems ("DO NOT introduce security vulnerabilities"). Write subsystem-specific constraints that prevent the most likely misunderstanding of THAT subsystem. (Validates INV-017.)

**Anti-patterns**:
```
X BAD: "DO NOT write bad code."
  -- Not specific, not falsifiable, not subsystem-specific.

X BAD: "DO NOT use global variables."
  -- Generic programming advice, not a spec-level constraint.

GOOD: "DO NOT bypass the reservation system for file writes.
  All file mutations must go through the ReservationManager (INV-022).
  Direct filesystem writes will cause data races with concurrent agents."

GOOD: "DO NOT assume event ordering beyond the guarantees in APP-INV-017.
  Events from different agents may arrive out of wall-clock order.
  The only ordering guarantee is per-agent causal ordering."
```

**Self-bootstrapping demonstration**: This document includes negative specifications throughout its own element specifications (the "DO NOT" paragraphs in §2.1, §2.2, §3.1, §3.2, etc.).

### Verification Prompt for Chapter 3 (PART 0 Elements)

After writing your spec's PART 0, verify:
1. [ ] Every non-negotiable could tempt an implementer to violate it under pressure — none are trivially obvious (§3.1)
2. [ ] Every non-goal is something someone would plausibly request, not an absurd exclusion (INV-017: explicit "DO NOT" constraints prevent the most likely hallucination patterns)
3. [ ] The first-principles model is formal enough that the architecture can be derived from it independently (INV-001: every implementation section traces to the formal model)
4. [ ] Every invariant has all five components: statement, formal expression, violation scenario, validation method, WHY THIS MATTERS (INV-003: every invariant can be violated by a concrete scenario and detected by a named test)
5. [ ] Every ADR has >=2 genuine alternatives where a competent engineer could choose differently (INV-002: every choice where a reasonable alternative exists is captured in an ADR)
6. [ ] Performance budgets have numbers, design points, and measurement methods — no aspirational claims (INV-005: every performance claim is tied to a benchmark and design point)
7. [ ] Your PART 0 does NOT contain non-negotiables that merely restate invariants (§3.1), strawman ADRs with obviously inferior options (§3.5), or unfalsifiable invariants (§3.4)

---

## Chapter 4: PART I Elements

### 4.1 Full Formal Model

**What it is**: The expanded version of the first-principles derivation from §0.2. While the executive summary gives the 1-page version, PART I gives the full treatment:

- Complete state definition (all fields, all types)
- Complete input/event taxonomy
- Complete output/effect taxonomy
- State transition semantics
- Composition rules (how subsystems interact)

### 4.2 State Machines

**What it is**: Every stateful component gets a formal state machine.

**Required per state machine**:
- State diagram (ASCII art or description)
- State x Event table (what happens for every combination — no empty cells)
- Guard conditions on transitions
- Invalid transition policy (ignore? error? log?)
- Entry/exit actions

**Quality criteria**: The state x event table has no empty cells. Every cell either names a transition or explicitly says "no transition" or "error."

**DO NOT** define state machines with only happy-path transitions. LLMs will implement only the transitions you show them. If you omit invalid transition handling, the LLM will either ignore invalid transitions (silent corruption) or crash (poor UX). (Validates INV-010, INV-017.)

### 4.3 Complexity Analysis

**What it is**: Complexity bounds for every fundamental operation defined in the first-principles model.

**Required**: Big-O bounds with constants where they matter for the design point. "O(n) where n = active_agents, expected <=300" is more useful than "O(n)."

**DO NOT** provide complexity bounds without anchoring to the design point. An LLM given "O(n^2)" cannot assess whether this is acceptable without knowing n at the design point. (Validates INV-005.)

### Verification Prompt for Chapter 4 (PART I Elements)

After writing your spec's PART I (Foundations), verify:
1. [ ] The full formal model includes complete state, input, output, and transition definitions — not just the summary from §0.2 (§4.1)
2. [ ] Every state machine has a state x event table with NO empty cells — every cell names a transition or says "invalid — [policy]" (INV-010: every state machine defines all states, transitions, guards, and invalid transition policy)
3. [ ] Invalid transition policies are explicit for every state machine — not just happy-path transitions (INV-010, INV-017)
4. [ ] Complexity analysis includes constants at the design point, not just asymptotic bounds (§4.3)
5. [ ] Your PART I does NOT define state machines with only happy-path transitions (§4.2) or complexity bounds without design-point context

---

## Chapter 5: PART II Elements

### 5.1 Implementation Chapters

**What it is**: One chapter per major subsystem — where the spec earns its value.

**Required components per chapter**:

1. **Purpose statement** (2-3 sentences): What this subsystem does and why it exists. References the formal model.
2. **Formal types**: Data structures with memory layout analysis where relevant. Include `// WHY NOT` annotations on non-obvious choices (see §5.4).
3. **Algorithm pseudocode**: Every non-trivial algorithm, in pseudocode or "close to [language]" sketches. Include complexity analysis inline.
4. **State machine** (if stateful): Full state machine per §4.2.
5. **Invariants preserved** (RESTATED): Which INV-NNN this subsystem is responsible for maintaining. **Restate each invariant's one-line statement, not just the ID** — this is required by INV-018 to prevent context loss in long documents.
6. **Negative specifications**: 3-8 "DO NOT" constraints specific to this subsystem, per §3.8. (Required by INV-017.)
7. **Worked example(s)**: At least one concrete scenario showing the subsystem in action with specific values, not variables.
8. **Edge cases and error handling**: What happens when inputs are malformed, resources are exhausted, or invariants are threatened.
9. **Test strategy**: What kinds of tests (unit, property, integration, replay, stress) cover this subsystem.
10. **Performance budget**: The subsystem's share of the overall performance budget.
11. **Verification prompt**: A structured self-check prompt per §5.6.
12. **Meta-instructions** (if applicable): Implementation ordering directives per §5.7.
13. **Cross-references**: To ADRs, invariants, other subsystems, the formal model.

**Quality criteria**: An implementer could build this subsystem from this chapter alone. (Understanding composition requires other chapters, but each chapter is self-contained for its subsystem.)

**DO NOT** write implementation chapters before locking their ADR dependencies — you will rewrite them when decisions change. **DO NOT** reference invariants by ID alone — restate them (INV-018). (Validates INV-001, INV-017, INV-018.)

---

### 5.2 Worked Examples

**What it is**: A concrete scenario with specific values (not variables) showing the subsystem processing a realistic input.

**Required properties**:
- Uses concrete values: `task_id = T-042`, not "some task"
- Shows state before, the operation, and state after
- Includes at least one non-trivial aspect (an edge case, a conflict, a boundary condition)

**DO NOT** use variables or placeholders. LLMs over-index on examples (§0.2.2) — "some task" produces vague implementations; `task_id = T-042` teaches precision. (Validates INV-017.)

**Anti-pattern**:
```
X BAD:
  "When a task is completed, the scheduler updates the DAG."
  -- No concrete values. No before/after state. No edge case.

GOOD:
  "Agent A-007 completes task T-042 (Implement login endpoint).
  Before: T-042 status=InProgress, T-043 depends on [T-042, T-041], T-041 status=Done
  Operation: TaskCompleted { task_id: T-042, agent_id: A-007, artifacts: [login.rs] }
  After: T-042 status=Done, T-043 status=Ready (all deps satisfied), T-043 enters scheduling queue
  Edge case: If T-043 had been cancelled while T-042 was in progress, T-043 remains Cancelled --
  completion of a dependency does not resurrect a cancelled task."
```

---

### 5.3 End-to-End Trace

**What it is**: A single worked scenario that traverses ALL major subsystems, showing how they interact.

**Required properties**:
- Traces one event or action from ingestion through every subsystem to final output
- Shows the exact data at each subsystem boundary
- Identifies which invariants are exercised at each step
- Includes at least one cross-subsystem interaction that could go wrong

**Why it exists**: Individual examples prove each piece works. The end-to-end trace proves the pieces fit together. Most bugs live at subsystem boundaries. (Validates INV-001.)

**Self-bootstrapping demonstration**: This document includes an end-to-end trace in §1.4 — tracing ADR-002 from recognition through the full DDIS authoring process.

---

### 5.4 WHY NOT Annotations

**What it is**: Inline comments next to design choices explaining the road not taken.

**When to use**: Whenever a design choice might look suboptimal to an implementer who doesn't have the full context. If an implementer might think "I can improve this by doing X instead," and X was considered and rejected, add a WHY NOT annotation.

**Format**:
```
// WHY NOT [alternative]? [Brief tradeoff explanation. Reference ADR-NNN if a full ADR exists.]
```

**Relationship to ADRs**: WHY NOT annotations are micro-justifications for local choices. ADRs are macro-justifications for architectural choices. If a WHY NOT annotation grows beyond 3 lines, it should become an ADR.

---

### 5.5 Comparison Blocks

**What it is**: Side-by-side SUBOPTIMAL vs CHOSEN comparisons with quantified reasoning.

**When to use**: For data structure choices, algorithm choices, or API designs where the quantitative difference is the justification.

**Format**:
```
// SUBOPTIMAL: [Rejected approach]
//   - [Quantified downside 1]
//   - [Quantified downside 2]
// CHOSEN: [Selected approach]
//   - [Quantified advantage 1]
//   - [Quantified advantage 2]
//   - See ADR-NNN for full analysis
```

---

### 5.6 Verification Prompts

**What it is**: A structured self-check at the end of each implementation chapter for verifying output against the spec before moving on. (Locked by ADR-010.)

**Required format**:
```
### Verification Prompt for [Subsystem]

After implementing this subsystem, verify:
1. [ ] [Specific positive check referencing INV-NNN: "Your implementation preserves INV-NNN by..."]
2. [ ] [Specific positive check: "The [algorithm] handles [edge case] by..."]
3. [ ] [Specific negative check: "Your implementation does NOT [prohibited behavior from §3.8]"]
4. [ ] [Specific integration check: "Your implementation's output is compatible with [adjacent subsystem]"]
```

**Quality criteria**:
- Each check is executable by the implementer without additional information
- At least one check is a positive invariant verification
- At least one check is a negative verification (references a negative specification)
- At least one check verifies integration with an adjacent subsystem
- Checks reference specific invariants (INV-NNN) or negative specifications

**DO NOT** write generic verification prompts ("did you test your code?"). Each check must be specific to the subsystem and reference concrete invariants or constraints. (Validates INV-017.)

**Self-bootstrapping demonstration**: An explicit verification prompt for this meta-standard:

> **Verification Prompt for a DDIS-conforming spec:**
> After writing your spec, verify:
> 1. [ ] Every implementation chapter has >=3 negative specifications (INV-017)
> 2. [ ] Every implementation chapter restates its preserved invariants (INV-018)
> 3. [ ] An explicit implementation ordering exists as a DAG (INV-019)
> 4. [ ] Five random sections trace backward to the formal model (INV-001, Gate 2)
> 5. [ ] The cross-reference graph has no orphan sections (INV-006, Gate 5)
> 6. [ ] Your spec does NOT contain aspirational invariants without violation scenarios (INV-003)

---

### 5.7 Meta-Instructions

**What it is**: Directives to the LLM implementer providing ordering, sequencing, and process guidance that human implementers infer from experience.

**Required format**:
```
> **META-INSTRUCTION**: [Directive to the implementer]
> Reason: [Why this ordering/process matters]
```

**When to use**:
- When implementation order matters and getting it wrong causes cascading rework (supports INV-019)
- When a common implementation shortcut would violate an invariant
- When the spec intentionally leaves a micro-decision to the implementer but wants to constrain the decision process

**DO NOT** use meta-instructions for things the spec should state directly. A meta-instruction is process guidance ("implement X before Y"), not specification content ("X must have property P"). If you find yourself using meta-instructions to convey requirements, the requirements section is incomplete. (Validates INV-017.)

**Example**:
```
> **META-INSTRUCTION**: Implement the event store before the scheduler.
> Reason: The scheduler depends on event store types (EventId, EventPayload)
> defined in the storage domain constitution. Implementing the scheduler first
> will require placeholder types that inevitably diverge from the real types.

> **META-INSTRUCTION**: Do not optimize the dispatch hot path until
> Benchmark B-003 confirms it is actually the bottleneck at the design point.
> Reason: Premature optimization of dispatch is the #1 cause of unnecessary
> complexity in coordination systems (see ADR-005, consequences).
```

**Self-bootstrapping demonstration**: This document includes meta-instructions in §0.3.1 (reading order for LLM implementers) and §11.1 (authoring sequence).

### Verification Prompt for Chapter 5 (PART II Elements)

After writing your spec's implementation chapters, verify:
1. [ ] Each chapter has all 13 required components from §5.1 (purpose, types, algorithms, state machine, invariants RESTATED, negative specs, examples, edge cases, tests, budgets, verification prompt, meta-instructions, cross-refs)
2. [ ] Preserved invariants are RESTATED with at minimum ID + one-line statement, not bare ID references (INV-018: every implementation chapter restates the invariants it must preserve)
3. [ ] Each chapter has >=3 subsystem-specific negative specifications using the §3.8 format (INV-017: every implementation chapter includes explicit "DO NOT" constraints)
4. [ ] Worked examples use concrete values (task_id = T-042), not variables or placeholders (§5.2)
5. [ ] Verification prompts include positive, negative, AND integration checks referencing specific INV-NNN (§5.6)
6. [ ] Meta-instructions use the prescribed `> **META-INSTRUCTION**:` format with dependency reasons (§5.7, INV-019)
7. [ ] Your implementation chapters do NOT reference invariants by ID alone (violates INV-018), use "see above" references (violates INV-006: cross-references use explicit §X.Y identifiers), or include generic negative specs like "DO NOT write bugs" (violates INV-017)

---

## Chapter 6: PART IV Elements

### 6.1 Operational Playbook

**What it is**: Prevents the most common failure mode of detailed specs: infinite refinement without shipping.

**Required sections**:

#### 6.1.1 Phase -1: Decision Spikes

Run tiny experiments to de-risk the hardest unknowns before building. Each spike produces an ADR.

**Required per spike**: What question it answers, maximum time budget (1-3 days), exit criterion (one ADR).

#### 6.1.2 Exit Criteria per Phase

Every phase in the roadmap must have a specific, testable exit criterion. Not "phase complete when done" but "phase complete when X, Y, Z are demonstrated."

**Anti-pattern**: "Phase 2: Implement the scheduler. Exit: Scheduler works."
**Good example**: "Phase 2: Implement the scheduler. Exit: Property test demonstrates fair scheduling across 50 agents with no starvation for > 1000 ticks. Benchmark shows dispatch completes in < 1ms at design point."

#### 6.1.3 Merge Discipline

What every PR touching invariants, reducers, or critical paths must include:
- Tests appropriate to the change
- A note on which invariants it preserves
- Benchmark comparison if touching a hot path

#### 6.1.4 Minimal Deliverables Order

Build order chosen to maximize the "working subset" at each stage. The first deliverable exercises the core loop, not a complete system missing its core. Must be an explicit DAG with dependency reasons (INV-019).

#### 6.1.5 Immediate Next Steps (First PRs)

The literal first 5-6 things to implement, in dependency order. Not strategic — tactical. Converts the spec from "a plan to study" into "a plan to execute now."

---

### 6.2 Testing Strategy

**What it is**: A taxonomy of test types used in the project, with examples and guidance on when to use each.

**Required taxonomy** (adapt to domain):

| Test Type | What It Validates | Example |
|---|---|---|
| Unit | Individual function correctness | Reservation conflict detection returns correct overlaps |
| Property | Invariant preservation under random inputs | forall events: replay(snapshot, events) = direct_state |
| Integration | Subsystem composition | Completed task triggers correct scheduling cascade |
| Stress | Behavior at design point limits | 300 agents, 10K tasks, sustained 60s |
| Replay | Determinism | Process N events, snapshot, replay, byte-compare |
| Adversarial | Robustness against malicious/malformed input | Agent sends event with forged task_id |
| LLM conformance | Spec faithfulness | LLM implementation matches negative specs (Gate 7) |

---

### 6.3 Error Taxonomy

**What it is**: A classification of errors the system can encounter, with handling strategy per class.

**Required properties**:
- Each error class has a severity (fatal, degraded, recoverable, ignorable)
- Each error class has a handling strategy (crash, retry, degrade, log-and-continue)
- Cross-references to invariants: which invariants might be threatened by each error class

For the error taxonomy of specification authoring errors, see Appendix C.

**DO NOT** conflate error severity with handling strategy. A "recoverable" error with a "crash" handler, or a "fatal" error with "log-and-continue", signals an inconsistent error model that an LLM will implement inconsistently. (Validates INV-017.)

### Verification Prompt for Chapter 6 (PART IV Elements)

After writing your spec's operational chapters, verify:
1. [ ] The operational playbook includes Phase -1 decision spikes with time budgets and ADR exit criteria (§6.1.1)
2. [ ] Every phase has a specific, testable exit criterion — not "phase complete when done" (§6.1.2, INV-003)
3. [ ] The minimal deliverables order is an explicit DAG with dependency reasons (INV-019)
4. [ ] The testing strategy includes at minimum: unit, property, integration, and stress test types with examples (§6.2)
5. [ ] The error taxonomy maps each error class to severity, handling strategy, and threatened invariants (§6.3)
6. [ ] Your operational chapters do NOT use aspirational exit criteria ("scheduler works"), generic test types without examples, or error classes without severity and handling strategy

---

## Chapter 7: Appendix Elements

### 7.1 Glossary

**What it is**: Every domain-specific term, defined in 1-3 sentences with a cross-reference to where it's formally specified.

**Required properties**:
- Alphabetized
- Each entry includes (see §X.Y) pointing to the formal definition
- Terms that have both a common meaning and a domain-specific meaning clearly distinguish the two

**DO NOT** define terms with circular references ("task: a unit of work in the task system"). **DO NOT** assume common-English meaning is sufficient for domain terms — LLMs will default to the most common meaning unless explicitly overridden. (Validates INV-009, INV-017.)

**Anti-pattern**: Defining "task" as "a unit of work." Define it as "a node in the task DAG representing a discrete, assignable unit of implementation work with explicit dependencies, acceptance criteria, and at most one assigned agent at any time (see §7.2, INV-012)."

---

### 7.2 Risk Register

**What it is**: Top 5-10 risks to the project, each with a concrete mitigation.

**Required per risk**:
- Risk description (what could go wrong)
- Impact (what happens if it materializes)
- Mitigation (what we do about it)
- Detection (how we know it's happening)

---

### 7.3 Master TODO Inventory

**What it is**: A comprehensive, checkboxable task list organized by subsystem, cross-referenced to phases and ADRs.

**Required properties**:
- Organized by subsystem (not by phase — phases cut across subsystems)
- Each item is small enough to be a single PR
- Cross-references to the ADR or invariant that justifies it
- Checkboxable format (`- [ ]`) so the document serves as a living tracker

**DO NOT** organize the Master TODO by phase alone — subsystem organization ensures that an LLM implementing one subsystem can find all related tasks without scanning the entire list. (Validates INV-017.)

### Verification Prompt for Chapter 7 (Appendix Elements)

After writing your spec's appendices, verify:
1. [ ] The glossary defines every domain-specific term with a cross-reference to its formal definition (INV-009)
2. [ ] Glossary definitions distinguish domain-specific meaning from common-English meaning where applicable (INV-009)
3. [ ] The risk register includes detection methods, not just mitigations — how do you know a risk is materializing? (§7.2)
4. [ ] The Master TODO is organized by subsystem and cross-referenced to ADRs and phases (§7.3)
5. [ ] Your appendices do NOT contain circular glossary definitions ("task: a unit of work in the task system") or risks without detection methods
