---
module: element-specifications
domain: core
maintains: []
interfaces: [INV-001, INV-002, INV-003, INV-004, INV-005, INV-006, INV-007, INV-008, INV-009, INV-010, INV-017, INV-018, INV-019, INV-020]
implements: [ADR-008, ADR-009, ADR-010]
adjacent: [core-standard, guidance-operations]
negative_specs:
  - "Must NOT state design goals in terms of implementation technology"
  - "Must NOT write generic negative specs that apply to all subsystems"
  - "Must NOT write generic verification prompts without subsystem-specific checks"
---

# PART II: CORE STANDARD — ELEMENT SPECIFICATIONS

**Key invariants governing element specifications (INV-018 compliance):**
- INV-017: Every implementation chapter includes ≥3 explicit DO NOT constraints preventing likely hallucination patterns (maintained by core-standard module)
- INV-018: Every implementation chapter restates the invariants it must preserve, not merely referencing them by ID (maintained by core-standard module)
- INV-020: Every element specification chapter includes a structured verification prompt block (maintained by core-standard module)
- INV-004: Every described algorithm includes pseudocode, complexity analysis, worked example, and edge cases (maintained by core-standard module)

(Full definitions for all 20 invariants: see core-standard module §0.5)

The heart of DDIS. Each section specifies one structural element: what it must contain, quality criteria, how it relates to other elements, and what good versus bad looks like. Each includes woven LLM-specific provisions (ADR-008).

## Chapter 2: Preamble Elements

### 2.1 Design Goal

**What it is**: A single sentence (≤ 30 words) that states the system's reason for existing.

**Required properties**:
- States the core value proposition, not the implementation
- Uses bold for emphasis on the 3–5 key properties
- Readable by a non-technical stakeholder

**Quality criteria**: A reader who sees only the design goal should be able to decide whether this system is relevant to them.

**DO NOT** state the design goal in terms of implementation technology ("Build a Rust-based event-sourced system") — state it in terms of value. (Validates INV-017.)

**DO NOT** exceed 30 words — longer becomes a design essay that LLMs treat as implementation requirements. (Validates INV-007.)

**DO NOT** use unmeasurable qualities ("robust", "scalable", "enterprise-grade") — LLMs generate boilerplate from abstract adjectives. (Validates INV-017.)

**Anti-pattern**: "Design goal: Build a distributed task coordination system using event sourcing and advisory reservations." ← This describes implementation, not value.

**Good example** (FrankenTUI): "Design goal: **scrollback-native, zero-flicker, agent-ergonomic, and high-performance** Rust terminal apps."

**Cross-references**: The design goal establishes vocabulary used throughout. Each bolded property should correspond to at least one invariant and one quality gate.

---

### 2.2 Core Promise

**What it is**: A single sentence (≤ 40 words) that describes what the system makes possible, from the user's perspective.

**Required properties**:
- Written from the user's viewpoint, not the architect's
- States concrete capabilities, not abstract properties
- Uses "without" clauses to highlight what would normally be sacrificed

**Quality criteria**: If you showed only this sentence to a potential user, they should understand what the system gives them and what it doesn't cost them.

**DO NOT** use abstract qualities without concrete meaning ("robust", "scalable", "enterprise-grade"). (Validates INV-017.)

**DO NOT** promise implementation details ("uses React", "built on PostgreSQL") — technical choices belong in ADRs. (Validates INV-002.)

**DO NOT** omit "without" clauses — a promise stating only what the system does leaves the most important constraints implicit. (Validates INV-017.)

**Anti-pattern**: "The system provides robust, scalable, enterprise-grade coordination." ← Meaningless buzzwords.

**Good example** (FrankenTUI): "ftui is designed so you can build a Claude Code / Codex-class agent harness UI without flicker, without cursor corruption, and without sacrificing native scrollback."

---

### 2.3 Document Note

**What it is**: A short disclaimer (2–4 sentences) about code blocks and where correctness lives.

**Why it exists**: Without this note, implementers treat code blocks as copy-paste targets. The document note redirects trust from code to invariants and tests. LLMs will reproduce code blocks verbatim unless explicitly told otherwise.

**DO NOT** omit this note even if it seems obvious — LLMs reproduce code blocks verbatim unless told otherwise. (Validates INV-017.)

**DO NOT** let the document note exceed 4 sentences — longer means you are embedding guidance that belongs in §2.4. (Validates INV-007.)

**DO NOT** omit mention of invariants and tests as the source of truth. (Validates INV-008, INV-017.)

**Template**:
> Code blocks in this plan are **design sketches** for API shape, invariants, and responsibilities.
> They are intentionally "close to [language]," but not guaranteed to compile verbatim.
> The correctness contract lives in the invariants, tests, and ADRs — not in pseudo-code syntax.

---

