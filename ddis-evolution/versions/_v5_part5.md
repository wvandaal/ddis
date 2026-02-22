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

**Negative specifications for PART 0 element authoring:**
- Invariants must NOT use subjective terms without quantification. 'The system is fast' is not an invariant — 'event ingestion < 100µs p99' is. (Prevents [[INV-003|unfalsifiable invariants]].)
- ADRs must NOT present strawman alternatives. If only one option is viable, the choice doesn't need an ADR — it belongs in Non-Negotiables. (Prevents the Strawman ADR anti-pattern, §8.3.)
- Non-negotiables must NOT restate type-system guarantees. 'All values are strongly typed' is enforced by the compiler, not the spec. (Prevents trivial invariants, §8.3.)
- Quality gates must NOT use subjective pass/fail criteria. 'The code is clean enough' is not a gate — 'zero orphan sections in reference graph' is. (Prevents [[INV-003|unfalsifiable invariants]] at the gate level.)

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
14. **Implementation mapping**: Spec-to-code traceability table (per §5.9, [[INV-025|spec-to-code traceability]]).

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

**Correctness requirement** (new in 4.0, validated by [[INV-023|example correctness]]): Every worked example must be consistent with the spec's own invariants. Before finalizing a worked example, verify: (a) the before-state is a valid system state, (b) the operation is permitted by the state machine, (c) the after-state satisfies all applicable invariants, (d) edge cases described are consistent with the error taxonomy. An incorrect example is worse than no example — LLMs reproduce examples faithfully, including errors.

**Example verification checklist:**
```
For each worked example:
  1. Identify applicable invariants for this subsystem
  2. Check: does the before-state satisfy them?
  3. Check: is the operation valid per the state machine?
  4. Check: does the after-state satisfy them?
  5. Check: does the edge case match the error handling spec?
  If any check fails, fix the example before publishing.
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

### Implementation Mapping
| Spec Element | Artifact | Type | Notes |
|---|---|---|---|
| [[APP-INV-003\|same events → identical state]] | src/snapshot/create.rs::snapshot_create() | Enforces | Snapshot + replay = direct processing |
| [[APP-INV-003\|same events → identical state]] | tests/snapshot_test.rs::test_replay_equivalence | Validates | Property test with random events |
| [[APP-INV-018\|snapshot is atomic]] | src/snapshot/create.rs::snapshot_create() | Enforces | Atomic write, no partial visibility |
| Algorithm: snapshot_create | src/snapshot/create.rs::snapshot_create() | Implements | O(state_size), cursor before snapshot |
```

This skeleton exercises 11 of the 14 required chapter components, demonstrates machine-readable cross-references, and shows how negative specs, verification prompts, meta-instructions, and implementation mappings interlock.

**Example correctness validation** ([[INV-023|example correctness]]): The snapshot manager example above was verified against its referenced invariants: APP-INV-003 (snapshot + replay = direct processing) holds for the worked example's values; APP-INV-018 (atomic snapshots) is not violated by the described behavior; the edge case (event during snapshot) is consistent with the state machine. This verification step is mandatory for all worked examples in conforming specs.

### 5.9 Implementation Mapping

**What it is**: A structured table at the end of each implementation chapter mapping spec elements (invariants, algorithms, negative specifications, ADRs) to implementation artifacts (files, functions, tests, assertions).

**Why it exists**: DDIS traces from first principles through invariants and ADRs to implementation chapters. The implementation mapping closes the final link: from spec to code. Without it, invariants exist in the spec but have no traceable path to the code that enforces them — a gap that widens silently as the codebase evolves. (Justified by §0.2.7; locked by [[ADR-016|structured implementation mapping]]; validated by [[INV-025|spec-to-code traceability]].)

**Required format**:
```
**Implementation Mapping for [Subsystem Name]:**

| Spec Element | Artifact | Type | Notes |
|---|---|---|---|
| [[INV-NNN\|substance]] | path/to/file.rs::function() | Enforces/Validates/Implements | Brief explanation |
| [[ADR-NNN\|substance]] | path/to/module/ | Implements | How the decision manifests |
| Negative spec: [constraint] | path/to/test.rs::test_name | Validates | What the test checks |
| Algorithm: [name] | path/to/file.rs::algorithm() | Implements | Key implementation notes |
```

**Column definitions**:
- **Spec Element**: The invariant, ADR, negative spec, or algorithm being traced. Use `[[ID|substance]]` for invariants and ADRs.
- **Artifact**: File path and optionally function/method name. Use `path::function()` syntax.
- **Type**: One of:
  - **Enforces**: The artifact's code directly maintains this invariant (e.g., a single-threaded event loop enforces determinism).
  - **Validates**: The artifact tests or asserts this property (e.g., a replay test validates determinism).
  - **Implements**: The artifact is the primary implementation of this decision or algorithm.
- **Notes**: Brief explanation of how the artifact relates to the spec element.

**Quality criteria**:
- **Coverage**: Every invariant in the chapter's "Invariants preserved" section has at least one mapping entry.
- **Bidirectional**: Every artifact in the mapping references at least one spec element, and every spec element references at least one artifact.
- **Granularity**: Artifacts are specific enough to navigate to (file + function, not just "the codebase").
- **Current**: The mapping is updated when code is refactored. Stale mappings are detected by Gate 10.

