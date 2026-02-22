I'll continue from §0.9 and produce the remainder of DDIS 2.0. Picking up exactly where the output was cut:

---

how to write well within the structure.
5. **Anti-Pattern Catalog** (PART III, §8.3) — what bad specs look like.
6. **LLM Consumption Model** (§0.2.3) — how to structure elements for LLM implementers.
7. **Completeness Checklist** (Part X) — mechanical conformance validation.
8. **Specification Error Taxonomy** (Appendix D) — classification of authoring errors.

## 0.10 Open Questions (for DDIS 3.0)

1. **Machine-readable cross-references**: Should DDIS define a syntax for cross-references enabling automated graph construction and stale-restatement detection? (Currently left to author convention.)

2. **Multi-document specs**: For very large systems, how should sub-specs reference each other? What invariants apply across spec boundaries? How do negative specifications compose?

3. **Formal verification bridge**: Should DDIS define a pathway from falsifiable invariants to machine-checked properties for safety-critical systems?

4. **Automated Gate 7 testing**: Can LLM implementation readiness (Gate 7) be automated as a CI check that feeds a spec chapter to an LLM and validates the output?

---

# PART I: FOUNDATIONS

## Chapter 1: The Formal Model of a Specification

### 1.1 A Specification as a State Machine

A specification is itself a stateful artifact that transitions through well-defined phases:

```
States:
  Skeleton    — Structure exists but sections are empty
  Drafted     — All sections have initial content
  Threaded    — Cross-references connect all sections
  Gated       — Quality gates pass
  Validated   — External implementer confirms readiness (Gate 6 + Gate 7)
  Living      — In use, being updated as implementation reveals gaps

Transitions (with guards):
  Skeleton  →[fill_sections]→     Drafted
    Guard: every required section (§0.3) has non-empty content
  Drafted   →[add_cross_refs]→    Threaded
    Guard: every section is reachable in the reference graph
  Threaded  →[run_gates]→         Gated
    Guard: Gates 1–5 pass; all invariant restatements match source (INV-012)
  Gated     →[external_validate]→ Validated
    Guard: Gates 6–7 pass (human and LLM implementation readiness)
  Validated →[begin_impl]→        Living
    Guard: at least one implementer has confirmed readiness
  Living    →[discover_gap]→      Drafted
    Guard: gap is documented; regression is scoped to affected sections only

Invalid transition policy: Reject and log. A transition that skips phases
indicates incomplete specification work. The reviewer (human or automated)
must reject the transition and indicate which prerequisite phase is incomplete.

  Skeleton → Gated:    INVALID — cannot pass gates with empty sections
  Skeleton → Validated: INVALID — cannot validate without content
  Drafted → Validated:  INVALID — cannot validate without cross-references
  Drafted → Gated:     INVALID — unthreaded specs cannot pass Gate 5
  Living → Skeleton:   INVALID — cannot regress past Drafted; gaps are patches
```

### 1.2 Completeness Properties

A complete specification satisfies two properties:

**Safety**: The spec never prescribes contradictory behavior.
```
∀ section_a, section_b ∈ spec:
  ¬(section_a.prescribes(behavior_X) ∧ section_b.prescribes(¬behavior_X))
```

**Liveness**: The spec eventually answers every architectural question an implementer will ask.
```
∀ question Q where Q.is_architectural:
  ◇(spec.answers(Q))  // "eventually" means by Validated state
```

**Negative completeness** (new in 2.0): The spec explicitly excludes the most plausible misinterpretations.
```
∀ subsystem S, ∀ misinterpretation M where M.is_plausible:
  spec.explicitly_excludes(M) ∨ spec.unambiguously_prevents(M)
```

### 1.3 Complexity of Specification Elements

| Element | Authoring Complexity | Reading Complexity | Verification Complexity |
|---|---|---|---|
| Invariant | O(domain_understanding) | O(1) per invariant | O(1) per invariant |
| ADR | O(alternatives × analysis_depth) | O(alternatives) per ADR | O(1) per ADR |
| Algorithm | O(algorithm_complexity × edge_cases) | O(pseudocode_length) | O(worked_examples) |
| Cross-reference | O(1) per reference | O(1) per reference | O(sections²) for full graph |
| End-to-end trace | O(subsystems × interactions) | O(subsystems) | O(1) (follow the trace) |
| Negative specification | O(domain_understanding) | O(1) per constraint | O(1) (check plausibility) |
| Verification prompt | O(invariants_per_chapter) | O(1) per chapter | O(1) (run the prompt) |

### 1.4 End-to-End Trace (of DDIS Itself)

This trace demonstrates DDIS coherence by following one element — an ADR — from the author's initial recognition of a decision through the DDIS authoring process to its final validated form.

**Scenario**: An author writing a domain spec (an event-sourced task scheduler) recognizes a decision: "Should the kernel loop be single-threaded or multi-threaded?"

**Step 1 — Recognition** (INV-002: Decision Completeness). The author realizes two reasonable alternatives exist. Per INV-002 (every choice where a reasonable alternative exists must be captured), this requires an ADR.

**Step 2 — Formal model check** (§3.3). The author's first-principles model defines `Reducer: (State, Event) → State` with a determinism invariant. Both single-threaded and multi-threaded approaches are compatible with the formal model, confirming this is a genuine decision.

**Step 3 — ADR authoring** (§3.5). Following the required format:
- Problem: kernel concurrency model
- Options: (A) Single-threaded — serialized events, deterministic replay, no locking. (B) Multi-threaded with locks — higher throughput, complex reasoning, replay requires lock ordering. (C) Actor model — message passing, natural for agents, higher latency per event.
- Decision: (A) Single-threaded, citing the determinism invariant.
- WHY NOT (B)? Lock ordering makes replay non-trivial; replay is a non-negotiable.
- Consequences: throughput capped at single-core speed; sufficient at design point of 300 agents.

**Step 4 — Cross-reference web** (INV-006, INV-012). The author adds references:
- From ADR → INV-003 (determinism: same events → identical state) — substance restated
- From ADR → the kernel implementation chapter
- From the kernel chapter → ADR (with substance: "single-threaded by ADR-003 for deterministic replay")

**Step 5 — Negative specification** (INV-011). The kernel chapter states: "The kernel must NOT spawn threads for event processing. Must NOT read wall-clock time during reduction. Must NOT acquire locks in the event loop."

**Step 6 — Verification prompt** (INV-013). The kernel chapter ends with: "Verify: Is your event loop single-threaded? Does your reducer avoid wall-clock reads? Can you replay 10K events and get byte-identical state?"

**Step 7 — Quality gate validation**:
- Gate 2: kernel chapter → ADR-003 → INV-003 → formal model ✓
- Gate 3: reviewer finds no obvious alternative not in ADR-003 ✓
- Gate 5: kernel chapter has inbound and outbound references with substance ✓
- Gate 7: an LLM given only the kernel chapter + glossary + restated invariants identifies the single-threaded constraint, lists the negative specs, and produces a single-threaded event loop ✓

