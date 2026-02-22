
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
  Tested      — Automated spec tests pass (Gate 8)
  Gated       — All quality gates pass (Gates 1–8)
  Validated   — External implementer confirms readiness (Gate 6 + Gate 7)
  Living      — In use, being updated as implementation reveals gaps

Transitions (with guards):
  Skeleton  →[fill_sections]→     Drafted
    Guard: every required section (§0.3) has non-empty content
  Drafted   →[add_cross_refs]→    Threaded
    Guard: every section is reachable in the reference graph;
           cross-references use machine-readable syntax ([[INV-022|parseable refs]])
  Threaded  →[run_spec_tests]→    Tested
    Guard: automated checks pass (Gate 8); all [[ID|substance]] refs resolve;
           proportional weight within tolerance ([[INV-021|weight compliance]])
  Tested    →[run_gates]→         Gated
    Guard: Gates 1–7 pass; all invariant restatements match source ([[INV-018|substance restated]])
  Gated     →[external_validate]→ Validated
    Guard: Gates 6–7 pass (human and LLM implementation readiness)
  Validated →[begin_impl]→        Living
    Guard: at least one implementer has confirmed readiness
  Living    →[discover_gap]→      Drafted
    Guard: gap is documented; regression is scoped to affected sections only

Invalid transition policy: Reject and log. A transition that skips phases
indicates incomplete specification work.

  Skeleton → Gated:     INVALID — cannot pass gates with empty sections
  Skeleton → Validated:  INVALID — cannot validate without content
  Drafted → Validated:   INVALID — cannot validate without cross-references
  Drafted → Gated:      INVALID — unthreaded specs cannot pass Gate 5
  Threaded → Gated:     INVALID — must pass automated tests (Tested) first
  Living → Skeleton:    INVALID — cannot regress past Drafted; gaps are patches
```

// WHY the Tested state (new in 3.0)? Automated spec testing ([[INV-022|parseable cross-refs]], Gate 8) catches structural issues before expensive manual gate reviews. The Tested→Gated transition ensures structural soundness before semantic validation.

### 1.2 Completeness Properties

A complete specification satisfies three properties:

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

**Negative completeness**: The spec explicitly excludes the most plausible misinterpretations.
```
∀ subsystem S, ∀ misinterpretation M where M.is_plausible:
  spec.explicitly_excludes(M) ∨ spec.unambiguously_prevents(M)