### 2.4 How to Use This Plan

**What it is**: A numbered list (4–6 items) giving practical reading and execution guidance.

**Required properties**:
- Starts with "Read PART 0 end-to-end"
- Identifies the churn-magnets to lock via ADRs
- Points to the Master TODO as the execution tracker
- Identifies at least one non-negotiable process requirement
- For LLM implementers: includes a step about reading negative specifications and verification prompts

**Quality criteria**: A new team member reading only this section knows exactly how to engage with the document.

**DO NOT** list more than 8 steps — this is a quick-start guide, not an operating manual. Process details belong in PART IV. (Validates INV-007.)

**DO NOT** omit the LLM-specific reading step for negative specifications and verification prompts — without it, LLMs skip constraints and implement from positive descriptions only. (Validates INV-017, INV-020.)

**DO NOT** use vague steps like "Understand the architecture" — every step must name a specific section or action. (Validates INV-017.)

### Verification Prompt for Chapter 2 (Preamble Elements)

After writing your spec's preamble, verify:
1. [ ] Design goal is ≤ 30 words and states value, not implementation technology (INV-017: every implementation chapter includes explicit "DO NOT" constraints — applied here as the negative spec against implementation-focused design goals)
2. [ ] Core promise uses "without" clauses and contains no abstract buzzwords (INV-017)
3. [ ] Document note explicitly states code blocks are design sketches, not copy-paste targets (INV-008: the spec is self-contained — this note prevents misinterpretation)
4. [ ] How-to-use list starts with "Read PART 0" and includes LLM-specific step for negative specs and verification prompts
5. [ ] Your preamble does NOT use marketing language ("enterprise-grade", "cutting-edge") — these cause LLMs to generate generic boilerplate
6. [ ] *Integration*: Your preamble design goal properties each correspond to at least one invariant in §0.5 and one quality gate in §0.7 (INV-001)

---

## Chapter 3: PART 0 Elements

### 3.1 Non-Negotiables (Engineering Contract)

**What it is**: 5–10 properties defining what the system IS. Stronger than invariants (which are formal and testable) — these are philosophical commitments that must never be compromised, even under pressure.

**Required format**:
```
- **[Property name in bold]**
  [One sentence explaining what this means concretely]
```

**Quality criteria for each non-negotiable**:
- An implementer could imagine a situation where violating it would be tempting (e.g., "just skip replay validation in dev mode — it's slow")
- The non-negotiable clearly says: no, even then
- It is not a restatement of a technical invariant; it is a commitment

**DO NOT** restate invariants as non-negotiables — non-negotiables are philosophical commitments; invariants are testable properties. (Validates INV-017.)

**DO NOT** list more than 10 — more than 10 means some are actually preferences, diluting the ones that matter. (Validates INV-007.)

**DO NOT** write non-negotiables that no reasonable person would violate ("the system must not corrupt data") — constrain tempting shortcuts, not universal ethics. (Validates INV-017.)

**Relationship to invariants**: Non-negotiables are the "why" that justifies groups of invariants. "Deterministic replay is real" (non-negotiable) justifies INV-003: "Same event log → identical state" (invariant). The non-negotiable is the commitment; the invariant is the testable manifestation.

---

### 3.2 Non-Goals

**What it is**: A list of 5–10 things the system explicitly does NOT attempt.

**Why it exists**: Scope creep is the most common spec failure. Non-goals give implementers permission to say "out of scope." For LLMs, non-goals prevent adding "helpful" features not in the spec.

**Quality criteria for each non-goal**:
- Someone has actually asked for this (or will), making the exclusion non-obvious
- The non-goal explains briefly why it's excluded (not just "not in scope" but why not)

**DO NOT** list absurd non-goals that nobody would request — every non-goal should exclude something a reasonable stakeholder has or will request. (Validates INV-007, INV-017.)

**DO NOT** frame non-goals as deferred features ("not yet," "in a future version"). A non-goal is a permanent scope boundary, not a backlog item. Deferred features cause LLMs to scaffold placeholder infrastructure. (Validates INV-017.)

**DO NOT** include non-goals that duplicate non-negotiables from §3.1 — non-goals exclude scope; non-negotiables define identity. Conflating them dilutes both. (Validates INV-007, INV-017.)

**Anti-pattern**: "Non-goal: Building a quantum computer." ← Nobody asked for this. Non-goals should exclude things that are tempting, not absurd.

---

### 3.3 First-Principles Derivation

**What it is**: The formal model from which the entire architecture derives. Makes every section feel *inevitable* rather than *asserted*.

**Required components**:

1. **"What IS a [System]?"** — A mathematical or pseudo-mathematical definition:
   ```
   System: (State, Input) → (State', Output)
   where:
     State = { ... }
     Input = { ... }
     Output = { ... }
   ```
   This establishes the system as a formally defined state machine or function.

