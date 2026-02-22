# PART II: CORE STANDARD — ELEMENT SPECIFICATIONS

This is the heart of DDIS. Each section specifies one structural element: what it must contain, what quality criteria it must meet, how it relates to other elements, and what it looks like when done well versus done badly.

// LLM NOTE: When using PART II as a reference while writing a new spec, you do NOT need to hold all element specifications in context simultaneously. Load the element specification for the section you are currently writing. Each element spec is self-contained. (See §0.2.3.)

## Chapter 2: Preamble Elements

### 2.1 Design Goal

**What it is**: A single sentence (≤ 30 words) that states the system's reason for existing.

**Required properties**:
- States the core value proposition, not the implementation
- Uses bold for emphasis on the 3–5 key properties
- Readable by a non-technical stakeholder

**Quality criteria**: A reader who sees only the design goal should be able to decide whether this system is relevant to them.

**What this element must NOT be**:
- Must NOT describe implementation ("Build a distributed task coordination system using event sourcing") — that's architecture, not value
- Must NOT use marketing language ("enterprise-grade", "cutting-edge")
- Must NOT exceed 30 words — brevity forces precision

**Good example** (FrankenTUI): "Design goal: **scrollback-native, zero-flicker, agent-ergonomic, and high-performance** Rust terminal apps."

**Cross-references**: The design goal establishes vocabulary used throughout. Each bolded property should correspond to at least one invariant and one quality gate.

---

### 2.2 Core Promise

**What it is**: A single sentence (≤ 40 words) that describes what the system makes possible, from the user's perspective.

**Required properties**:
- Written from the user's viewpoint, not the architect's
- States concrete capabilities, not abstract properties
- Uses "without" clauses to highlight what would normally be sacrificed

**What this element must NOT be**:
- Must NOT use buzzwords ("robust, scalable, enterprise-grade coordination")
- Must NOT describe internal architecture — the user doesn't care about your event bus

**Good example** (FrankenTUI): "ftui is designed so you can build a Claude Code / Codex-class agent harness UI without flicker, without cursor corruption, and without sacrificing native scrollback."

---

### 2.3 Document Note

**What it is**: A short disclaimer (2–4 sentences) about the nature of code blocks and where the correctness contract lives.

**Why it exists**: Without this note, implementers (especially LLMs) treat code blocks as copy-paste targets. When the pseudocode has a typo or uses a slightly wrong API, they copy the bug. The document note redirects trust from code to invariants and tests.

**Template**:
> Code blocks in this plan are **design sketches** for API shape, invariants, and responsibilities.
> They are intentionally "close to [language]," but not guaranteed to compile verbatim.
> The correctness contract lives in the invariants, tests, and ADRs — not in pseudo-code syntax.
> **LLM implementers**: Do NOT copy code blocks verbatim. Use them to understand intent, then implement according to the invariants.

---

### 2.4 How to Use This Plan

**What it is**: A numbered list (4–7 items) giving practical reading and execution guidance.

**Required properties**:
- Starts with "Read PART 0 end-to-end"
- Identifies the churn-magnets to lock via ADRs
- Points to the Master TODO as the execution tracker
- Identifies at least one non-negotiable process requirement
- Includes a step for LLM implementers: "check negative specifications before implementing each subsystem"

**What this element must NOT be**:
- Must NOT be vague ("read the spec carefully") — each step must be actionable
- Must NOT omit the reading order — LLMs need explicit sequencing

---

## Chapter 3: PART 0 Elements

### 3.1 Non-Negotiables (Engineering Contract)

**What it is**: 5–10 properties that define what the system IS. These are stronger than invariants (which are formal and testable) — they are the philosophical commitments that an implementer must never compromise, even under pressure.

**Required format**:
```
- **[Property name in bold]**
  [One sentence explaining what this means concretely]
```

**Quality criteria for each non-negotiable**:
- An implementer could imagine a situation where violating it would be tempting
- The non-negotiable clearly says: no, even then
- It is not a restatement of a technical invariant; it is a commitment