```

### 1.3 Complexity of Specification Elements

| Element | Authoring Complexity | Reading Complexity | Verification Complexity |
|---|---|---|---|
| Invariant | O(domain_understanding) | O(1) per invariant | O(1) (construct counterexample) |
| ADR | O(alternatives × analysis_depth) | O(alternatives) per ADR | O(1) (check genuine alternatives) |
| Algorithm | O(algorithm_complexity × edge_cases) | O(pseudocode_length) | O(worked_examples) |
| Cross-reference | O(1) per reference | O(1) per reference | O(1) automated with [[INV-022|parseable refs]] |
| End-to-end trace | O(subsystems × interactions) | O(subsystems) | O(1) (follow the trace) |
| Negative specification | O(domain_understanding) | O(1) per constraint | O(1) (check plausibility) |
| Verification prompt | O(invariants_per_chapter) | O(1) per chapter | O(1) (run the prompt) |

### 1.4 End-to-End Trace (of DDIS Itself)

This trace demonstrates DDIS coherence by following one element — an ADR — from the author's initial recognition of a decision through the DDIS authoring process to its final validated form.

**Scenario**: An author writing a domain spec (an event-sourced task scheduler) recognizes a decision: "Should the kernel loop be single-threaded or multi-threaded?"

**Step 1 — Recognition** ([[INV-002|every choice with reasonable alternative needs an ADR]]). The author realizes two reasonable alternatives exist. Per [[INV-002|decision completeness]], this requires an ADR.

**Step 2 — Formal model check** (§3.3). The author's first-principles model defines `Reducer: (State, Event) → State` with a determinism invariant. Both approaches are compatible with the model, confirming this is a genuine decision.

**Step 3 — ADR authoring** (§3.5). Following the required format:
- Problem: kernel concurrency model
- Confidence: Decided (validated by determinism invariant analysis)
- Options: (A) Single-threaded — serialized events, deterministic replay, no locking. (B) Multi-threaded with locks — higher throughput, complex reasoning, replay requires lock ordering. (C) Actor model — message passing, natural for agents, higher latency per event.
- Decision: (A) Single-threaded, citing the determinism invariant.
- WHY NOT (B)? Lock ordering makes replay non-trivial; replay is a non-negotiable.

**Step 4 — Cross-reference web** ([[INV-006|no orphan sections]], [[INV-018|substance restated]]). The author adds machine-readable references:
- From ADR → [[APP-INV-003|determinism: same events → identical state]]
- From ADR → the kernel implementation chapter
- From the kernel chapter → [[APP-ADR-003|single-threaded for deterministic replay]]

**Step 5 — Negative specification** ([[INV-017|negative spec per chapter]]). The kernel chapter states: "Must NOT spawn threads for event processing. Must NOT read wall-clock time during reduction. Must NOT acquire locks in the event loop."

**Step 6 — Verification prompt** ([[INV-019|self-check per chapter]]). The kernel chapter ends with: "Verify: Is your event loop single-threaded? Does your reducer avoid wall-clock reads? Can you replay 10K events and get byte-identical state?"

**Step 7 — Automated spec testing** ([[INV-022|parseable cross-refs]]). The cross-reference parser validates: all `[[ID|substance]]` references resolve; the kernel chapter has inbound and outbound references; no orphan sections.

**Step 8 — Quality gate validation**:
- Gate 2: kernel chapter → ADR-003 → INV-003 → formal model ✓
- Gate 3: reviewer finds no obvious alternative not in ADR-003 ✓
- Gate 5: kernel chapter has inbound and outbound references with substance ✓
- Gate 7: an LLM given only the kernel chapter + glossary + restated invariants identifies the single-threaded constraint, lists the negative specs, and produces a single-threaded event loop ✓
- Gate 8: automated parser confirms all references resolve and proportional weight is within tolerance ✓

This trace exercises: element specs (§3.5, §3.8, §5.6), invariants ([[INV-002|decision completeness]], [[INV-006|cross-ref density]], [[INV-017|negative specs]], [[INV-018|substance restated]], [[INV-019|verification prompts]], [[INV-022|parseable refs]]), quality gates (2, 3, 5, 7, 8), and the cross-reference web.

---

# PART II: CORE STANDARD — ELEMENT SPECIFICATIONS

This is the heart of DDIS. Each section specifies one structural element: what it must contain, what quality criteria it must meet, how it relates to other elements, and what it looks like when done well versus done badly.

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

**Required properties**: Written from the user's viewpoint. States concrete capabilities. Uses "without" clauses to highlight what would normally be sacrificed. The "without" clauses serve as implicit negative specifications (§3.8) — they constrain what the system must NOT sacrifice.

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

**Required properties**: Starts with "Read PART 0 end-to-end." Identifies churn-magnets to lock via ADRs. Points to the Master TODO. Includes at least one note about LLM consumption (e.g., "each implementation chapter is self-contained — process independently if needed"). If the spec has external dependencies (§0.2.5), lists them.

---

## Chapter 3: PART 0 Elements

### 3.1 Non-Negotiables (Engineering Contract)

**What it is**: 5–10 properties that define what the system IS. Stronger than invariants — they are philosophical commitments.

**Required format**: `- **[Property name]** [One sentence explaining what this means concretely]`

**Quality criteria**: An implementer could imagine a situation where violating it would be tempting, and the non-negotiable clearly says: no, even then. Must NOT be a restatement of a type constraint the compiler already enforces.

**Relationship to invariants**: Non-negotiables are the "why" that justifies groups of invariants. "Deterministic replay is real" (non-negotiable) justifies [[INV-003|same events → identical state]] (invariant).

### 3.2 Non-Goals

**What it is**: 5–10 things the system explicitly does NOT attempt.

**Why it exists**: Non-goals are the immune system against scope creep. They give implementers permission to say "out of scope."

**Quality criteria**: Someone has actually asked for this (or will). Briefly explains why it's excluded.

**Anti-pattern**: "Non-goal: Building a quantum computer." ← Nobody asked. Non-goals exclude things that are tempting, not absurd.

**LLM note**: Non-goals prevent LLMs from adding "helpful" features not in the spec. An LLM that sees a task scheduler may add a priority queue even if priorities are explicitly a non-goal. Phrase as direct prohibitions: "This system does NOT implement task priorities."

### 3.3 First-Principles Derivation

**What it is**: The formal model from which the entire architecture derives.

**Required components**:
1. **"What IS a [System]?"** — A mathematical definition establishing the system as a formally defined function or state machine.
2. **Consequences** — 3–5 bullets explaining what the definition implies. Each should feel like a discovery.
3. **Fundamental Operations Table** — Every primitive operation with its model and complexity target.

**Quality criteria**: After reading this section, an implementer can derive the architecture independently. If the architecture is a surprise after reading first principles, the derivation is incomplete.

**Relationship to other elements**: Referenced by every invariant, every ADR, and every algorithm.

### 3.4 Invariants

**What it is**: A numbered list of properties that must hold at all times.

**Required format per invariant**:
```
**INV-NNN: [Descriptive Name]**
*[Plain-language statement in one sentence]*
  [Semi-formal expression]