2. **Consequences** — 3–5 bullet points explaining what this formal definition implies for the architecture. Each consequence should feel like a discovery, not an assertion.

3. **Fundamental Operations Table** — Every primitive operation with its mathematical model and complexity target.

**Quality criteria**: After reading this section, an implementer should be able to derive the system's architecture independently. If the architecture is a surprise after reading the first principles, the derivation is incomplete.

**DO NOT** assert the architecture without deriving it from the formal model — an LLM given an asserted architecture will make downstream decisions that violate the model. (Validates INV-001, INV-017.)

**DO NOT** present the formal model in a specific programming language. Use mathematical notation, pseudocode, or language-neutral set theory — language-specific syntax constrains LLMs to language-shaped solutions. (Validates INV-008, INV-017.)

**DO NOT** skip the Consequences section — without it, LLMs treat the formal model as decoration and assert the architecture independently. Consequences bridge model to architecture. (Validates INV-001, INV-017.)

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

**Quantity guidance**: A medium-complexity system typically has 10–25 invariants. Fewer suggests under-specification. More suggests the invariants are too granular (consider grouping related invariants under a non-negotiable).

**DO NOT** write invariants without violation scenarios (violates INV-003). **DO NOT** restate type system guarantees as invariants (e.g., "TaskId values are unique" when using newtype with auto-increment). **DO NOT** write aspirational invariants without measurable criteria. (Validates INV-003, INV-017.)

**Anti-patterns**:
```
❌ BAD: "INV-001: The system shall be performant."
  - Not falsifiable (what is "performant"?)
  - No violation scenario (everything is or isn't "performant")
  - Not testable

❌ BAD: "INV-002: TaskId values are unique."
  - Trivially enforced by the type system (use a newtype with a counter)
  - Not worth an invariant unless uniqueness has subtle cross-boundary implications

✅ GOOD: "INV-003: Event Log Determinism
  Same event sequence applied to same initial state produces identical final state.
  ∀ events, ∀ state₀: reduce(state₀, events) = reduce(state₀, events)
  Violation: A reducer reads wall-clock time, causing different states on replay.
  Validation: Replay test — process 10K events, snapshot, replay from scratch, byte-compare.
  // WHY THIS MATTERS: If replay diverges, we lose auditability and debugging via replay."
```

---

### 3.5 Architecture Decision Records (ADRs)

**What it is**: A record of each significant design decision, including the alternatives that were rejected and why.

**Required format per ADR**:

```
### ADR-NNN: [Descriptive Title]

#### Problem
[1–3 sentences describing the decision that needs to be made]

#### Options
A) **[Option name]**
- Pros: [concrete advantages]
- Cons: [concrete disadvantages]

B) **[Option name]**
- Pros: [concrete advantages]
- Cons: [concrete disadvantages]

[At least 2 options, at most 4]

#### Decision
**[Chosen option]**: [Rationale in 2–5 sentences]

// WHY NOT [rejected option]? [Brief explanation]

#### Consequences
[2–4 bullet points on what this decision implies for the rest of the system]

#### Tests
[How we will know this decision was correct or needs revisiting]

#### Confidence [Optional]
**[Committed|Provisional|Speculative]**: [Brief justification or spike reference]
```

**Confidence levels** (optional, recommended for early-stage specs):
- **Committed**: High confidence, well-analyzed, unlikely to change. Default if omitted.
- **Provisional**: Medium confidence, will be revisited after a decision spike (§6.1.1, guidance-operations module). Implementation should minimize coupling to this decision.
- **Speculative**: Low confidence, research needed. Implementation should use an abstraction boundary so the decision can be swapped without cascading rework.

**DO NOT** mark all ADRs as Committed in an early-stage spec — if no ADRs are Provisional, you are likely over-committing. (Validates INV-002, INV-017.)

**DO NOT** use Speculative confidence to defer decisions indefinitely — every Speculative ADR must reference a specific decision spike in §6.1.1 (guidance-operations module) with a time budget and exit criterion. (Validates INV-017, INV-019.)

**Quality criteria for each ADR**:
- **Genuine alternatives**: Each option must have a real advocate. If Option B is a strawman nobody would choose, it is not a genuine alternative. The test: would a competent engineer in a different context reasonably choose Option B?
- **Concrete tradeoffs**: Pros and cons cite specific, measurable properties — not vague qualities like "simpler" or "more robust."
- **Consequential decision**: The choice materially affects the system. If swapping Option A for Option B would require < 1 day of refactoring, it's not an ADR — it's a local implementation choice.

**DO NOT** include decisions that predate the spec's scope (e.g., language choice if already decided). **DO NOT** create strawman ADRs where one option is obviously superior. **DO NOT** omit WHY NOT annotations for rejected options — these prevent LLMs from re-exploring rejected paths. (Validates INV-002, INV-017.)