**Relationship to invariants**: Non-negotiables are the "why" that justifies groups of invariants. "Deterministic replay is real" (non-negotiable) justifies INV-003: "Same event log → identical state" (invariant).

**What this element must NOT be**:
- Must NOT be vague aspirations ("the system should be good")
- Must NOT duplicate invariants — non-negotiables are philosophical, invariants are testable

---

### 3.2 Non-Goals

**What it is**: A list of 5–10 things the system explicitly does NOT attempt.

**Why it exists**: Scope creep is the most common spec failure. Non-goals are the immune system. They give implementers permission to say "that's out of scope."

**Quality criteria for each non-goal**:
- Someone has actually asked for this (or will), making the exclusion non-obvious
- The non-goal explains briefly why it's excluded

**What this element must NOT be**:
- Must NOT exclude absurd things ("Non-goal: Building a quantum computer") — non-goals should exclude things that are tempting, not impossible
- Must NOT be used as a dumping ground for features you haven't thought about yet

---

### 3.3 First-Principles Derivation

**What it is**: The formal model from which the entire architecture derives. This is the section that makes every other section feel *inevitable* rather than *asserted*.

**Required components**:

1. **"What IS a [System]?"** — A mathematical or pseudo-mathematical definition establishing the system as a formally defined state machine or function.

2. **Consequences** — 3–5 bullet points explaining what this formal definition implies for the architecture.

3. **Fundamental Operations Table** — Every primitive operation with its mathematical model and complexity target.

**Quality criteria**: After reading this section, an implementer should be able to derive the system's architecture independently.

**What this element must NOT be**:
- Must NOT be hand-wavy ("the system processes events") — the formal model must be precise enough to derive invariants from
- Must NOT be disconnected from the architecture — if the architecture surprises a reader who understood the first principles, the derivation is incomplete
- Must NOT use notation that requires specialized training without defining it inline

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
- **Falsifiable**: You can construct a concrete counterexample
- **Consequential**: Violating it causes observable, bad behavior
- **Non-trivial**: Not a tautology or compiler-enforced constraint
- **Testable**: The validation method is specific enough to implement
- **Complete**: Has all five components (statement, formal expression, violation scenario, validation, WHY THIS MATTERS). Missing components violate INV-018 (Structural Predictability).

**Quantity guidance**: A medium-complexity system typically has 10–25 invariants.

**What this element must NOT be**:
- Must NOT be aspirational ("INV-001: The system shall be performant") — not falsifiable
- Must NOT be trivially true ("INV-002: TaskId values are unique" — if enforced by type system)
- Must NOT omit the violation scenario — this is the most common authoring error and the component most valuable for LLM consumers

**Anti-patterns**:
```
❌ BAD: "INV-001: The system shall be performant."
  - Not falsifiable, no violation scenario, not testable

✅ GOOD: "INV-003: Event Log Determinism
  Same event sequence applied to same initial state produces identical final state.
  ∀ events, ∀ state₀: reduce(state₀, events) = reduce(state₀, events)
  Violation: A reducer reads wall-clock time, causing different states on replay.
  Validation: Replay test — process 10K events, snapshot, replay, byte-compare.
  // WHY THIS MATTERS: If replay diverges, we lose auditability and debugging."
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

Confidence: [high/medium/low] — [brief justification per ADR-012 criteria]

// WHY NOT [rejected option]? [Brief explanation]

#### Consequences
[2–4 bullet points on what this decision implies]

#### Tests
[How we will know this decision was correct or needs revisiting]
```

**Quality criteria for each ADR**:
- **Genuine alternatives**: Each option must have a real advocate. The test: would a competent engineer in a different context reasonably choose it?
- **Concrete tradeoffs**: Pros and cons cite specific, measurable properties
- **Consequential decision**: Swapping options would require > 1 day of refactoring
- **Confidence assessed**: The Confidence field honestly reflects the certainty level per the criteria in ADR-012. Low-confidence ADRs must have a spike or re-evaluation plan in the operational playbook (§6.1.1).