Violation scenario: [Concrete description of how this could break]
Validation: [Named test strategy]
// WHY THIS MATTERS: [One sentence on consequences of violation]
```

**Quality criteria**:
- **Falsifiable**: You can construct a concrete counterexample ([[INV-003|invariant falsifiability]])
- **Consequential**: Violating it causes observable bad behavior
- **Non-trivial**: Not a tautology or compiler-enforced type constraint
- **Testable**: Validation method is specific enough to implement
- **LLM-parseable**: Semi-formal expression uses notation an LLM can interpret; violation scenario is concrete enough for a negative test case

**Quantity guidance**: 10–25 invariants for medium complexity.

**Anti-patterns**:
```
❌ BAD: "INV-001: The system shall be performant."
  — Not falsifiable, no violation scenario, not testable

✅ GOOD: "INV-003: Event Log Determinism
  Same event sequence → identical final state.
  ∀ events, ∀ state₀: reduce(state₀, events) = reduce(state₀, events)
  Violation: A reducer reads wall-clock time, causing different states on replay.
  Validation: Replay test — process 10K events, snapshot, replay, byte-compare.
  // WHY THIS MATTERS: If replay diverges, we lose auditability."
```

**Must NOT**: Invariants must NOT use subjective terms ("fast," "reliable") without quantification. Must NOT reference implementation details that could change — reference the formal model instead.

### 3.5 Architecture Decision Records (ADRs)

**What it is**: A record of each significant design decision, including rejected alternatives.

**Required format per ADR**:
```
### ADR-NNN: [Descriptive Title]
**Confidence: Decided | Provisional (review trigger: [concrete trigger])**
#### Problem — [1–3 sentences]
#### Options — [At least 2, at most 4, each with concrete pros/cons]
#### Decision — [Chosen option with rationale]
  // WHY NOT [rejected]? [Brief explanation]
#### Consequences — [2–4 bullet points]
#### Tests — [How we verify this was correct]
```

**Confidence field** (new in 3.0, locked by [[ADR-012|confidence levels on ADRs]]):
- **Decided** — Validated by experience, spike, or strong reasoning. Default for most ADRs.
- **Provisional (review trigger: X)** — Best current judgment but should be revisited. The trigger must be concrete and observable (e.g., "after processing 10K events in production," not "when we know more").

**Quality criteria**:
- **Genuine alternatives**: Each option has a real advocate. A competent engineer in a different context would reasonably choose each rejected option.
- **Concrete tradeoffs**: Cite specific, measurable properties.
- **Consequential decision**: Swapping options would require > 1 day of refactoring.
- **Self-contained**: An LLM reading only this ADR can understand the problem and rationale. Restate referenced invariants per [[INV-018|substance restated at point of use]].

**Anti-pattern — The Strawman ADR**:
```
❌ Options: A) Our chosen approach (clearly the best)
           B) A terrible approach nobody would choose