**Anti-pattern**:
```
❌ BAD:
  ADR-001: Use Rust
  Options: A) Rust B) C++ C) Go
  Decision: Rust because it's safe.
  ← Not a genuine decision within the spec's scope.
    Language choice predates the spec.
    No concrete tradeoff analysis.
```

**Churn-magnets**: After all ADRs are written, add a brief section identifying which decisions cause the most downstream rework if changed. These are the decisions to lock first and spike earliest (see §6.1.1 in guidance-operations module, Phase -1).

---

### 3.6 Quality Gates

**What it is**: 4–8 stop-ship criteria, ordered by priority.

**Required properties per gate**:
- A gate is a **predicate**, not a task. It is either passing or failing at any point in time.
- Each gate references specific invariants or test suites.
- Gates are ordered such that a failing Gate N makes Gate N+1 irrelevant.
- At least one gate specifically validates LLM implementation readiness (see Gate 7 in §0.7).

**Quality criteria**: A project manager should be able to assess gate status in < 30 minutes using the referenced tests. Proportional weight violations signal spec imbalance that may cause gate failures (see §9.2, guidance-operations module).

**DO NOT** define gates without concrete measurement procedures. "Code quality is high" is not a gate; "All invariants have passing tests" is. (Validates INV-003, INV-017.)

**DO NOT** define overlapping gates that test the same property — if two gates can fail for the same root cause, merge them. (Validates INV-007, INV-017.)

**DO NOT** omit the gate ordering rationale. Gates must be ordered so failing Gate N makes Gate N+1 irrelevant — without this, LLMs waste effort on later gates while earlier ones fail. (Validates INV-019, INV-017.)

---

### 3.7 Performance Budgets and Design Point

**What it is**: A table of performance targets anchored to a specific design point (hardware, workload, scale).

**Required components**:

1. **Design point**: The specific scenario these budgets apply to. E.g., "M1 Max, 300 concurrent agents, 10K tasks, 60Hz TUI refresh."

2. **Budget table**: Operation → target → measurement method.

3. **Measurement harness description**: How to run the benchmarks (at minimum, benchmark names and what they simulate).

4. **Adjustment guidance**: "These are validated against the design point. If your design point differs, adjust with reasoning — but document the new targets and re-validate."

**Quality criteria**: An implementer can run the benchmarks and get a pass/fail signal without asking anyone.

**DO NOT** include performance claims without numbers, design points, or measurement methods. "The system should be fast enough for real-time use" is not a budget. (Validates INV-005, INV-017.)

**DO NOT** anchor budgets to "current hardware" without naming the specific hardware — name the exact machine or class (e.g., "M1 Max, 32GB RAM") so budgets are reproducible. (Validates INV-005, INV-017.)

**DO NOT** omit adjustment guidance. Without it, implementers either treat budgets as absolute gospel on different hardware, or ignore them entirely. (Validates INV-008, INV-017.)

**Anti-pattern**: "The system should be fast enough for real-time use." ← No number, no design point, no measurement method.

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

**Quantity guidance**: 3–8 per subsystem. Fewer suggests under-specification; more suggests the positive spec is ambiguous.

**DO NOT** write generic negative specs that apply to all subsystems ("DO NOT introduce security vulnerabilities") — write subsystem-specific constraints. (Validates INV-017.)

**DO NOT** let negative specs exceed 8 per subsystem — more than 8 suggests the positive specification is too ambiguous; fix the positive spec instead. (Validates INV-007, INV-017.)

**DO NOT** write negative specs that simply negate a positive requirement ("DO NOT fail to implement X") — negative specs must address plausible wrong behaviors an implementer might choose, not restate requirements. (Validates INV-017.)