**What this element must NOT be**:
- Must NOT contain strawman alternatives (The Strawman ADR anti-pattern — §8.3)
- Must NOT record pre-decided choices ("ADR-001: Use Rust" — language choice predates the spec)
- Must NOT omit the WHY NOT annotation — LLMs use WHY NOT to understand the solution space boundary

**Churn-magnets**: After all ADRs are written, add a brief section identifying which decisions cause the most downstream rework if changed. These are the decisions to lock first and spike earliest (see §6.1.1).

---

### 3.6 Quality Gates

**What it is**: 4–8 stop-ship criteria, ordered by priority.

**Required properties per gate**:
- A gate is a **predicate**, not a task. It is either passing or failing.
- Each gate references specific invariants or test suites.
- Gates are ordered such that a failing Gate N makes Gate N+1 irrelevant.
- At least one gate validates LLM implementation readiness (Gate 7 in this standard).

**What this element must NOT be**:
- Must NOT be vague ("Gate 1: Code quality is good") — gates must be testable predicates
- Must NOT omit the measurement procedure — a gate without a measurement is aspirational

---

### 3.7 Performance Budgets and Design Point

**What it is**: A table of performance targets anchored to a specific design point.

**Required components**:

1. **Design point**: The specific scenario these budgets apply to.
2. **Budget table**: Operation → target → measurement method.
3. **Measurement harness description**: How to run the benchmarks.
4. **Adjustment guidance**: How to recalibrate for different design points.

**What this element must NOT be**:
- Must NOT contain unanchored claims ("the system should be fast enough for real-time use")
- Must NOT omit the design point — a performance number without context is meaningless

---

### 3.8 Negative Specifications

**What it is**: Explicit constraints on what the system (or subsystem) must NOT do. Negative specifications address the most plausible misinterpretations that an implementer — especially an LLM — might introduce.

**Required format**:
```
NEGATIVE SPECIFICATION (what [subsystem] must NOT do):
- Must NOT [specific prohibited behavior] ([rationale or invariant reference])
- Must NOT [specific prohibited behavior] ([rationale or invariant reference])
```

**Quality criteria for each negative specification**:
- **Plausible**: A competent implementer (or LLM) might reasonably do this if not told otherwise. "Must NOT launch nuclear missiles" fails this test; "Must NOT cache event payloads across sessions" passes.
- **Specific**: States exactly what is prohibited, not vague categories. "Must NOT optimize prematurely" is too vague; "Must NOT add an in-memory cache for event lookups — the log is the cache (see APP-ADR-011)" is specific.
- **Justified**: References an invariant or ADR that the prohibited behavior would violate.

**Quantity guidance**: Each implementation chapter should have 2–5 negative specifications. Fewer suggests the author hasn't thought adversarially. More suggests the subsystem's positive specification is too ambiguous.

**Where negative specifications appear**:
- In each implementation chapter's "Negative Specifications" section (INV-017)
- In module headers for modular specs (§0.13.5)
- In the manifest's `negative_specs` field (§0.13.9)

**Anti-patterns**:
```
❌ BAD: "Must NOT be slow."
  ← Not specific. Not actionable. Use a performance budget instead.

❌ BAD: "Must NOT use goto statements."
  ← Not plausible for modern languages. Wastes the reader's attention.

✅ GOOD: "Must NOT implement event log compaction or mutation of
  persisted events. The log is append-only by design (APP-INV-017).
  Space management is handled by external archival (APP-ADR-015)."
  ← Specific, plausible (LLMs often add compaction), justified.
```

**Relationship to other elements**: Negative specifications are the complement of invariants. Invariants state what MUST hold; negative specifications state what must NOT happen. Together they bound the implementation from both sides. Every high-risk invariant should have at least one corresponding negative specification in the relevant implementation chapter.