```

**Must NOT**: ADRs must NOT assume the reader has read preceding sections. Must NOT use "as mentioned above" — restate or cite explicitly.

### 3.6 Quality Gates

**What it is**: 4–8 stop-ship criteria, ordered by priority.

**Required properties**: Each gate is a predicate (passing or failing), references specific invariants or tests, and gates are ordered so a failing Gate N makes Gate N+1 irrelevant.

**Must NOT**: Gates must NOT be subjective ("the code is clean enough"). Every gate must have a mechanical or reproducible assessment method.

### 3.7 Performance Budgets and Design Point

**What it is**: A table of performance targets anchored to a specific design point.

**Required components**:
1. **Design point**: Specific scenario (hardware, workload, scale)
2. **Budget table**: Operation → target → measurement method
3. **Measurement harness**: How to run the benchmarks
4. **Adjustment guidance**: How to adapt for different design points

**Must NOT**: Performance sections must NOT use qualitative terms ("fast," "real-time") without quantification. An LLM will interpret these literally and produce untested claims.

### 3.8 Negative Specifications

**What it is**: Explicit "must NOT" constraints for each implementation subsystem, preventing the most plausible misinterpretations and LLM hallucinations.

**Why it exists**: Positive specifications describe what to build. Negative specifications describe what NOT to build. The gap between these two is where LLMs hallucinate. Negative specifications close this gap for the highest-risk areas. (Justified by §0.2.3; locked by [[ADR-008|negative specs required per chapter]]; validated by [[INV-017|negative spec coverage]].)

**Required format**:
```
**Negative Specifications for [Subsystem Name]:**
- Must NOT [constraint]. [One sentence explaining why / what goes wrong.]
- Must NOT [constraint]. [Explanation.]
[Minimum 1 per implementation chapter; typically 3–5]
```

**Quality criteria per negative specification**:
- **Plausible**: The prohibited behavior is something a competent implementer (or LLM) might reasonably do.
- **Specific**: Names a concrete behavior, not a vague quality.
- **Justified**: Briefly explains what goes wrong.
- **Non-redundant with invariants**: If a negative specification is just the negation of an existing invariant, reference the invariant rather than re-derive it.

**Worked example**:
```
Negative Specifications for the Event Reducer:
- Must NOT read wall-clock time during event processing. (Breaks
  deterministic replay — [[INV-003|same events → identical state]].)
- Must NOT spawn threads or async tasks from within the reduce function.
  (Serialized processing is the mechanism for replay; see [[ADR-003|
  single-threaded for deterministic replay]].)
- Must NOT modify global state outside the reducer's owned State struct.
  (Side effects outside State make snapshots incomplete.)
- Must NOT silently drop malformed events. (Every event must either be
  processed or produce an explicit error entry in the event log.)
```

**Anti-pattern**:
```
❌ BAD: "Must NOT be insecure." — Vague, not actionable.
✅ GOOD: "Must NOT cache query results across user boundaries." — Plausible,
  specific, consequence is clear (data leak).
```

---

## Chapter 4: PART I Elements

### 4.1 Full Formal Model

**What it is**: The expanded version of the first-principles derivation from §0.2. Complete state definition, input/event taxonomy, output/effect taxonomy, state transition semantics, and composition rules.

### 4.2 State Machines

**What it is**: Every stateful component gets a formal state machine.

**Required per state machine**: State diagram (ASCII or description), state × event table (every combination), guard conditions, invalid transition policy, entry/exit actions.

**Quality criteria**: No empty cells in the state × event table. Every cell names a transition or explicitly says "invalid — [policy]." LLMs must NOT need to infer any transition.

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
5. **Invariants preserved**: Which INV-NNN this subsystem maintains — **with substance restated** (per [[INV-018|substance restated at point of use]]). Format: "Preserves [[INV-003|same event sequence → identical final state]]."
6. **Negative specifications**: What this subsystem must NOT do (per §3.8, [[INV-017|negative spec per chapter]]). Minimum 1, typically 3–5.
7. **Worked example(s)**: At least one concrete scenario with specific values.
8. **Edge cases and error handling**: What happens with malformed inputs, resource exhaustion, or threatened invariants.
9. **Test strategy**: Unit, property, integration, replay, stress — as applicable.
10. **Performance budget**: This subsystem's share of the overall budget.
11. **Cross-references**: To ADRs, invariants, other subsystems — all with substance restated, using machine-readable syntax ([[INV-022|parseable refs]]).
12. **Meta-instructions**: Implementation ordering guidance (per §5.7, [[INV-020|explicit sequencing]]).
13. **Verification prompt**: Self-check for the implementer (per §5.6, [[INV-019|self-check per chapter]]).

**Quality criteria**: An implementer could build this subsystem from this chapter alone (plus the glossary and restated invariants). For LLMs: the chapter is self-contained — processing it in isolation produces a correct implementation.

### 5.2 Worked Examples

**What it is**: A concrete scenario with specific values.

**Required properties**: Uses concrete values (`task_id = T-042`, not "some task"). Shows state before, the operation, and state after. Includes at least one non-trivial aspect. LLMs over-index on examples — every example must be correct and representative.

**Anti-pattern**:
```
❌ BAD: "When a task is completed, the scheduler updates the DAG."
  — No values. No before/after. No edge case.