This trace exercises: element specs (§3.5, §3.8, §5.6), invariants (INV-002, INV-006, INV-011, INV-012, INV-013), quality gates (2, 3, 5, 7), and the cross-reference web — demonstrating that the pieces compose into a coherent authoring process.

---

# PART II: CORE STANDARD — ELEMENT SPECIFICATIONS

## Chapter 2: Preamble Elements

### 2.1 Design Goal

**What it is**: A single sentence (≤ 30 words) stating the system's reason for existing.

**Required properties**: States core value proposition, not implementation. Uses bold for the 3–5 key properties. Readable by a non-technical stakeholder.

**Quality criteria**: A reader seeing only the design goal can decide whether this system is relevant. An LLM reading only the design goal can determine the system's domain and optimization targets.

**Anti-pattern**: "Build a distributed task coordination system using event sourcing and advisory reservations." ← Describes implementation, not value.

**Good example**: "Design goal: **scrollback-native, zero-flicker, agent-ergonomic, and high-performance** Rust terminal apps."

**Cross-references**: Each bolded property should correspond to at least one invariant (§3.4) and one quality gate (§3.6).

### 2.2 Core Promise

**What it is**: A single sentence (≤ 40 words) describing what the system makes possible, from the user's perspective.

**Required properties**: Written from the user's viewpoint. States concrete capabilities. Uses "without" clauses to highlight what would normally be sacrificed.

**Quality criteria**: A potential user reading only this sentence understands what the system gives them and what it doesn't cost them. The "without" clauses serve as implicit negative specifications (§3.8) — they constrain what the system must NOT sacrifice.

**Anti-pattern**: "The system provides robust, scalable, enterprise-grade coordination." ← Meaningless buzzwords that an LLM will reproduce as meaningless boilerplate.

### 2.3 Document Note

**What it is**: A short disclaimer (2–4 sentences) about code blocks and where the correctness contract lives.

**Why it exists**: Without this note, implementers (especially LLMs) treat code blocks as copy-paste targets. LLMs over-index on examples (§0.2.3) — a code block with a subtle error will be faithfully reproduced.

**Template**:
> Code blocks in this plan are **design sketches** for API shape, invariants, and responsibilities.
> They are intentionally "close to [language]," but not guaranteed to compile verbatim.
> The correctness contract lives in the invariants, tests, and ADRs — not in pseudo-code syntax.
> LLM implementers: treat invariants as ground truth. When a code sketch contradicts an invariant, the invariant wins.

### 2.4 How to Use This Plan

**What it is**: A numbered list (4–7 items) giving practical reading and execution guidance.

**Required properties**: Starts with "Read PART 0 end-to-end." Identifies churn-magnets to lock via ADRs. Points to the Master TODO. Includes at least one note about LLM consumption (e.g., "each implementation chapter is self-contained — process independently if needed").

---

## Chapter 3: PART 0 Elements

### 3.1 Non-Negotiables (Engineering Contract)

**What it is**: 5–10 properties that define what the system IS. Stronger than invariants — they are philosophical commitments.

**Required format**:
```
- **[Property name in bold]**
  [One sentence explaining what this means concretely]
```

**Quality criteria**: An implementer could imagine a situation where violating it would be tempting, and the non-negotiable clearly says: no, even then. Must NOT be a restatement of a type constraint the compiler already enforces.

**Relationship to invariants**: Non-negotiables are the "why" that justifies groups of invariants. "Deterministic replay is real" (non-negotiable) justifies INV-003: "Same events → identical state" (invariant).

### 3.2 Non-Goals

**What it is**: 5–10 things the system explicitly does NOT attempt.

**Why it exists**: Non-goals are the immune system against scope creep. They give implementers permission to say "out of scope."

**Quality criteria**: Someone has actually asked for this (or will). Briefly explains why it's excluded.

**Anti-pattern**: "Non-goal: Building a quantum computer." ← Nobody asked. Non-goals exclude things that are tempting, not absurd.

**LLM note**: Non-goals prevent LLMs from adding "helpful" features not in the spec. An LLM that sees a task scheduler may add a task priority queue even if priorities are explicitly a non-goal. Non-goals must be phrased as direct prohibitions: "This system does NOT implement task priorities."

### 3.3 First-Principles Derivation

**What it is**: The formal model from which the entire architecture derives.

**Required components**:
1. **"What IS a [System]?"** — A mathematical definition establishing the system as a formally defined function or state machine.
2. **Consequences** — 3–5 bullets explaining what the definition implies. Each should feel like a discovery, not an assertion.
3. **Fundamental Operations Table** — Every primitive operation with its model and complexity target.

**Quality criteria**: After reading this section, an implementer should be able to derive the architecture independently. If the architecture is a surprise after reading the first principles, the derivation is incomplete.

**Relationship to other elements**: Referenced by every invariant (constrains model states), every ADR (decides between alternatives within the model), and every algorithm (implements model transitions).

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

**Quality criteria**:
- **Falsifiable**: You can construct a concrete counterexample (INV-003)
- **Consequential**: Violating it causes observable bad behavior
- **Non-trivial**: Not a tautology or compiler-enforced type constraint
- **Testable**: Validation method is specific enough to implement
- **LLM-parseable**: Semi-formal expression uses notation an LLM can interpret without specialized training; violation scenario is concrete enough that an LLM can use it as a negative test case

**Quantity guidance**: 10–25 invariants for medium complexity. Fewer suggests under-specification. More suggests invariants are too granular.

**Anti-patterns**:
```
❌ BAD: "INV-001: The system shall be performant."
  — Not falsifiable, no violation scenario, not testable

❌ BAD: "INV-002: TaskId values are unique."
  — Trivially enforced by the type system

✅ GOOD: "INV-003: Event Log Determinism
  Same event sequence applied to same initial state produces identical final state.
  ∀ events, ∀ state₀: reduce(state₀, events) = reduce(state₀, events)
  Violation: A reducer reads wall-clock time, causing different states on replay.
  Validation: Replay test — process 10K events, snapshot, replay, byte-compare.
  // WHY THIS MATTERS: If replay diverges, we lose auditability and debugging."
```

**Must NOT**: Invariants must NOT use subjective terms ("fast," "reliable," "secure") without quantification. Must NOT reference implementation details that could change — reference the formal model instead.

### 3.5 Architecture Decision Records (ADRs)

**What it is**: A record of each significant design decision, including rejected alternatives and why.

**Required format per ADR**:
```
### ADR-NNN: [Descriptive Title]

#### Problem
[1–3 sentences describing the decision]

#### Options
A) **[Option name]**
- Pros: [concrete advantages]
- Cons: [concrete disadvantages]
[At least 2 options, at most 4]

#### Decision
**[Chosen option]**: [Rationale in 2–5 sentences]
// WHY NOT [rejected option]? [Brief explanation]

#### Consequences
[2–4 bullet points]

#### Tests
[How we verify this decision was correct]
```