// WHY THIS ELEMENT EXISTS: This element specification is new in DDIS 2.0, motivated by the LLM Consumption Model (§0.2.3). Empirical observation: LLMs implementing from specs without negative specifications produce "helpful" additions (caching, optimization, deduplication, compaction) that violate unstated invariants. Negative specifications are the single highest-leverage addition for LLM implementation correctness. (Locked by ADR-008.)

---

## Chapter 4: PART I Elements

### 4.1 Full Formal Model

**What it is**: The expanded version of the first-principles derivation from §0.2. While the executive summary gives the 1-page version, PART I gives the full treatment:

- Complete state definition (all fields, all types)
- Complete input/event taxonomy
- Complete output/effect taxonomy
- State transition semantics
- Composition rules (how subsystems interact)

**What this element must NOT be**:
- Must NOT be a restatement of the executive summary — it must add detail
- Must NOT introduce concepts not grounded in the formal model

### 4.2 State Machines

**What it is**: Every stateful component gets a formal state machine.

**Required per state machine**:
- State diagram (ASCII art or description)
- State × Event table (what happens for every combination — no empty cells)
- Guard conditions on transitions
- Invalid transition policy (ignore? error? log?)
- Entry/exit actions

**What this element must NOT be**:
- Must NOT have empty cells in the state × event table — every combination must be specified
- Must NOT omit the invalid transition policy — LLMs will invent one if you don't specify it

### 4.3 Complexity Analysis

**What it is**: Complexity bounds for every fundamental operation defined in the first-principles model.

**Required**: Big-O bounds with constants where they matter for the design point. "O(n) where n = active_agents, expected ≤ 300" is more useful than "O(n)."

---

## Chapter 5: PART II Elements

### 5.1 Implementation Chapters

**What it is**: One chapter per major subsystem. This is where the spec earns its value.

**Required components per chapter**:

1. **Purpose statement** (2–3 sentences): What this subsystem does and why it exists. References the formal model.
2. **Formal types**: Data structures with memory layout analysis where relevant.
3. **Algorithm pseudocode**: Every non-trivial algorithm, with complexity analysis inline.
4. **State machine** (if stateful): Full state machine per §4.2.
5. **Invariants preserved**: Which INV-NNN this subsystem is responsible for maintaining.
6. **Negative specifications**: What this subsystem must NOT do (§3.8, INV-017). Minimum 2.
7. **Worked example(s)**: At least one concrete scenario with specific values.
8. **Edge cases and error handling**: What happens when inputs are malformed or resources exhausted.
9. **Test strategy**: What kinds of tests cover this subsystem.
10. **Performance budget**: The subsystem's share of the overall performance budget.
11. **Cross-references**: To ADRs, invariants, other subsystems, the formal model.
12. **Verification prompt**: A self-check prompt for LLM implementers (§5.6).

**Quality criteria**: An implementer could build this subsystem from this chapter alone (plus the invariants and ADRs it references), without reading any other chapter.

// RECALL INV-017: Every implementation chapter must include ≥ 2 negative specifications. This is not optional polish — it is the primary defense against LLM hallucination in implementation chapters.

---

### 5.2 Worked Examples

**What it is**: A concrete scenario with specific values (not variables) showing the subsystem processing a realistic input.

**Required properties**:
- Uses concrete values: `task_id = T-042`, not "some task"
- Shows state before, the operation, and state after
- Includes at least one non-trivial aspect (an edge case, a conflict, a boundary condition)

**What this element must NOT be**:
- Must NOT use variables instead of values ("when a task is completed" — which task? what state?)
- Must NOT show only the happy path — include at least one edge case

**Anti-pattern**:
```
❌ BAD:
  "When a task is completed, the scheduler updates the DAG."

✅ GOOD:
  "Agent A-007 completes task T-042 (Implement login endpoint).
  Before: T-042 status=InProgress, T-043 depends on [T-042, T-041], T-041 status=Done
  Operation: TaskCompleted { task_id: T-042, agent_id: A-007, artifacts: [login.rs] }
  After: T-042 status=Done, T-043 status=Ready (all deps satisfied)
  Edge case: If T-043 had been cancelled while T-042 was in progress,
  T-043 remains Cancelled — completion of a dependency does not resurrect
  a cancelled task."
```