✅ GOOD:
  "Agent A-007 completes task T-042 (Implement login endpoint).
  Before: T-042 status=InProgress, T-043 depends on [T-042, T-041], T-041 status=Done
  Operation: TaskCompleted { task_id: T-042, agent_id: A-007, artifacts: [login.rs] }
  After: T-042 status=Done, T-043 status=Ready (all deps met)
  Edge case: If T-043 was Cancelled while T-042 was in progress, T-043 remains
  Cancelled — completion of a dependency does NOT resurrect a cancelled task."
```

### 5.3 End-to-End Trace

**What it is**: A single scenario traversing ALL major subsystems.

**Required properties**: Traces one event from ingestion through every subsystem to final output. Shows exact data at each boundary. Identifies which invariants are exercised at each step. Includes at least one cross-subsystem interaction that could go wrong.

**Why it exists**: Individual examples prove each piece works. The trace proves pieces fit together. (Validates [[INV-001|causal traceability]].)

### 5.4 WHY NOT Annotations

**What it is**: Inline comments explaining the road not taken.

**When to use**: Whenever an implementer might think "I can improve this by doing X instead," and X was considered and rejected.

**Format**: `// WHY NOT [alternative]? [Tradeoff. Reference [[ADR-NNN|substance]] if one exists.]`

If a WHY NOT annotation grows beyond 3 lines, it should become an ADR.

### 5.5 Comparison Blocks

**What it is**: Side-by-side ❌ SUBOPTIMAL vs ✅ CHOSEN comparisons with quantified reasoning.

### 5.6 Verification Prompts

**What it is**: A structured self-check at the end of each implementation chapter that an implementer (human or LLM) uses to verify their output against the spec.

**Why it exists**: LLMs can follow explicit self-verification instructions. Verification prompts convert passive specification into active self-checking, catching errors during generation rather than at review. (Justified by §0.2.3, Principle L3; locked by [[ADR-010|verification prompts required per chapter]]; validated by [[INV-019|verification prompt per chapter]].)

**Required format**:
```
**Verification prompt for [Subsystem]:**
1. Does your implementation preserve [[INV-NNN|restated substance]]?
2. Does your implementation preserve [[INV-NNN|restated substance]]?
3. Does your implementation avoid [negative spec 1]?
4. Does your implementation avoid [negative spec 2]?
5. Can you trace your implementation back to [[ADR-NNN|substance]]?
6. [Domain-specific check, e.g., "Run the worked example through
   your implementation. Does the output match?"]
```

**Quality criteria**:
- References specific invariants with substance restated ([[INV-018|substance restated]])
- References specific negative specifications from the same chapter ([[INV-017|negative spec coverage]])
- Includes at least one concrete verification step (e.g., running the worked example)
- Answerable by an LLM examining its own generated code

**Worked example** (for a task scheduler chapter):
```
Verification prompt for the Task Scheduler:
1. Does your implementation preserve [[INV-003|same events → identical state]]?
   Specifically: does your scheduler use wall-clock time? If yes, replay will diverge.
2. Does your implementation preserve [[INV-007|no starvation for > 1000 ticks]]?
   Check: can you construct a scenario where a low-priority task is never scheduled?
3. Does your implementation avoid spawning threads for event processing? (Negative spec #1)
4. Does your implementation avoid caching scheduling decisions across reducer calls? (Negative spec #2)
5. Run the worked example: Agent A-007 completes T-042. Does T-043 transition to Ready?
   Does a cancelled T-043 remain Cancelled?
```

**Must NOT**: Verification prompts must NOT be generic checklists ("is the code clean?"). Every item must reference a specific invariant, negative spec, or worked example from the same chapter.