**Anti-patterns**:
```
❌ BAD: "DO NOT write bad code."
  ← Not specific, not falsifiable, not subsystem-specific.

❌ BAD: "DO NOT use global variables."
  ← Generic programming advice, not a spec-level constraint.

✅ GOOD: "DO NOT bypass the reservation system for file writes.
  All file mutations must go through the ReservationManager (INV-022).
  Direct filesystem writes will cause data races with concurrent agents."

✅ GOOD: "DO NOT assume event ordering beyond the guarantees in APP-INV-017.
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
5. [ ] Every ADR has ≥ 2 genuine alternatives where a competent engineer could choose differently (INV-002: every choice where a reasonable alternative exists is captured in an ADR)
6. [ ] Performance budgets have numbers, design points, and measurement methods — no aspirational claims (INV-005: every performance claim is tied to a benchmark and design point)
7. [ ] Your PART 0 does NOT contain non-negotiables that merely restate invariants (§3.1), strawman ADRs with obviously inferior options (§3.5), or unfalsifiable invariants (§3.4)
8. [ ] *Integration*: Your PART 0 invariants and ADRs are referenced by at least one PART II implementation chapter (INV-006)

---

## Chapter 4: PART I Elements

### 4.1 Full Formal Model

**What it is**: The expanded version of the first-principles derivation from §0.2. While the executive summary gives the 1-page version, PART I gives the full treatment:

- Complete state definition (all fields, all types)
- Complete input/event taxonomy
- Complete output/effect taxonomy
- State transition semantics
- Composition rules (how subsystems interact)

**DO NOT** copy-paste the §0.2 executive summary and call it the full formal model — include complete state definitions, input/output taxonomies, transition semantics, and composition rules. (Validates INV-004, INV-017.)

### 4.2 State Machines

**What it is**: Every stateful component gets a formal state machine.

**Required per state machine**:
- State diagram (ASCII art or description)
- State × Event table (what happens for every combination — no empty cells)
- Guard conditions on transitions
- Invalid transition policy (ignore? error? log?)
- Entry/exit actions

**Quality criteria**: The state × event table has no empty cells. Every cell either names a transition or explicitly says "no transition" or "error."

**DO NOT** define state machines with only happy-path transitions — LLMs implement only the transitions you show them. Omitting invalid transition handling causes silent corruption or crashes. (Validates INV-010, INV-017.)

### 4.3 Complexity Analysis

**What it is**: Complexity bounds for every fundamental operation defined in the first-principles model.

**Required**: Big-O bounds with constants where they matter for the design point. "O(n) where n = active_agents, expected ≤ 300" is more useful than "O(n)."

**DO NOT** provide complexity bounds without anchoring to the design point — "O(n²)" is meaningless without knowing n. (Validates INV-005.)

### Verification Prompt for Chapter 4 (PART I Elements)

After writing your spec's PART I (Foundations), verify:
1. [ ] The full formal model includes complete state, input, output, and transition definitions — not just the summary from §0.2 (§4.1)
2. [ ] Every state machine has a state × event table with NO empty cells — every cell names a transition or says "invalid — [policy]" (INV-010: every state machine defines all states, transitions, guards, and invalid transition policy)
3. [ ] Invalid transition policies are explicit for every state machine — not just happy-path transitions (INV-010, INV-017)
4. [ ] Complexity analysis includes constants at the design point, not just asymptotic bounds (§4.3)
5. [ ] Your PART I does NOT define state machines with only happy-path transitions (§4.2) or complexity bounds without design-point context
6. [ ] *Integration*: Your PART I state machines cover all states referenced by PART II implementation chapters (INV-010)

---

## Chapter 5: PART II Elements

### 5.1 Implementation Chapters

**What it is**: One chapter per major subsystem — where the spec earns its value.

**Required components per chapter**:

1. **Purpose statement** (2–3 sentences): What this subsystem does and why it exists. References the formal model.

2. **Formal types**: Data structures with memory layout analysis where relevant. Include `// WHY NOT` annotations on non-obvious choices (see §5.4) and comparison blocks for quantified design trade-offs (see §5.5).

3. **Algorithm pseudocode**: Every non-trivial algorithm, in pseudocode or "close to [language]" sketches. Include complexity analysis inline.

4. **State machine** (if stateful): Full state machine per §4.2.

5. **Invariants preserved** (RESTATED): Which INV-NNN this subsystem is responsible for maintaining. **Restate each invariant's one-line statement, not just the ID** — this is required by INV-018 to prevent context loss in long documents.

6. **Negative specifications**: 3–8 "DO NOT" constraints specific to this subsystem, per §3.8. (Required by INV-017.)

7. **Worked example(s)**: At least one concrete scenario showing the subsystem in action with specific values, not variables.

8. **Edge cases and error handling**: What happens when inputs are malformed, resources are exhausted, or invariants are threatened.

9. **Test strategy**: What kinds of tests (unit, property, integration, replay, stress) cover this subsystem.

10. **Performance budget**: The subsystem's share of the overall performance budget.

11. **Verification prompt**: A structured self-check prompt per §5.6.

12. **Meta-instructions** (if applicable): Implementation ordering directives per §5.7.

13. **Cross-references**: To ADRs, invariants, other subsystems, the formal model. Include proportional weight (§9.1, guidance-operations module) to validate line budget allocation.

**Quality criteria**: An implementer could build this subsystem from this chapter alone. (Understanding composition requires other chapters, but each chapter is self-contained for its subsystem.)

**DO NOT** write implementation chapters before locking their ADR dependencies. **DO NOT** reference invariants by ID alone — restate them per INV-018. (Validates INV-001, INV-017, INV-018.)

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
❌ BAD:
  "When a task is completed, the scheduler updates the DAG."
  ← No concrete values. No before/after state. No edge case.