**Quality criteria**:
- **Genuine alternatives**: Each option has a real advocate. A competent engineer in a different context would reasonably choose each rejected option.
- **Concrete tradeoffs**: Pros and cons cite specific, measurable properties.
- **Consequential decision**: Swapping options would require > 1 day of refactoring.
- **Self-contained**: An LLM reading only this ADR (without preceding sections) can understand the problem, evaluate the options, and follow the rationale. Per INV-012 (structural redundancy), each ADR restates the invariants it references.

**Anti-pattern — The Strawman ADR**:
```
❌ Options:
  A) Our chosen approach (clearly the best)
  B) A terrible approach nobody would choose
  Decision: A, obviously.
```

**Must NOT**: ADRs must NOT assume the reader has read preceding sections. Must NOT use "as mentioned above" — restate or cite explicitly.

### 3.6 Quality Gates

**What it is**: 4–8 stop-ship criteria, ordered by priority.

**Required properties**: Each gate is a predicate (passing or failing), references specific invariants or tests, and gates are ordered so a failing Gate N makes Gate N+1 irrelevant.

**Quality criteria**: A project manager can assess gate status in < 30 minutes.

**Must NOT**: Gates must NOT be subjective ("the code is clean enough"). Every gate must have a mechanical or reproducible assessment method.

### 3.7 Performance Budgets and Design Point

**What it is**: A table of performance targets anchored to a specific design point.

**Required components**:
1. **Design point**: Specific scenario (hardware, workload, scale)
2. **Budget table**: Operation → target → measurement method
3. **Measurement harness**: How to run the benchmarks
4. **Adjustment guidance**: How to adapt for different design points

**Must NOT**: Performance sections must NOT use qualitative terms ("fast," "real-time," "responsive") without quantification. An LLM will interpret these literally and produce untested claims.

### 3.8 Negative Specifications

**What it is**: Explicit "must NOT" constraints for each implementation subsystem, preventing the most plausible misinterpretations and LLM hallucinations.

**Why it exists**: Positive specifications describe what to build. Negative specifications describe what NOT to build. The gap between these two is where LLMs hallucinate — they fill unspecified space with plausible behavior from their training data. Negative specifications close this gap for the highest-risk areas. (Justified by LLM Consumption Model §0.2.3; locked by ADR-006; validated by INV-011.)

**Required format**:
```
**Negative Specifications for [Subsystem Name]:**
- Must NOT [constraint]. [One sentence explaining why / what goes wrong if violated.]
- Must NOT [constraint]. [Explanation.]
[Minimum 1 per implementation chapter; typically 3–5]
```

**Quality criteria per negative specification**:
- **Plausible**: The prohibited behavior is something a competent implementer (or LLM) might reasonably do absent the constraint. "Must NOT format the hard drive" is not plausible for a scheduler.
- **Specific**: Names a concrete behavior, not a vague quality. "Must NOT be slow" is not a negative specification.
- **Justified**: Briefly explains why this is prohibited — what goes wrong.
- **Non-redundant with invariants**: If a negative specification is just the negation of an existing invariant, it should reference the invariant rather than re-derive it.

**Worked example**:
```
Negative Specifications for the Event Reducer:
- Must NOT read wall-clock time during event processing. (Breaks
  deterministic replay — INV-003: same events → identical state.)
- Must NOT spawn threads or async tasks from within the reduce function.
  (Serialized processing is the mechanism for replay; see ADR-003.)
- Must NOT modify global state outside the reducer's owned State struct.
  (Side effects outside State make snapshots incomplete.)
- Must NOT silently drop malformed events. (Every event must either be
  processed or produce an explicit error entry in the event log.)
```

**Anti-pattern**:
```
❌ BAD: "Must NOT be insecure." — Vague, not actionable.
❌ BAD: "Must NOT use recursion." — Not plausible as a misinterpretation;
  this is a micro-optimization choice, not a spec-level constraint.
✅ GOOD: "Must NOT cache query results across user boundaries." — Plausible
  (global caches are common), specific, and the consequence is clear (data leak).
```

---

## Chapter 4: PART I Elements

### 4.1 Full Formal Model

**What it is**: The expanded version of the first-principles derivation. Complete state definition, input/event taxonomy, output/effect taxonomy, state transition semantics, and composition rules.

### 4.2 State Machines

**What it is**: Every stateful component gets a formal state machine.

**Required per state machine**: State diagram (ASCII or description), state × event table (every combination covered), guard conditions, invalid transition policy, entry/exit actions.

**Quality criteria**: No empty cells in the state × event table. Every cell names a transition or explicitly says "invalid — [policy]." LLMs implementing from a state machine must NOT need to infer any transition — all are explicit.

### 4.3 Complexity Analysis

**What it is**: Complexity bounds for every fundamental operation. Big-O with constants where relevant: "O(n) where n = active_agents, expected ≤ 300" is more useful than "O(n)."

---

## Chapter 5: PART II Elements

### 5.1 Implementation Chapters

**What it is**: One chapter per major subsystem. This is where the spec earns its value.

**Required components per chapter**:

1. **Purpose statement** (2–3 sentences): What this subsystem does and why. References the formal model.
2. **Formal types**: Data structures with memory layout analysis where relevant. `// WHY NOT` annotations on non-obvious choices.
3. **Algorithm pseudocode**: Every non-trivial algorithm with inline complexity analysis.
4. **State machine** (if stateful): Full state machine per §4.2.
5. **Invariants preserved**: Which INV-NNN this subsystem maintains — **with substance restated** (per INV-012), not ID-only. Format: "Preserves INV-003 (same event sequence → identical final state)."
6. **Negative specifications**: What this subsystem must NOT do (per §3.8, INV-011). Minimum 1, typically 3–5.
7. **Worked example(s)**: At least one concrete scenario with specific values, not variables.
8. **Edge cases and error handling**: What happens with malformed inputs, resource exhaustion, or threatened invariants.
9. **Test strategy**: Unit, property, integration, replay, stress — as applicable.
10. **Performance budget**: This subsystem's share of the overall budget.
11. **Cross-references**: To ADRs, invariants, other subsystems — all with substance restated.
12. **Meta-instructions**: Implementation ordering guidance: what to build first, what depends on what, and why (per §5.7, INV-014).
13. **Verification prompt**: Self-check for the implementer (per §5.6, INV-013).

**Quality criteria**: An implementer could build this subsystem from this chapter alone (plus the glossary and restated invariants). For LLMs: the chapter is self-contained — processing it in isolation (without the rest of the spec) produces a correct implementation.

### 5.2 Worked Examples

**What it is**: A concrete scenario with specific values showing the subsystem processing a realistic input.

**Required properties**: Uses concrete values (`task_id = T-042`, not "some task"). Shows state before, the operation, and state after. Includes at least one non-trivial aspect. LLMs over-index on examples — every example must be correct and representative. A misleading example is actively harmful.