### 5.7 Meta-Instructions

**What it is**: Explicit directives about implementation strategy, ordering, and non-obvious approaches — guidance invisible to compilers but valuable to human and LLM implementers.

**Why it exists**: LLMs implement in whatever order they encounter content. Without explicit ordering, dependent subsystems may be implemented before their dependencies. (Justified by §0.2.3; validated by [[INV-020|meta-instruction explicitness]].)

**Required format**:
```
**Meta-instructions for [Subsystem]:**
- Implement [X] before [Y] because [Y depends on X's interface for Z].
- When implementing [algorithm], start with [simple case] and extend to [general case].
- Do NOT optimize [hot path] until benchmarks confirm it is the bottleneck.
```

**Quality criteria**:
- Every ordering directive includes a rationale
- Strategy directives are actionable
- References invariants or ADRs where relevant

**Worked example** (for a storage layer):
```
Meta-instructions for the Storage Layer:
- Implement the event log writer before the snapshot mechanism, because
  snapshots are validated by replaying the event log ([[INV-003|determinism]]).
- Implement the read path as a pure function of snapshot + event suffix.
  Do NOT add caching until Benchmark B-003 confirms read latency exceeds budget.
- Start with a single-file event log. The sharding strategy ([[ADR-007|
  single-file chosen over sharded]]) is a later concern.
```

**Must NOT**: Meta-instructions must NOT contradict the spec's content. Must NOT be vague ("implement carefully"). Must NOT prescribe micro-level details.

### 5.8 Element Composition Trace

**What it is**: A worked example demonstrating how all DDIS element types compose within a single implementation chapter, showing the web of relationships between them.

**Why it exists**: Individual element specifications (§§2–5) explain each element in isolation. This trace shows how they work together in practice, exercising [[INV-001|causal traceability]], [[INV-006|cross-reference density]], [[INV-017|negative spec coverage]], [[INV-018|substance restated]], [[INV-019|verification prompts]], and [[INV-022|parseable cross-refs]] simultaneously.

**Worked example** — A complete implementation chapter skeleton for a "Snapshot Manager" subsystem:

```
## Chapter N: Snapshot Manager

### Purpose
The Snapshot Manager periodically captures the full state of the system,
enabling fast recovery and bounding replay time.
(Derived from [[APP-INV-003|same events → identical state]].)

### Formal Types
  SnapshotState = { version: u64, state: SystemState, event_cursor: EventId }
  // WHY NOT store a diff instead of full state? Diffs require the preceding
  // snapshot to reconstruct — breaks independent recovery ([[APP-ADR-007|
  // full snapshot chosen over incremental]]).

### Algorithm: snapshot_create
  [pseudocode]
  Complexity: O(state_size), expected < 50ms at design point
  Worked example: With 300 agents and 10K tasks, snapshot is ~2MB, created in 35ms.
  Edge case: If an event arrives during snapshot, the cursor is set to the last
  event processed BEFORE snapshot began.

### Invariants Preserved
  - [[APP-INV-003|same events → identical state]]: snapshot + replay(events_after_cursor)
    must produce identical state to direct processing of all events
  - [[APP-INV-018|snapshot is atomic]]: no partial snapshots are visible to readers

### Negative Specifications
  - Must NOT read events during snapshot creation. (Race condition breaks atomicity.)
  - Must NOT compress snapshots with lossy algorithms. (State must be byte-exact.)
  - Must NOT delete old snapshots automatically. (Retention is a policy decision,
    not an implementation concern — see Non-Goals.)

### Test Strategy
  - Property test: snapshot + replay = direct processing (for random event sequences)
  - Stress test: concurrent reads during snapshot creation

### Meta-instructions
  - Implement after the event log writer — snapshot validation requires
    event replay ([[APP-INV-003|determinism]]).

### Verification Prompt
  1. Does your snapshot + replay produce byte-identical state to direct processing?
  2. Can concurrent reads observe a partial snapshot?
  3. Does your implementation avoid reading events during creation? (Neg spec #1)
  4. Run: create snapshot at cursor 1000, then replay events 1001-1100. Compare.
```

This skeleton exercises 10 of the 13 required chapter components, demonstrates machine-readable cross-references, and shows how negative specs, verification prompts, and meta-instructions interlock.