---

### 5.3 End-to-End Trace

**What it is**: A single worked scenario that traverses ALL major subsystems, showing how they interact.

**Required properties**:
- Traces one event or action from ingestion through every subsystem to final output
- Shows the exact data at each subsystem boundary
- Identifies which invariants are exercised at each step
- Includes at least one cross-subsystem interaction that could go wrong

**Why it exists**: Individual subsystem examples prove each piece works. The end-to-end trace proves the pieces fit together. Many bugs live at subsystem boundaries. (Validated by INV-001.)

**What this element must NOT be**:
- Must NOT skip subsystems — every major subsystem must appear
- Must NOT use abstract descriptions instead of concrete data at boundaries

---

### 5.4 WHY NOT Annotations

**What it is**: Inline comments next to design choices explaining the road not taken.

**When to use**: Whenever a design choice might look suboptimal to an implementer who doesn't have the full context.

**Format**:
```
// WHY NOT [alternative]? [Brief tradeoff explanation. Reference ADR-NNN if exists.]
```

**Relationship to ADRs**: WHY NOT annotations are micro-justifications for local choices. ADRs are macro-justifications for architectural choices. If a WHY NOT annotation grows beyond 3 lines, it should become an ADR.

**What this element must NOT be**:
- Must NOT be used for obvious choices — only for choices that might surprise an implementer

---

### 5.5 Comparison Blocks

**What it is**: Side-by-side ❌ SUBOPTIMAL vs ✅ CHOSEN comparisons with quantified reasoning.

**Format**:
```
// ❌ SUBOPTIMAL: [Rejected approach]
//   - [Quantified downside 1]
//   - [Quantified downside 2]
// ✅ CHOSEN: [Selected approach]
//   - [Quantified advantage 1]
//   - See ADR-NNN for full analysis
```

---

### 5.6 Verification Prompts

**What it is**: A structured self-check at the end of each implementation chapter that an LLM implementer can use to verify its own output against the spec.

**Required format**:
```
### Verification Prompt

Before considering this subsystem complete, verify:
1. [ ] [Invariant check]: Does the implementation preserve INV-NNN?
       Concrete test: [specific thing to check]
2. [ ] [Negative spec check]: Does the implementation violate any
       negative specification listed above? List each and confirm.
3. [ ] [Interface check]: Do the types at subsystem boundaries match
       the types declared in the formal model (§X.Y)?
4. [ ] [Edge case check]: Does the worked example's edge case
       produce the documented behavior?
```

**Quality criteria**:
- References specific invariants by number (INV-NNN)
- References specific negative specifications from the chapter
- Each check is a concrete, binary (yes/no) verification
- The prompt is self-contained — an LLM can run it without additional context

**Quantity guidance**: 4–8 verification items per chapter. Fewer suggests under-verification. More suggests the chapter is trying to do too much.

**What this element must NOT be**:
- Must NOT be vague ("verify the implementation is correct") — each item must be specific
- Must NOT reference sections outside the current chapter without providing the referenced content inline (INV-020: restatements must be fresh)
- Must NOT duplicate the test strategy — verification prompts check the implementation against the spec; tests check the implementation against expected behavior

// WHY THIS ELEMENT EXISTS: New in DDIS 2.0, motivated by §0.2.3. LLMs benefit from explicit self-check opportunities. A verification prompt transforms passive spec consumption into active verification, catching errors before they propagate. (Locked by ADR-010.)

---

### 5.7 Implementation Meta-Instructions

**What it is**: Explicit directives to the LLM implementer about how to approach the implementation — ordering, priorities, and process guidance that are invisible to compilers but valuable to LLM implementers.