**Anti-pattern**:
```
❌ BAD: "When a task is completed, the scheduler updates the DAG."
  — No values. No before/after. No edge case.

✅ GOOD:
  "Agent A-007 completes task T-042 (Implement login endpoint).
  Before: T-042 status=InProgress, T-043 depends on [T-042, T-041], T-041 status=Done
  Operation: TaskCompleted { task_id: T-042, agent_id: A-007, artifacts: [login.rs] }
  After: T-042 status=Done, T-043 status=Ready (all deps met), T-043 enters scheduling queue
  Edge case: If T-043 was Cancelled while T-042 was in progress, T-043 remains Cancelled —
  completion of a dependency does NOT resurrect a cancelled task."
```

### 5.3 End-to-End Trace

**What it is**: A single scenario traversing ALL major subsystems, showing data at each boundary.

**Required properties**: Traces one event from ingestion through every subsystem to final output. Shows exact data at each boundary. Identifies which invariants are exercised at each step. Includes at least one cross-subsystem interaction that could go wrong.

**Why it exists**: Individual examples prove each piece works. The trace proves pieces fit together. Many bugs live at subsystem boundaries. (Validates INV-001.)

### 5.4 WHY NOT Annotations

**What it is**: Inline comments explaining the road not taken.

**When to use**: Whenever an implementer might think "I can improve this by doing X instead," and X was considered and rejected.

**Format**: `// WHY NOT [alternative]? [Tradeoff. Reference ADR-NNN if one exists.]`

If a WHY NOT annotation grows beyond 3 lines, it should become an ADR.

### 5.5 Comparison Blocks

**What it is**: Side-by-side ❌ SUBOPTIMAL vs ✅ CHOSEN comparisons with quantified reasoning.

**Format**:
```
// ❌ SUBOPTIMAL: [Rejected approach]
//   - [Quantified downside]
// ✅ CHOSEN: [Selected approach]
//   - [Quantified advantage]
//   - See ADR-NNN for full analysis
```

### 5.6 Verification Prompts

**What it is**: A structured self-check at the end of each implementation chapter that an implementer (human or LLM) uses to verify their output against the spec.

**Why it exists**: LLMs can follow explicit self-verification instructions. Verification prompts convert passive specification into active self-checking, catching errors during generation rather than at review. (Justified by LLM Consumption Model §0.2.3; locked by ADR-008; validated by INV-013.)

**Required format**:
```
**Verification prompt for [Subsystem]:**
1. Does your implementation preserve [INV-NNN]: [restated substance]?
2. Does your implementation preserve [INV-NNN]: [restated substance]?
3. Does your implementation avoid [negative spec 1]?
4. Does your implementation avoid [negative spec 2]?
5. Can you trace your implementation back to [ADR-NNN]?
6. [Domain-specific check: e.g., "Run the worked example above through
   your implementation. Does the output match?"]
```

**Quality criteria**:
- References specific invariants with substance restated (INV-012)
- References specific negative specifications from the same chapter (INV-011)
- Includes at least one concrete verification step (e.g., running the worked example)
- Answerable by an LLM examining its own generated code

**Worked example** (for a task scheduler chapter):
```
Verification prompt for the Task Scheduler:
1. Does your implementation preserve INV-003 (same events → identical state)?
   Specifically: does your scheduler use wall-clock time? If yes, replay will diverge.
2. Does your implementation preserve INV-007 (no starvation for > 1000 ticks)?
   Check: can you construct a scenario where a low-priority task is never scheduled?
3. Does your implementation avoid spawning threads for event processing? (Negative spec #1)
4. Does your implementation avoid caching scheduling decisions across reducer calls? (Negative spec #2)
5. Run the worked example: Agent A-007 completes T-042. Does T-043 transition to Ready?
   Does a cancelled T-043 remain Cancelled?
```

**Must NOT**: Verification prompts must NOT be generic checklists ("is the code clean?"). Every item must reference a specific invariant, negative spec, or worked example from the same chapter.

### 5.7 Meta-Instructions

**What it is**: Explicit directives to the implementer about implementation strategy, ordering, preconditions, and non-obvious approaches — guidance that is invisible to compilers but valuable to human and LLM implementers.

**Why it exists**: LLMs implement in whatever order they encounter content. Without explicit ordering, an LLM may implement a dependent subsystem before its dependency, creating integration failures. Meta-instructions make implementation strategy explicit. (Justified by LLM Consumption Model §0.2.3; validated by INV-014, INV-015.)

**Required format**:
```
**Meta-instructions for [Subsystem]:**
- Implement [X] before [Y] because [Y depends on X's interface for Z].
- When implementing [algorithm], start with [simple case] and extend to [general case].
- Do NOT optimize [hot path] until benchmarks confirm it is the bottleneck.
  [Premature optimization here risks breaking INV-NNN: restated substance.]
```

**Quality criteria**:
- Every ordering directive includes a rationale (not just "do X first" but "because Y depends on X")
- Strategy directives are actionable (not "be careful" but "start with the happy path, then handle each error case from the error taxonomy")
- References invariants or ADRs where relevant

**Worked example** (for a storage layer chapter):
```
Meta-instructions for the Storage Layer:
- Implement the event log writer before the snapshot mechanism, because
  snapshots are validated by replaying the event log (INV-003: determinism).
- Implement the read path as a pure function of snapshot + event suffix.
  Do NOT add caching until Benchmark B-003 confirms read latency exceeds budget.
- Start with a single-file event log. The sharding strategy (ADR-007: single-file
  chosen over sharded, citing design point of < 1M events) is a later concern.
```

**Must NOT**: Meta-instructions must NOT contradict the spec's content. Must NOT be vague ("implement carefully"). Must NOT prescribe micro-level details (variable names, indentation) — only macro-level strategy.

---

## Chapter 6: PART IV Elements

### 6.1 Operational Playbook

**What it is**: Prevents infinite refinement without shipping.

#### 6.1.1 Phase -1: Decision Spikes

Before building anything, run experiments that de-risk the hardest unknowns. Each spike produces an ADR. Required per spike: question it answers, time budget (1–3 days), exit criterion (one ADR).

#### 6.1.2 Exit Criteria per Phase

Every phase has a specific, testable exit criterion.

**Anti-pattern**: "Phase 2: Implement the scheduler. Exit: Scheduler works."
**Good example**: "Phase 2 exit: Property test demonstrates fair scheduling across 50 agents with no starvation for > 1000 ticks. Benchmark shows dispatch < 1ms at design point."

#### 6.1.3 Merge Discipline

What every PR touching invariants or critical paths must include: tests, a note on which invariants it preserves (INV-012 applies to PR descriptions too), benchmark comparison if touching a hot path.

#### 6.1.4 Minimal Deliverables Order

The order in which subsystems should be built, chosen to maximize the "working subset" at each stage. Must include dependency rationale — why this order, not another. This is the spec-level expression of INV-015 (implementation ordering explicitness).