**Worked example** (for the Snapshot Manager from §5.8):
```
**Implementation Mapping for Snapshot Manager:**

| Spec Element | Artifact | Type | Notes |
|---|---|---|---|
| [[APP-INV-003\|same events → identical state]] | src/snapshot/create.rs::snapshot_create() | Enforces | Snapshot + replay must equal direct processing |
| [[APP-INV-003\|same events → identical state]] | tests/snapshot_test.rs::test_snapshot_replay_equivalence | Validates | Property test: random events, snapshot, replay, compare |
| [[APP-INV-018\|snapshot is atomic]] | src/snapshot/create.rs::snapshot_create() | Enforces | Atomic write with no concurrent reads during creation |
| [[APP-INV-018\|snapshot is atomic]] | tests/snapshot_test.rs::test_concurrent_read_during_snapshot | Validates | Stress test: concurrent reads see complete or no snapshot |
| Negative spec: no event reads during creation | tests/snapshot_test.rs::test_no_event_reads_during_create | Validates | Asserts no event log access during snapshot_create() |
| Negative spec: no lossy compression | src/snapshot/create.rs::snapshot_create() | Enforces | Uses lossless serialization; comment: 'MUST NOT compress lossy' |
| Algorithm: snapshot_create | src/snapshot/create.rs::snapshot_create() | Implements | O(state_size), atomic, cursor set before snapshot begins |
```

**Anti-patterns**:
```
❌ BAD: "INV-003 → the codebase" — Not specific enough; which file? which function?
❌ BAD: Mapping with zero "Validates" entries — no tests trace to spec elements
✅ GOOD: Specific file::function with both Enforces and Validates entries
```

**Must NOT**: Implementation mappings must NOT include artifacts that don't exist yet (aspirational mappings). Map to actual code. For specs written before implementation, the mapping starts empty and is populated during implementation — this is expected and tracked by Gate 10's coverage metric.

**LLM usage**: During Pass 4 of the multi-pass workflow (§0.2.7), an LLM reviews the implementation mapping to verify coverage completeness. The mapping also serves as a navigation aid: an LLM asked to modify a subsystem can consult the mapping to find which invariants are at risk.

**Relationship to verification prompts**: The implementation mapping and verification prompts (§5.6) are complementary. Verification prompts check the implementation during generation (Pass 2). The mapping verifies traceability after generation (Pass 4). [[INV-026|Verification coverage completeness]] ensures every invariant appears in at least one verification prompt; [[INV-025|spec-to-code traceability]] ensures every invariant appears in at least one mapping entry. Together, they close the loop from spec to code to verification.

---

## Chapter 6: PART IV Elements

### 6.1 Operational Playbook

**What it is**: Prevents infinite refinement without shipping.

#### 6.1.1 Phase -1: Decision Spikes

Before building anything, run experiments that de-risk unknowns. Each spike produces an ADR (with Confidence field — spikes often produce Provisional ADRs). Required per spike: question it answers, time budget (1–3 days), exit criterion (one ADR).

#### 6.1.2 Exit Criteria per Phase

Every phase has a specific, testable exit criterion.

**Anti-pattern**: "Phase 2: Implement the scheduler. Exit: Scheduler works."
**Good example**: "Phase 2 exit: Property test demonstrates fair scheduling across 50 agents with no starvation for > 1000 ticks. Benchmark shows dispatch < 1ms at design point."

#### 6.1.3 Merge Discipline

What every PR touching invariants or critical paths must include: tests, a note on which invariants it preserves (with substance restated per [[INV-018|substance restated at point of use]]), benchmark comparison if touching a hot path.

#### 6.1.4 Minimal Deliverables Order

The order to build subsystems, chosen to maximize the "working subset" at each stage. Must include dependency rationale.

#### 6.1.5 Immediate Next Steps (First PRs)

The literal first 5–6 things to implement, in dependency order. Tactical, not strategic.

### 6.2 Testing Strategy

| Test Type | What It Validates | Example |
|---|---|---|
| Unit | Individual function correctness | Conflict detection returns correct overlaps |
| Property | Invariant preservation under random inputs | ∀ events: replay(snapshot, events) = direct_state |
| Integration | Subsystem composition | Completed task triggers scheduling cascade |
| Stress | Behavior at design point limits | 300 agents, 10K tasks, sustained 60s |
| Replay | Determinism | Process N events, snapshot, replay, byte-compare |
| Adversarial | Robustness against malformed input | Agent sends event with forged task_id |
| LLM Output | Implementation matches spec | Verification prompt (§5.6) run against generated code |
| Example Verification | Worked examples consistent with invariants | Snapshot example: after-state satisfies APP-INV-003 ([[INV-023|example correctness]]) |
| Mapping Coverage | Implementation traces to spec | Every INV has ≥ 1 artifact in mapping; Gate 10 ≥ 0.9 coverage |
| Spec Structural | Spec itself is well-formed | Automated cross-ref validation (§12.3, Gate 8) |

### 6.3 Error Taxonomy

**What it is**: Classification of errors with handling strategy per class.

**Required**: Each error class has severity (fatal/degraded/recoverable/ignorable), handling strategy (crash/retry/degrade/log), and cross-references to threatened invariants.

See also Appendix D for the specification authoring error taxonomy (meta-level).

---

## Chapter 7: Appendix Elements

### 7.1 Glossary

**What it is**: Every domain-specific term, defined in 1–3 sentences with a cross-reference.

**Required**: Alphabetized. Each entry includes `(see §X.Y)`. Terms with both common and domain-specific meanings clearly distinguish the two.

**LLM note**: The glossary is the primary disambiguation mechanism ([[INV-009|glossary coverage]]). Domain terms with common-English meanings are the #1 source of LLM misinterpretation.

### 7.2 Risk Register

**What it is**: Top 5–10 risks, each with description, impact, mitigation, and detection.

### 7.3 Master TODO Inventory

**What it is**: Comprehensive, checkboxable task list organized by subsystem.

**Required**: Organized by subsystem. Each item is PR-sized. Cross-references to ADRs/invariants. Includes tasks for writing negative specifications and verification prompts per subsystem. Includes a "Spec Testing" section for setting up automated validation.

---