**Required format**:
```
### Implementation Meta-Instructions

- Implement [subsystem A] before [subsystem B] because [B depends on A's types/API].
- Do NOT optimize [hot path] until benchmarking confirms it is actually hot.
- When implementing [algorithm], start with the worked example and generalize.
- [Subsystem] has a dependency on [external library] — use the API documented
  in [public reference], not the deprecated API from [older version].
```

**Quality criteria**:
- Each directive is actionable and specific
- Ordering directives include rationale (why A before B)
- Directives reference specific subsystems, not abstract categories

**What this element must NOT be**:
- Must NOT be generic advice ("write clean code") — must be specific to this spec
- Must NOT contradict the operational playbook's deliverables order (§6.1.4)
- Must NOT include micro-level implementation details (variable names, formatting) — only macro-level process guidance

**Where meta-instructions appear**:
- At the end of each implementation chapter (chapter-specific)
- In the operational playbook §6.1.4 (system-wide ordering)

// WHY THIS ELEMENT EXISTS: New in DDIS 2.0. LLMs benefit from explicit sequencing guidance. Without meta-instructions, an LLM may implement subsystems in an order that creates integration problems. Meta-instructions prevent the most common ordering errors. (Validates INV-019.)

---

## Chapter 6: PART IV Elements

### 6.1 Operational Playbook

**What it is**: A chapter that prevents the most common failure mode of detailed specs: infinite refinement without shipping.

**Required sections**:

#### 6.1.1 Phase -1: Decision Spikes

Before building anything, run tiny experiments that de-risk the hardest unknowns. Each spike produces an ADR.

**Required per spike**: What question it answers, maximum time budget (1–3 days), exit criterion (one ADR).

#### 6.1.2 Exit Criteria per Phase

Every phase must have specific, testable exit criteria.

**Anti-pattern**: "Phase 2: Implement the scheduler. Exit: Scheduler works."
**Good example**: "Phase 2: Implement the scheduler. Exit: Property test demonstrates fair scheduling across 50 agents with no starvation for > 1000 ticks. Benchmark shows dispatch < 1ms at design point."

#### 6.1.3 Merge Discipline

What every PR touching invariants, reducers, or critical paths must include.

#### 6.1.4 Minimal Deliverables Order

The order in which subsystems should be built, chosen to maximize the "working subset" at each stage. Must be consistent with meta-instructions (§5.7) and satisfy INV-019.

#### 6.1.5 Immediate Next Steps (First PRs)

The literal first 5–6 things to implement, in dependency order.

---

### 6.2 Testing Strategy

**What it is**: A taxonomy of test types with examples and guidance.

| Test Type | What It Validates | Example |
|---|---|---|
| Unit | Individual function correctness | Reservation conflict detection |
| Property | Invariant preservation under random inputs | ∀ events: replay(snapshot, events) = direct_state |
| Integration | Subsystem composition | Completed task triggers scheduling cascade |
| Stress | Behavior at design point limits | 300 agents, 10K tasks, 60s sustained |
| Replay | Determinism | Process N events, snapshot, replay, byte-compare |
| Adversarial | Robustness against malicious input | Agent sends event with forged task_id |

---

### 6.3 Error Taxonomy

**What it is**: A classification of errors the system can encounter, with handling strategy per class.

**Required properties**:
- Each error class has a severity (fatal, degraded, recoverable, ignorable)
- Each error class has a handling strategy (crash, retry, degrade, log-and-continue)
- Cross-references to invariants: which invariants might be threatened by each error class

**What this element must NOT be**:
- Must NOT omit the handling strategy — an error classification without a response plan is incomplete
- Must NOT treat all errors the same — severity differentiation is the entire point

---

## Chapter 7: Appendix Elements

### 7.1 Glossary

**What it is**: Every domain-specific term, defined in 1–3 sentences with a cross-reference.

**Required properties**:
- Alphabetized
- Each entry includes (see §X.Y) pointing to the formal definition
- Terms with both common and domain-specific meanings clearly distinguish the two

**What this element must NOT be**:
- Must NOT define terms vaguely ("task: a unit of work") — use the formal definition
- Must NOT omit cross-references — the glossary is a navigation aid, not just a dictionary