#### 6.1.5 Immediate Next Steps (First PRs)

The literal first 5–6 things to implement, in dependency order. Tactical, not strategic. Converts the spec from "a plan to study" into "a plan to execute now."

### 6.2 Testing Strategy

**What it is**: A taxonomy of test types with examples and guidance.

| Test Type | What It Validates | Example |
|---|---|---|
| Unit | Individual function correctness | Conflict detection returns correct overlaps |
| Property | Invariant preservation under random inputs | ∀ events: replay(snapshot, events) = direct_state |
| Integration | Subsystem composition | Completed task triggers scheduling cascade |
| Stress | Behavior at design point limits | 300 agents, 10K tasks, sustained 60s |
| Replay | Determinism | Process N events, snapshot, replay, byte-compare |
| Adversarial | Robustness against malformed input | Agent sends event with forged task_id |
| LLM Output | Implementation matches spec | Verification prompt (§5.6) run against generated code |

### 6.3 Error Taxonomy

**What it is**: Classification of errors with handling strategy per class.

**Required**: Each error class has severity (fatal/degraded/recoverable/ignorable), handling strategy (crash/retry/degrade/log), and cross-references to threatened invariants.

See also Appendix D for the specification authoring error taxonomy (meta-level errors in the spec itself, not runtime errors in the system).

---

## Chapter 7: Appendix Elements

### 7.1 Glossary

**What it is**: Every domain-specific term, defined in 1–3 sentences with a cross-reference to where it's formally specified.

**Required properties**: Alphabetized. Each entry includes `(see §X.Y)` pointing to the formal definition. Terms with both common and domain-specific meanings clearly distinguish the two.

**Anti-pattern**: Defining "task" as "a unit of work." Define it as "a node in the task DAG representing a discrete, assignable unit of implementation work with explicit dependencies, acceptance criteria, and at most one assigned agent at any time (see §7.2, INV-012)."

**LLM note**: The glossary is the primary disambiguation mechanism for LLM consumption (INV-009). Domain terms with common-English meanings are the #1 source of LLM misinterpretation. Every term that could be confused with its common meaning needs a glossary entry.

### 7.2 Risk Register

**What it is**: Top 5–10 risks, each with description, impact, mitigation, and detection.

### 7.3 Master TODO Inventory

**What it is**: Comprehensive, checkboxable task list organized by subsystem, cross-referenced to phases and ADRs.

**Required**: Organized by subsystem. Each item is PR-sized. Cross-references to ADRs/invariants. Checkboxable format (`- [ ]`). Includes tasks for writing negative specifications and verification prompts per subsystem.

---

# PART III: GUIDANCE (RECOMMENDED)

## Chapter 8: Voice and Style

### 8.1 The DDIS Voice

**Technically precise but human.** The voice of a senior engineer explaining their system to a peer they respect.

**Properties**:
- Uses concrete examples, not abstract descriptions
- Admits uncertainty where it exists
- Is direct about tradeoffs
- Does not hedge every statement
- Never uses marketing language ("enterprise-grade," "cutting-edge")
- Never uses bureaucratic language ("it is recommended that," "the system shall")

**LLM-specific voice guidance**: LLMs trained on corporate documentation tend to produce hedging, passive voice, and vague claims. The DDIS voice actively counteracts this. When reviewing LLM-generated spec sections, check for: passive voice ("it was decided" → "we chose"), hedge words ("arguably," "somewhat" → delete), and abstract claims ("provides robust handling" → "retries 3 times with exponential backoff, then returns error E-004").

**Calibration examples**:
```
✅ GOOD: "The kernel loop is single-threaded by design — not because concurrency
is hard, but because serialization through the event log is the mechanism that
gives us deterministic replay for free."

❌ BAD (academic): "The kernel loop utilizes a single-threaded architecture paradigm
to facilitate deterministic replay capabilities."

❌ BAD (casual): "We made the kernel single-threaded and it's awesome!"

❌ BAD (bureaucratic): "It is recommended that the kernel loop shall be implemented
in a single-threaded manner to support the deterministic replay requirement."
```

### 8.2 Formatting Conventions

- **Bold** for terms being defined, non-negotiable properties, critical warnings
- `Code` for types, function names, file names, anything in source code
- `// Comments` for inline justifications and WHY NOT annotations
- Tables for structured data (prefer tables over equivalent prose for LLM consumption)
- ASCII diagrams preferred over external images (spec must be readable in any text editor)
- `Must NOT` in negative specifications always bold and capitalized

### 8.3 Anti-Pattern Catalog

**The Hedge Cascade**:
```
❌ "It might be worth considering the possibility of potentially using a
single-threaded loop, which could arguably provide some benefits..."
✅ "The kernel loop is single-threaded. This gives us deterministic replay.
See ADR-003 for the throughput analysis."
```

**The Orphan Section**: A section that references nothing and is referenced by nothing. Either connect it or remove it. (Violates INV-006.)

**The Trivial Invariant**: "INV-042: The system uses UTF-8 encoding." Either enforced by the platform (not worth an invariant) or so fundamental it belongs in Non-Negotiables.

**The Strawman ADR**: Options where only one is viable. Every option must have a genuine advocate.

**The Percentage-Free Performance Budget**: "The system should respond quickly." Without a number, a design point, and a measurement method, this is a wish.

**The Spec That Requires Oral Tradition**: If an implementer must ask a question the spec should have answered, the spec has a gap. (Violates INV-008.)

**The Implicit Context Reference**: "As discussed above, we use event sourcing." An LLM may not have "above" in context. Cite explicitly: "Per ADR-003 (event sourcing chosen over CRUD for audit trail requirements), we use event sourcing." (Violates INV-012.)

**The Positive-Only Specification**: A chapter that says what to build but never says what NOT to build. LLMs will fill the gap with plausible but incorrect behavior. (Violates INV-011.)

---

## Chapter 9: Proportional Weight Deep Dive

### 9.1 Identifying the Heart

Every system has a "heart" — the 2–3 subsystems where most complexity and most bugs live. These receive 40–50% of PART II's line budget.

**How to identify**: Which subsystems have the most invariants? The most ADRs? The most cross-references? If you cut the spec in half, which subsystems would you keep?

### 9.2 Signals of Imbalanced Weight

- A subsystem with 5 invariants and 50 lines of spec is **starved**
- A subsystem with 1 invariant and 500 lines of spec is **bloated**
- PART 0 longer than PART II means the spec is top-heavy
- Appendices longer than PART II means reference material displaces implementation spec
- A chapter exceeding 500 lines should be split for LLM context window management (§0.8.1)

---

## Chapter 10: Cross-Reference Patterns

### 10.1 Reference Syntax

DDIS recommends consistent conventions with restated substance (per INV-012):