✅ GOOD:
  "Agent A-007 completes task T-042 (Implement login endpoint).
  Before: T-042 status=InProgress, T-043 depends on [T-042, T-041], T-041 status=Done
  Operation: TaskCompleted { task_id: T-042, agent_id: A-007, artifacts: [login.rs] }
  After: T-042 status=Done, T-043 status=Ready (all deps satisfied), T-043 enters scheduling queue
  Edge case: If T-043 had been cancelled while T-042 was in progress, T-043 remains Cancelled —
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

**Why it exists**: Individual examples prove each piece works. The end-to-end trace proves the pieces fit together. Most bugs live at subsystem boundaries. (Validates INV-001: every section traces to the formal model.)

**DO NOT** trace only the happy path — include at least one cross-subsystem failure mode or conflict. Happy-path-only traces teach LLMs to implement only the success case. (Validates INV-003, INV-017.)

**DO NOT** use abstract values at subsystem boundaries. "The scheduler sends a message" is abstract; "The scheduler emits `TaskAssigned { task_id: T-042, agent_id: A-007, deadline: 1500ms }`" is concrete. (Validates INV-004, INV-017.)

**DO NOT** skip intermediary subsystems — show every A→B→C handoff explicitly. Skipping intermediaries causes LLMs to implement direct coupling. (Validates INV-006, INV-017.)

**Anti-pattern**:
```
❌ BAD:
  "An event enters the system. It gets processed by the scheduler, then the
  event store, then the TUI. The output is correct."
  ← No concrete values. No data at boundaries. No invariant annotations.
    No failure mode. Happy-path-only.

✅ GOOD (excerpt from §1.4, core-standard module):
  "Step 3: ADR Creation (§3.5) — The author writes ADR-002 per §3.5 format:
   Problem: Aspirational, formally proven, or falsifiable?
   Options: Three genuine alternatives with concrete pros/cons.
   Decision: Option C (falsifiable) with WHY NOT annotations.
   Tests: 'Validated by INV-003' — forward reference to the enforcing invariant."
  ← Concrete values, specific section references, traceable cross-subsystem path.
```

**Self-bootstrapping demonstration**: This document includes an end-to-end trace in §1.4 (core-standard module) — tracing ADR-002 from recognition through the full DDIS authoring process.

---

### 5.4 WHY NOT Annotations

**What it is**: Inline comments next to design choices explaining the road not taken.

**When to use**: Whenever a design choice might look suboptimal to an implementer who doesn't have the full context. If an implementer might think "I can improve this by doing X instead," and X was considered and rejected, add a WHY NOT annotation.

**Format**:
```
// WHY NOT [alternative]? [Brief tradeoff explanation. Reference ADR-NNN if a full ADR exists.]
```

**Relationship to ADRs**: WHY NOT annotations are micro-justifications for local choices. ADRs are macro-justifications for architectural choices. If a WHY NOT annotation grows beyond 3 lines, it should become an ADR.

**DO NOT** use WHY NOT for micro-decisions (variable naming, formatting, import ordering) — reserve for design choices where a reasonable alternative was rejected. (Validates INV-007, INV-017.)

**DO NOT** let WHY NOT annotations exceed 3 lines — longer explanations should be promoted to an ADR (§3.5). (Validates INV-002, INV-017.)

**Anti-pattern**:
```
❌ BAD:
  // WHY NOT use a different approach? Because this one is better.
  ← No tradeoff, no specificity, no ADR reference. "Better" is not a reason.

✅ GOOD:
  // WHY NOT mandatory locking? Causes priority inversion under high contention
  // (measured: 3× latency at 200 concurrent agents). Advisory locking avoids
  // this at cost of occasional stale reads. See ADR-003.
  ← Named alternative, quantified tradeoff, ADR reference for full analysis.
```

---

### 5.5 Comparison Blocks

**What it is**: Side-by-side ❌ SUBOPTIMAL vs ✅ CHOSEN comparisons with quantified reasoning.

**When to use**: For data structure choices, algorithm choices, or API designs where the quantitative difference is the justification.

**Format**:
```
// ❌ SUBOPTIMAL: [Rejected approach]
//   - [Quantified downside 1]
//   - [Quantified downside 2]
// ✅ CHOSEN: [Selected approach]
//   - [Quantified advantage 1]
//   - [Quantified advantage 2]
//   - See ADR-NNN for full analysis
```

**DO NOT** use comparison blocks without quantified reasoning — "Option A is slower" is not a comparison; numbers are required so LLMs can evaluate context-dependent tradeoffs. (Validates INV-005, INV-017.)

**DO NOT** compare more than 2 approaches in one block — three-way comparisons should be promoted to a full ADR (§3.5). (Validates INV-002, INV-017.)