---

### 7.2 Risk Register

**What it is**: Top 5–10 risks to the project, each with concrete mitigation.

**Required per risk**: Risk description, impact, mitigation, detection method.

---

### 7.3 Master TODO Inventory

**What it is**: A comprehensive, checkboxable task list organized by subsystem.

**Required properties**:
- Organized by subsystem (not by phase)
- Each item is small enough to be a single PR
- Cross-references to the ADR or invariant that justifies it
- Checkboxable format (`- [ ]`)

---

# PART III: GUIDANCE (RECOMMENDED)

## Chapter 8: Voice and Style

### 8.1 The DDIS Voice

**Technically precise but human.** The voice of a senior engineer explaining their system to a peer they respect.

**Properties**:
- Uses concrete examples, not abstract descriptions
- Admits uncertainty where it exists ("this decision may need revisiting if...")
- Is direct about tradeoffs ("we chose X, which costs us Y")
- Does not hedge every statement ("arguably", "it could be said that")
- Uses humor sparingly and only when it clarifies
- Never uses marketing language ("enterprise-grade", "cutting-edge", "revolutionary")
- Never uses bureaucratic language ("it is recommended that", "the system shall")

**LLM-specific voice guidance**: LLMs that generate DDIS-conforming specs tend toward two failure modes: (1) academic verbosity ("utilizes a single-threaded architecture paradigm to facilitate...") and (2) generic boilerplate ("this module provides a robust and scalable..."). The DDIS voice inoculates against both — it is too specific for boilerplate and too direct for academic padding.

**Calibration examples**:

```
✅ GOOD: "The kernel loop is single-threaded by design — not because concurrency
is hard, but because serialization through the event log is the mechanism that
gives us deterministic replay for free."

❌ BAD (academic): "The kernel loop utilizes a single-threaded architecture
paradigm to facilitate deterministic replay capabilities."

❌ BAD (casual): "We made the kernel single-threaded and it's awesome!"

❌ BAD (bureaucratic): "It is recommended that the kernel loop shall be
implemented in a single-threaded manner to support the deterministic replay
requirement as specified in section 4.3.2.1."
```

### 8.2 Formatting Conventions

- **Bold** for terms being defined, non-negotiable properties, and emphasis on critical warnings
- `Code` for types, function names, file names, and anything that would appear in source code
- `// Comments` for inline justifications and WHY NOT annotations
- Tables for structured data (operations, budgets, comparisons) — prefer tables over prose for any data with ≥ 3 comparable items (LLMs parse tables more reliably than prose — §0.2.3)
- Blockquotes for the preamble elements only
- ASCII diagrams preferred over external image references

### 8.3 Anti-Pattern Catalog

Every DDIS element has bad and good examples defined in its specification (PART II). This section collects cross-cutting anti-patterns:

**Anti-pattern: The Hedge Cascade**
```
❌ "It might be worth considering the possibility of potentially using a
single-threaded loop, which could arguably provide some benefits..."
✅ "The kernel loop is single-threaded. This gives us deterministic replay.
See ADR-003 for the throughput analysis that confirms this is sufficient."
```

**Anti-pattern: The Orphan Section**
A section that references nothing and is referenced by nothing. Either connect it or remove it. (Validated by INV-006.)

**Anti-pattern: The Trivial Invariant**
"INV-042: The system uses UTF-8 encoding." Either enforced by the language/platform (not worth an invariant) or so fundamental it belongs in Non-Negotiables.

**Anti-pattern: The Strawman ADR**
```
❌ Options:
  A) Our chosen approach (clearly the best)
  B) A terrible approach nobody would choose
  Decision: A, obviously.
```
Every option must have a genuine advocate. (Validated by INV-002.)

**Anti-pattern: The Percentage-Free Performance Budget**
"The system should respond quickly." Without a number, design point, and measurement method, this is a wish.