```
(see §3.2)                                     — section reference
(preserves INV-004: every algorithm has pseudocode + examples)  — invariant with substance
(locked by ADR-003: single-threaded for deterministic replay)   — ADR with substance
(measured by Benchmark B-001)                   — performance reference
(defined in Glossary: "task")                   — glossary reference
```

### 10.2 Reference Density Targets

| Section Type | Minimum Outbound References |
|---|---|
| Implementation chapter | 3 (at least: one ADR, one invariant, one other chapter) |
| ADR | 2 (at least: one invariant, one implementation chapter) |
| Invariant | 1 (at least: one test or validation method) |
| Performance budget | 2 (at least: one benchmark, one design point) |
| Test strategy | 2 (at least: one invariant, one implementation chapter) |
| Negative specification | 1 (at least: one invariant or one plausible misinterpretation source) |

---

# PART IV: OPERATIONS

## Chapter 11: Applying DDIS to a New Project

### 11.1 The Authoring Sequence

Write sections in this order (not document order) to minimize rework:

1. **Design goal + Core promise** (articulate the value)
2. **First-principles formal model** (understand the domain)
3. **Non-negotiables** (commit to what matters)
4. **Invariants** (formalize the commitments)
5. **ADRs** (lock controversial decisions)
6. **Implementation chapters** — heaviest subsystems first (the "heart")
7. **Negative specifications** per chapter (think adversarially: what will an LLM get wrong?)
8. **End-to-end trace** (reveals gaps in subsystem interfaces)
9. **Performance budgets** (anchor to measurable targets)
10. **Test strategies** (turn invariants into verification)
11. **Verification prompts** per chapter (convert spec into self-checks)
12. **Meta-instructions** per chapter (make implementation ordering explicit)
13. **Cross-references with substance** (weave the web; restate at point of use)
14. **Glossary** (extract terms from the complete spec)
15. **Master TODO** (convert spec into execution plan)
16. **Operational playbook** (how to start building)

### 11.2 Common Mistakes in First DDIS Specs

1. **Writing implementation chapters before ADRs.** You'll rewrite them when ADRs imply different choices.
2. **Writing the glossary first.** You don't know your terminology until you've written the spec.
3. **Treating the end-to-end trace as optional.** It's the single most effective quality check.
4. **Under-investing in WHY NOT annotations.** Every non-obvious choice needs one.
5. **Skipping negative specifications.** "The LLM will figure it out" is exactly the failure mode negative specs prevent.
6. **Writing ID-only cross-references.** "See INV-003" is useless to an LLM without context. Always restate substance.
7. **Generic verification prompts.** "Check your work" is not a verification prompt. Reference specific invariants and negative specs.
8. **Skipping the anti-patterns.** Show what bad output looks like. LLMs benefit significantly from negative examples.

---

## Chapter 12: Validating a DDIS Specification

### 12.1 Self-Validation Checklist

1. Pick 5 random implementation sections. Trace each backward to the formal model. Any broken chains? (Gate 2)
2. Read each ADR's alternatives. Would a competent engineer genuinely choose any rejected option? (Gate 3)
3. For each invariant, spend 60 seconds constructing a violation scenario. Can't? Too vague or trivially true. (Gate 4)
4. Build the cross-reference graph. Any orphan sections? Do references include substance? (Gate 5)
5. Read the spec as a first-time implementer. Where did you guess? (Gate 6)
6. Pick one implementation chapter. Give it (with glossary and restated invariants) to an LLM. Does the LLM correctly identify invariants, negative specs, and produce a valid skeleton? (Gate 7)
7. Check each negative specification: is the prohibited behavior plausible? (INV-011)
8. Check each verification prompt: does it reference specific invariants and negative specs? (INV-013)

### 12.2 External Validation

Give the spec to an implementer (or LLM) and track:
- Questions they ask that the spec should have answered (→ gaps, INV-008 violation)
- Incorrect implementations the spec didn't prevent (→ missing negative specs, INV-011)
- Hallucinated features not in the spec (→ missing negative specs or non-goals)
- Sections skipped because they couldn't be understood (→ voice/clarity issues)

---

## Chapter 13: Evolving a DDIS Specification

### 13.1 The Living Spec

Once implementation begins, the spec enters the Living state (§1.1). In this state:

- **Gaps discovered during implementation** are patched back into the spec. Track each gap's category using the Specification Error Taxonomy (Appendix D).
- **ADRs may be superseded.** Mark old ADRs as "Superseded by ADR-NNN" and update all cross-references and substance restatements.
- **New invariants may be added** with full INV-NNN format.
- **Negative specifications grow.** Implementation reveals plausible misinterpretations the author didn't anticipate. Add them.
- **Performance budgets may be revised** with documented rationale.

### 13.2 Spec Versioning

`Major.Minor` where:
- **Major** increments when the formal model or a non-negotiable changes
- **Minor** increments when ADRs, invariants, negative specs, or implementation chapters are added or revised

---

# APPENDICES

## Appendix A: Glossary

| Term | Definition |
|---|---|
| **ADR** | Architecture Decision Record. A structured record of a design choice, including alternatives and rationale. (See §3.5) |
| **Causal chain** | The traceable path from a first principle through an invariant and/or ADR to an implementation detail. (See §0.2.2, INV-001) |
| **Churn-magnet** | A decision that, if left open, causes the most downstream rework. ADRs should prioritize locking churn-magnets. (See §3.5) |
| **Comparison block** | A side-by-side ❌/✅ comparison with quantified reasoning. (See §5.5) |
| **Cross-reference** | An explicit link between two sections, forming part of the reference web. In DDIS 2.0, must include substance restated, not just ID. (See Chapter 10, INV-006, INV-012) |
| **DDIS** | Decision-Driven Implementation Specification. This standard. |
| **Decision spike** | A time-boxed experiment that de-risks an unknown and produces an ADR. (See §6.1.1) |
| **Design point** | The specific hardware, workload, and scale scenario against which performance budgets are validated. (See §3.7) |
| **End-to-end trace** | A worked scenario traversing all major subsystems, showing data at each boundary. (See §5.3, §1.4) |
| **Exit criterion** | A specific, testable condition for phase completion. (See §6.1.2) |
| **Falsifiable** | A property of an invariant: can be violated by a concrete scenario and detected by a test. (See INV-003, ADR-002) |
| **First principles** | The formal model from which the architecture derives. (See §3.3) |
| **Formal model** | A mathematical or pseudo-mathematical definition of the system as a state machine or function. (See §0.2.1) |
| **Gate** | A quality gate: a stop-ship predicate. (See §3.6) |
| **Invariant** | A numbered, falsifiable property that must hold at all times. (See §3.4) |
| **Living spec** | A specification in active use, updated as implementation reveals gaps. (See §13.1) |
| **LLM Consumption Model** | The formal model of how LLMs process specifications, justifying structural provisions for LLM effectiveness. (See §0.2.3) |
| **Master TODO** | A checkboxable task inventory cross-referenced to subsystems, phases, and ADRs. (See §7.3) |
| **Meta-instruction** | An explicit directive to the implementer about implementation strategy, ordering, or approach. Invisible to compilers, valuable to LLMs. (See §5.7, INV-014) |
| **Negative specification** | An explicit "must NOT" constraint preventing a plausible misinterpretation. (See §3.8, INV-011) |
| **Non-goal** | Something the system explicitly does not attempt. (See §3.2) |
| **Non-negotiable** | A philosophical commitment defining what the system IS. Stronger than invariants. (See §3.1) |
| **Operational playbook** | How the spec gets converted into shipped software. (See §6.1) |
| **Proportional weight** | Line budget guidance preventing bloat and starvation. (See §0.8.2) |
| **Self-bootstrapping** | A property of this standard: it is written in the format it defines. (See ADR-004) |
| **Structural redundancy** | Restating the substance of referenced constraints at point of use, trading DRY for LLM self-sufficiency per chapter. (See INV-012, ADR-007) |
| **Verification prompt** | A structured self-check at the end of each implementation chapter, referencing specific invariants and negative specs. (See §5.6, INV-013) |
| **Voice** | The writing style prescribed by DDIS: technically precise but human. (See §8.1) |
| **WHY NOT annotation** | Inline comment explaining why a non-obvious alternative was rejected. (See §5.4) |
| **Worked example** | A concrete scenario with specific values showing a subsystem in action. (See §5.2) |