**Anti-pattern**:
```
❌ BAD:
  // ❌ SUBOPTIMAL: HashMap
  //   - Slower for small collections
  // ✅ CHOSEN: BTreeMap
  //   - Faster for small collections
  ← Qualitative only ("slower", "faster"). No numbers. No design point.
    LLM cannot evaluate whether this holds for n=10 vs n=10,000.

✅ GOOD:
  // ❌ SUBOPTIMAL: HashMap<TaskId, Task>
  //   - O(1) amortized lookup, but 24 bytes overhead per entry (hash + metadata)
  //   - At design point (n ≤ 100 tasks), cache misses dominate: ~85ns per lookup
  // ✅ CHOSEN: BTreeMap<TaskId, Task>
  //   - O(log n) lookup, 8 bytes overhead per entry
  //   - At design point (n ≤ 100): ~12ns per lookup (cache-line friendly)
  //   - See ADR-005 for full benchmark analysis
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

**Self-bootstrapping demonstration**: This document's element specifications implicitly serve as verification prompts — each quality criteria section tells the author what to check. An explicit verification prompt for this meta-standard:

> **Verification Prompt for a DDIS-conforming spec:**
> After writing your spec, verify:
>
> *Positive checks:*
> 1. [ ] Five random sections trace backward to the formal model (INV-001, Gate 2)
> 2. [ ] Every design choice with a reasonable alternative has an ADR with ≥ 2 genuine options (INV-002, Gate 3)
> 3. [ ] Every algorithm has pseudocode, complexity analysis, ≥ 1 worked example, and a test strategy (INV-004)
> 4. [ ] Every performance claim has a named benchmark, a design point, and a measurement method (INV-005)
> 5. [ ] The cross-reference graph has no orphan sections (INV-006, Gate 5)
> 6. [ ] The spec is self-contained: an implementer needs no external information (INV-008)
> 7. [ ] Every domain-specific term is defined in the glossary (INV-009)
> 8. [ ] Every state machine has a complete state × event table with no empty cells (INV-010)
> 9. [ ] Every implementation chapter has ≥ 3 negative specifications (INV-017)
> 10. [ ] Every implementation chapter restates its preserved invariants with at minimum ID + one-line statement (INV-018)
> 11. [ ] An explicit implementation ordering exists as a DAG with dependency reasons (INV-019)
> 12. [ ] Every element specification chapter includes a verification prompt block with positive, negative, and integration checks (INV-020)
>
> *Negative checks:*
> 13. [ ] Your spec does NOT contain aspirational invariants without violation scenarios (INV-003)
> 14. [ ] Your ADRs do NOT use strawman alternatives where one option is obviously inferior (§3.5, Gate 3)
> 15. [ ] Your implementation chapters do NOT use "see above" references — all cross-refs use explicit §X.Y, INV-NNN, or ADR-NNN identifiers (INV-006)
>
> *Integration check:*
> 16. [ ] Give one implementation chapter (plus glossary and relevant invariants) to an LLM — it produces a correct implementation without hallucinating unauthorized behaviors (Gate 7)

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

**DO NOT** use meta-instructions for things the spec should state directly — a meta-instruction is process guidance ("implement X before Y"), not specification content ("X must have property P"). (Validates INV-017.)

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
3. [ ] Each chapter has ≥ 3 subsystem-specific negative specifications using the §3.8 format (INV-017: every implementation chapter includes explicit "DO NOT" constraints)
4. [ ] Worked examples use concrete values (task_id = T-042), not variables or placeholders (§5.2)
5. [ ] Verification prompts include positive, negative, AND integration checks referencing specific INV-NNN (§5.6)
6. [ ] Meta-instructions use the prescribed `> **META-INSTRUCTION**:` format with dependency reasons (§5.7, INV-019)
7. [ ] Your implementation chapters do NOT reference invariants by ID alone (violates INV-018), use "see above" references (violates INV-006: cross-references use explicit §X.Y identifiers), or include generic negative specs like "DO NOT write bugs" (violates INV-017)
8. [ ] *Integration*: Your PART II implementation chapters cross-reference the ADRs they depend on in PART 0 (INV-001, INV-006)

---

## Chapter 6: PART IV Elements

### 6.1 Operational Playbook

**What it is**: Prevents the most common failure mode of detailed specs: infinite refinement without shipping.

**Required sections**:

#### 6.1.1 Phase -1: Decision Spikes

Run tiny experiments to de-risk the hardest unknowns before building. Each spike produces an ADR.

**Required per spike**: What question it answers, maximum time budget (1–3 days), exit criterion (one ADR).

**DO NOT** define decision spikes without explicit time budgets — an open-ended spike is a project, not a spike. Maximum 1–3 days, exit criterion: one ADR. (Validates INV-017, INV-019.)

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

**DO NOT** present the deliverable order as a flat list — LLMs implement in list order, not dependency order. Make the DAG explicit with edges and reasons. (Validates INV-017, INV-019.)

#### 6.1.5 Immediate Next Steps (First PRs)

The literal first 5–6 things to implement, in dependency order. Not strategic — tactical. Converts the spec from "a plan to study" into "a plan to execute now."

---

### 6.2 Testing Strategy

**What it is**: A taxonomy of test types used in the project, with examples and guidance on when to use each.

**Required taxonomy** (adapt to domain):

| Test Type | What It Validates | Example |
|---|---|---|
| Unit | Individual function correctness | Reservation conflict detection returns correct overlaps |
| Property | Invariant preservation under random inputs | ∀ events: replay(snapshot, events) = direct_state |
| Integration | Subsystem composition | Completed task triggers correct scheduling cascade |
| Stress | Behavior at design point limits | 300 agents, 10K tasks, sustained 60s |
| Replay | Determinism | Process N events, snapshot, replay, byte-compare |
| Adversarial | Robustness against malicious/malformed input | Agent sends event with forged task_id |
| LLM conformance | Spec faithfulness | LLM implementation matches negative specs (Gate 7) |

**DO NOT** write only unit tests — every project needs at least unit, property, integration, and stress test types. (Validates INV-003, INV-017.)

**DO NOT** omit property tests for invariant-heavy systems — invariants are formal properties that hold under all inputs; property-based testing is their natural validation. (Validates INV-003, INV-017.)

**DO NOT** skip the LLM conformance test type — without it, Gate 7 has no concrete test suite to reference. (Validates INV-017, INV-020.)

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
1. [ ] The operational playbook includes Phase -1 decision spikes with time budgets and ADR exit criteria (§6.1.1, guidance-operations module)
2. [ ] Every phase has a specific, testable exit criterion — not "phase complete when done" (§6.1.2, INV-003: every invariant/criterion must be falsifiable)
3. [ ] The minimal deliverables order is an explicit DAG with dependency reasons (INV-019: the spec provides an explicit dependency chain for implementation ordering)
4. [ ] The testing strategy includes at minimum: unit, property, integration, and stress test types with examples (§6.2)
5. [ ] The error taxonomy maps each error class to severity, handling strategy, and threatened invariants (§6.3)
6. [ ] Your operational chapters do NOT use aspirational exit criteria ("scheduler works"), generic test types without examples, or error classes without severity and handling strategy
7. [ ] *Integration*: Your operational playbook phases reference the quality gates from §0.7 that must pass before phase exit (INV-019)

---

## Chapter 7: Appendix Elements

### 7.1 Glossary

**What it is**: Every domain-specific term, defined in 1–3 sentences with a cross-reference to where it's formally specified.

**Required properties**:
- Alphabetized
- Each entry includes (see §X.Y) pointing to the formal definition
- Terms that have both a common meaning and a domain-specific meaning clearly distinguish the two

**DO NOT** define terms with circular references ("task: a unit of work in the task system"). **DO NOT** assume common-English meaning suffices for domain terms — LLMs default to the most common meaning. (Validates INV-009, INV-017.)

**Anti-pattern**: Defining "task" as "a unit of work." Define it as "a node in the task DAG representing a discrete, assignable unit of implementation work with explicit dependencies, acceptance criteria, and at most one assigned agent at any time (see §7.2, INV-012)."

---

### 7.2 Risk Register

**What it is**: Top 5–10 risks to the project, each with a concrete mitigation.

**Required per risk**:
- Risk description (what could go wrong)
- Impact (what happens if it materializes)
- Mitigation (what we do about it)
- Detection (how we know it's happening)

**DO NOT** list risks without detection methods — a risk you cannot detect materializing is a risk you cannot mitigate in time. (Validates INV-003, INV-017.)

**DO NOT** write mitigations as aspirational statements ("we will monitor closely") — each mitigation must be a specific, actionable procedure. (Validates INV-017.)

**DO NOT** list more than 10 risks — more than 10 means the risk register is a worry list, not a triage tool. Focus on risks that threaten invariants or quality gates. (Validates INV-007, INV-017.)

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
1. [ ] The glossary defines every domain-specific term with a cross-reference to its formal definition (INV-009: every domain-specific term is defined in the glossary)
2. [ ] Glossary definitions distinguish domain-specific meaning from common-English meaning where applicable (INV-009)
3. [ ] The risk register includes detection methods, not just mitigations — how do you know a risk is materializing? (§7.2)
4. [ ] The Master TODO is organized by subsystem and cross-referenced to ADRs and phases (§7.3)
5. [ ] Your appendices do NOT contain circular glossary definitions ("task: a unit of work in the task system") or risks without detection methods
6. [ ] *Integration*: Your glossary defines every term that appears in bold or code formatting across PART II chapters (INV-009)