**Anti-pattern: The Spec That Requires Oral Tradition**
If an implementer must ask a question the spec should have answered, the spec has a gap.

**Anti-pattern: The Afterthought LLM Section**
Adding a single "Chapter 14: LLM Considerations" appendix. LLM optimization must be woven throughout — into element specifications, quality gates, invariants, and authoring guidance. (Locked by ADR-009.)

**Anti-pattern: The Missing Negative**
An implementation chapter that says what the system DOES but never says what it must NOT do. LLMs fill the silence with plausible behavior. (Validated by INV-017.)

**Anti-pattern: The Ambiguous Reference**
An implementation chapter says "as described in the architecture section" without a section number. An LLM cannot resolve the reference and either skips it (missing a constraint) or hallucinates what the "architecture section" might say. (Validated by INV-021.)

---

## Chapter 9: Proportional Weight Deep Dive

### 9.1 Identifying the Heart

Every system has a "heart" — the 2–3 subsystems where most complexity and most bugs live. These subsystems should receive 40–50% of the PART II line budget.

**How to identify the heart**:
- Which subsystems have the most invariants?
- Which subsystems have the most ADRs?
- Which subsystems appear in the most cross-references?
- If you had to cut the spec in half, which subsystems would you keep?

### 9.2 Signals of Imbalanced Weight

- A subsystem with 5 invariants and 50 lines of spec is **starved**
- A subsystem with 1 invariant and 500 lines of spec is **bloated**
- PART 0 longer than PART II means the spec is top-heavy
- Appendices longer than PART II means reference material is displacing implementation spec

---

## Chapter 10: Cross-Reference Patterns

### 10.1 Reference Syntax (Required)

DDIS mandates the following reference forms (INV-021). These forms are machine-parseable and serve as reliable lookup keys for LLMs and automated validation tools.

**Standard reference forms:**

| Form | When to Use | Example |
|---|---|---|
| `(see §N.M)` | Section reference | (see §3.2) |
| `(INV-NNN)` | Invariant reference | (INV-004) |
| `(ADR-NNN)` | ADR reference | (ADR-003) |
| `(Benchmark B-NNN)` | Performance reference | (Benchmark B-001) |
| `(Glossary: "term")` | Glossary reference | (Glossary: "task") |
| `(Gate N)` | Quality gate reference | (Gate 7) |
| `(EXT:spec-name:INV-NNN)` | Cross-spec reference (§0.14) | (EXT:system-a:INV-017) |
| `(validated by INV-NNN)` | Validation reference | (validated by INV-004) |
| `(locked by ADR-NNN)` | Decision lock reference | (locked by ADR-003) |
| `(measured by Benchmark B-NNN)` | Measurement reference | (measured by Benchmark B-001) |
| `(defined in Glossary: "term")` | Glossary definition | (defined in Glossary: "task") |

**Prohibited reference forms:**

| Form | Why Prohibited |
|---|---|
| "See above" / "as mentioned earlier" | Positional references are unreliable for LLMs |
| "The previous section" / "the next section" | Navigational references break under reordering |
| "As discussed" (without section number) | Vague references that cannot be resolved |
| "The X invariant" (without INV-NNN) | LLMs may confuse similarly named invariants |

// LLM NOTE: Always use explicit section numbers and invariant IDs. Positional references ("see above") are unreliable — LLMs cannot resolve them. Standard forms serve as lookup keys that enable both automated validation and reliable LLM navigation. (Enforced by INV-021, locked by ADR-011.)

### 10.2 Reference Density Targets

| Section Type | Minimum Outbound References |
|---|---|
| Implementation chapter | 3 (at least: one ADR, one invariant, one other chapter) |
| ADR | 2 (at least: one invariant, one implementation chapter) |
| Invariant | 1 (at least: one test or validation method) |
| Performance budget | 2 (at least: one benchmark, one design point) |
| Test strategy | 2 (at least: one invariant, one implementation chapter) |
| Negative specification | 1 (at least: one invariant or ADR it protects) |

---