---

## Appendix B: Risk Register

| # | Risk | Impact | Mitigation | Detection |
|---|---|---|---|---|
| 1 | Standard is too prescriptive, authors feel constrained | Low adoption | Non-goals and [Optional] elements provide flexibility | Author feedback |
| 2 | Standard is too verbose, specs become shelfware | Implementers don't read | Proportional weight limits bloat; voice guide keeps prose readable | Track questions that spec should have answered |
| 3 | Cross-reference + restatement requirement is burdensome | Authors skip or restate incorrectly | Authoring sequence defers to step 13; stale restatements are an auditable error class (Appendix D) | Reference graph analysis; automated stale-check |
| 4 | Self-bootstrapping creates circular confusion | Readers can't distinguish meta/object level | Document note and consistent "this standard" vs "a conforming specification" language | Reader feedback |
| 5 | Negative specifications become trivial boilerplate | Authors write "must NOT format the hard drive" | Quality criteria require plausible misinterpretations; Gate 7 validates | Review negative specs for plausibility |
| 6 | LLM provisions increase spec length without proportional value | Longer specs, diminishing returns | Each provision has a specific failure mode it prevents (§0.2.2 table); proportional weight enforced | Measure LLM implementation accuracy with and without provisions |
| 7 | Verification prompts become generic checklists | Self-checks don't catch errors | Quality criteria require specific invariant and negative spec references | Gate 7 tests whether LLM actually catches errors using the prompt |

---

## Appendix C: Quick-Reference Card

```
PREAMBLE: Design goal → Core promise → Document note → How to use
PART 0:   Summary → First principles + LLM consumption model → Architecture → Layout →
          Invariants → ADRs → Gates (1-7) → Budgets → API → Non-negotiables → Non-goals
PART I:   Formal model → State machines → Complexity → End-to-end trace
PART II:  [Per subsystem: types → algorithm → state machine → invariants (restated) →
          negative specs → example → WHY NOT → tests → budget → cross-refs (with substance) →
          meta-instructions → verification prompt]
          End-to-end trace (crosses all subsystems)
PART III: Protocol schemas → Adapters → UI contracts
PART IV:  Test taxonomy → Error taxonomy → Operational playbook
          (spikes → exit criteria → merge discipline → deliverable order → first PRs)
APPENDICES: Glossary → Risks → Formats → Error Taxonomy → Benchmarks
PART X:   Master TODO (checkboxable, by subsystem)

Every invariant: ID + statement + formal + violation + test + WHY THIS MATTERS
Every ADR: problem + options (genuine) + decision + WHY NOT + consequences + tests
Every algorithm: pseudocode + complexity + example + edge cases
Every chapter: negative specs (≥1) + verification prompt + meta-instructions
Cross-refs: web, not list. No orphans. Substance restated, not ID-only.
Voice: senior engineer to respected peer. No hedging. No marketing. No bureaucracy.
LLM: Each chapter self-contained. Negative specs prevent hallucination.
     Verification prompts enable self-check. Meta-instructions order implementation.
```

---

## Appendix D: Specification Error Taxonomy

Classification of errors that occur during specification authoring — the meta-level analog of a system's error taxonomy (§6.3).

| Error Class | Severity | Symptom | Detection | Resolution |
|---|---|---|---|---|
| **Broken causal chain** | Critical | Implementation section with no path to formal model | Gate 2 audit | Add cross-references; if no justification exists, the section may be unjustified |
| **Strawman ADR** | Critical | ADR with no genuine alternative | Gate 3 adversarial review | Research real alternatives or demote to a WHY NOT annotation |
| **Unfalsifiable invariant** | Critical | Invariant with no constructible counterexample | Gate 4 check | Sharpen the invariant or remove it |
| **Orphan section** | Major | Section with no inbound or outbound references | Gate 5 graph analysis | Add references or remove section |
| **Missing negative specification** | Major | Implementation chapter with no "must NOT" constraints | Gate 7 LLM test (hallucinated features indicate missing negatives) | Add plausible negative specs |
| **Stale restatement** | Major | Restated invariant substance no longer matches source | Cross-reference consistency audit | Update restatement to match source |
| **ID-only reference** | Moderate | Cross-reference with identifier but no substance | INV-012 audit | Add one-sentence substance |
| **Generic verification prompt** | Moderate | Prompt says "check your work" without specific invariant refs | INV-013 audit | Reference specific invariants and negative specs |
| **Implicit context dependency** | Moderate | Section uses "as discussed above" or assumes prior reading | LLM chapter-isolation test | Replace with explicit reference + substance |
| **Missing meta-instruction** | Minor | Chapter with ordering dependencies but no explicit guidance | INV-014 check via cross-ref analysis | Add meta-instructions with rationale |
| **Vague non-goal** | Minor | Non-goal says "out of scope" without explaining why | Review | Add brief rationale |
| **Trivial negative spec** | Minor | "Must NOT format the hard drive" — not a plausible misinterpretation | Plausibility review | Replace with plausible constraint or remove |

---

# PART X: MASTER TODO INVENTORY

## A) Meta-Standard Validation
- [x] Self-bootstrapping: this document uses the format it defines
- [x] Preamble elements: design goal, core promise, document note, how to use — all updated for LLM-first focus
- [x] Non-negotiables defined (§0.1.2) — includes LLM implementation non-negotiable
- [x] Non-goals defined (§0.1.3)
- [x] First-principles derivation (§0.2)
- [x] LLM Consumption Model (§0.2.3) — justifies INV-011 through INV-015 and ADR-006 through ADR-009
- [x] Document structure prescribed (§0.3) — updated to include negative specs, verification prompts, meta-instructions
- [x] Invariants numbered and falsifiable (§0.5, INV-001 through INV-015)
- [x] INV-011 (Negative Specification Coverage): full format with violation scenario, validation method
- [x] INV-012 (Structural Redundancy): full format with violation scenario, validation method
- [x] INV-013 (Verification Prompt Coverage): full format with violation scenario, validation method
- [x] INV-014 (Meta-Instruction Explicitness): full format with violation scenario, validation method
- [x] INV-015 (Implementation Ordering): full format with violation scenario, validation method
- [x] ADRs with genuine alternatives (§0.6, ADR-001 through ADR-009)
- [x] ADR-006 (Negative Specs Required): 3 genuine options, concrete tradeoffs, WHY NOT annotations
- [x] ADR-007 (Structural Redundancy over DRY): 3 genuine options, concrete tradeoffs
- [x] ADR-008 (Verification Prompts Required): 3 genuine options, concrete tradeoffs
- [x] ADR-009 (LLM Provisions Woven Throughout): 3 genuine options, concrete tradeoffs
- [x] Quality gates defined (§0.7) — Gates 1 through 7, including Gate 7 (LLM Implementation Readiness)
- [x] Gate 7 operational with concrete test procedure, pass/fail criteria, and specific thresholds
- [x] Performance budgets (§0.8) — includes LLM context window guidance
- [x] Proportional weight guide (§0.8.2)

## B) Element Specifications
- [x] Preamble elements specified (Chapter 2)
- [x] PART 0 elements specified (Chapter 3)
- [x] Negative Specifications element spec (§3.8) — with format, quality criteria, worked example, anti-patterns
- [x] PART I elements specified (Chapter 4)
- [x] PART II elements specified (Chapter 5)
- [x] Verification Prompts element spec (§5.6) — with format, quality criteria, worked example
- [x] Meta-Instructions element spec (§5.7) — with format, quality criteria, worked example
- [x] PART IV elements specified (Chapter 6)
- [x] Appendix elements specified (Chapter 7)
- [x] Anti-pattern catalog (§8.3) — updated with LLM-specific anti-patterns
- [x] Cross-reference patterns (Chapter 10) — updated with substance-restated syntax

## C) Guidance
- [x] Voice and style guide (Chapter 8) — includes LLM-specific voice guidance
- [x] Proportional weight deep dive (Chapter 9)
- [x] Authoring sequence (§11.1) — updated with negative specs, verification prompts, meta-instructions steps
- [x] Common mistakes (§11.2) — updated with LLM-specific mistakes
- [x] Validation procedure (Chapter 12) — includes Gate 7 validation
- [x] Evolution guidance (Chapter 13)

## D) Reference Material
- [x] Glossary (Appendix A) — expanded with new terms (LLM Consumption Model, meta-instruction, negative specification, structural redundancy, verification prompt)
- [x] Risk register (Appendix B) — expanded with LLM-provision risks
- [x] Quick-reference card (Appendix C) — updated for DDIS 2.0
- [x] Specification Error Taxonomy (Appendix D) — new, classifies authoring errors

## E) Self-Conformance Fixes from 1.0
- [x] State machine (§1.1) completed with guards, invalid transition matrix, and policy (was incomplete in 1.0, violating INV-010)
- [x] End-to-end trace (§1.4) added (was missing in 1.0 despite being required by §5.3)
- [x] Failure mode table (§0.2.2) includes LLM-specific failure modes
- [x] All new invariants (INV-011–015) have complete format: statement, formal expression, violation scenario, validation method, WHY THIS MATTERS (satisfying INV-003 applied to the new invariants)
- [x] All new ADRs (ADR-006–009) have genuine alternatives with concrete tradeoffs (satisfying INV-002)
- [x] LLM provisions woven throughout PART II element specs, not isolated in a single chapter (validated by ADR-009)

## F) Validation
- [x] INV-001 (Causal Traceability): Every element specification traces to the formal model via the failure mode table (§0.2.2)
- [x] INV-003 (Falsifiability): Each invariant has violation scenario and validation method
- [x] INV-006 (Cross-Reference Density): Sections reference each other throughout
- [x] INV-007 (Signal-to-Noise): Each section serves a named purpose
- [x] INV-011 (Negative Spec Coverage): Demonstrated in §3.8 worked example and throughout element specs ("Must NOT" constraints)
- [x] INV-012 (Structural Redundancy): Demonstrated in §1.4 end-to-end trace and reference syntax (§10.1)
- [x] INV-013 (Verification Prompt Coverage): Demonstrated in §5.6 worked example
- [ ] INV-008 (Self-Containment): Requires external validation — give this standard to a first-time author
- [ ] Gate 6 (Implementation Readiness): Requires a non-trivial spec to be written conforming to DDIS 2.0
- [ ] Gate 7 (LLM Implementation Readiness): Requires an LLM to attempt implementing from a conforming spec chapter

---

## Conclusion

DDIS 2.0 synthesizes techniques from established traditions and adds a new axis: optimization for LLM consumption.

**Retained from 1.0**: Architecture Decision Records (Nygard), Design by Contract (Meyer), temporal invariants (Lamport), performance budgeting, test-driven specification, the causal chain structure, the cross-reference web, WHY NOT annotations, comparison blocks, voice guidance, and self-bootstrapping validation.

**Added in 2.0**: The LLM Consumption Model (§0.2.3) provides the formal justification for five new invariants and four new ADRs that address the primary specification failure mode for modern AI-assisted development: LLM hallucination of unstated requirements. Specifically:

- **Negative specifications** (INV-011, ADR-006, §3.8) close the gap between what the spec says to build and what LLMs might plausibly add.
- **Structural redundancy at point of use** (INV-012, ADR-007) ensures each chapter is self-contained for LLM consumption, trading DRY for reliability.
- **Verification prompts** (INV-013, ADR-008, §5.6) convert passive specs into active self-checks that LLMs can execute during generation.
- **Meta-instructions and implementation ordering** (INV-014, INV-015, §5.7) make implicit implementation strategy explicit for consumers who cannot infer it.
- **Gate 7** (LLM Implementation Readiness) provides an operational test that validates the above provisions work in practice.

These provisions are woven throughout the element specifications (per ADR-009), not isolated in a single chapter. The end-to-end trace (§1.4) demonstrates how all elements — old and new — compose into a coherent authoring and validation process.

The result is a specification standard that is:

- **Decision-driven**: Architecture emerges from locked decisions, not assertions
- **Invariant-anchored**: Correctness is defined before implementation
- **Falsifiable throughout**: Every claim can be tested
- **LLM-optimized**: Every structural element prevents a specific LLM failure mode
- **Self-validating**: Quality gates — including LLM readiness — provide conformance checking
- **Self-bootstrapping**: This document is both the standard and its first conforming instance

*DDIS 2.0: Where rigor meets readability — and specifications become implementations, whether the implementer is human or machine.*
