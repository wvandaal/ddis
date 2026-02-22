# DDIS: Decision-Driven Implementation Specification Standard

## Version 3.0 — A Self-Bootstrapping Meta-Specification

> Design goal: **A formal standard for writing implementation specifications that are precise enough for an LLM to implement correctly on the first pass, while remaining readable enough that a senior engineer would choose to read them voluntarily.**

> Core promise: A specification conforming to DDIS contains everything needed to implement the described system — architecture, algorithms, invariants, decisions, negative constraints, test strategies, performance budgets, and execution plan — in a single cohesive document where every section earns its place by serving the sections around it, without requiring the implementer to guess, infer, or hallucinate any architectural detail.

> Document note (important):
> This standard is **self-bootstrapping**: it is written in the format it defines.
> Every structural element prescribed by DDIS is demonstrated in this document.
> Where this document says "the spec must include X," this document includes X — about itself.
> Code blocks are design sketches for illustration. The correctness contract lives in the
> invariants, not in any particular syntax.
> **LLM implementers**: Do NOT treat code blocks as copy-paste targets. They illustrate intent
> and structure; the invariants and tests define correctness.

> How to use this standard (practical):
> 1) Read **PART 0** once end-to-end: understand what DDIS requires, why, and how elements connect.
> 2) Lock your spec's **churn-magnets** via ADRs before writing implementation sections.
> 3) Write your spec following the **Document Structure** (§0.3), using PART II as the element-by-element reference.
> 4) For each implementation chapter, include **negative specifications** (what the subsystem must NOT do) and a **verification prompt** (§5.6, §5.7).
> 5) Validate against the **Quality Gates** (§0.7) — including Gate 7 (LLM Implementation Readiness) — and the **Completeness Checklist** (Part X) before considering the spec "done."
> 6) Treat the **cross-reference web** as a product requirement, not polish — it is the mechanism that makes the spec cohere.
> 7) If your spec exceeds **2,500 lines** or your target LLM's context window, read **§0.13 (Modularization Protocol)** and decompose into a manifest-driven module structure.
> 8) If your system depends on another DDIS-specified system, read **§0.14 (Composition Protocol)** for cross-spec reference rules.

---

# PART 0: EXECUTIVE BLUEPRINT

## 0.1 Executive Summary

DDIS (Decision-Driven Implementation Specification) is a standard for writing technical specifications that bridge the gap between architectural vision and correct implementation.

Most specifications fail in one of two ways: they are too abstract (the implementer must guess at critical details) or too mechanical (they prescribe code without explaining why, making evolution impossible). DDIS avoids both failure modes by requiring a **causal chain** from first principles through decisions to implementation details, where every element justifies its existence by serving the elements around it.

**The primary optimization target is LLM consumption.** The primary implementer reading a DDIS-conforming spec will be a large language model. Human readability remains a requirement — humans review, audit, and evolve specs — but when human readability and LLM effectiveness conflict, LLM effectiveness wins. This has specific structural consequences formalized in the LLM Consumption Model (§0.2.3) and enforced by INV-017 through INV-022.

DDIS synthesizes techniques from several traditions — Architecture Decision Records (Nygard), Design by Contract (Meyer), temporal invariants (Lamport), performance budgeting (game engine development), and test-driven specification — into a unified document structure. The synthesis is the contribution: these techniques are well-known individually but rarely composed into a single coherent standard.

### 0.1.1 What DDIS Is

DDIS is a document standard. It specifies:

- What structural elements a specification must contain
- How those elements must relate to each other (the cross-reference web)
- What quality criteria each element must meet
- How to structure elements for LLM consumption (§0.2.3, INV-017–022)
- How to validate that a specification is complete

DDIS is domain-agnostic. It can describe a terminal rendering kernel, an agent coordination system, a database engine, a compiler, or any system where correctness matters and multiple people (or LLMs) will implement from the spec.

### 0.1.2 Non-Negotiables (Engineering Contract)

These are not aspirations; they are the contract. If any are violated, a document is not a DDIS-conforming specification.

- **Causal chain is unbroken**
  Every implementation detail traces back through a decision, through an invariant, to a first principle. If you cannot trace a section's ancestry to the formal model, the section is unjustified.

- **Decisions are explicit and locked**
  Every design choice that could reasonably go another way is captured in an ADR with genuine alternatives considered. "We chose X" without "we rejected Y because Z" is not a decision — it is an assertion.

- **Invariants are falsifiable**
  Every invariant can be violated by a concrete scenario and detected by a concrete test. An invariant that cannot be tested is a wish, not a contract.

- **No implementation detail is unsupported**
  Every algorithm, data structure, state machine, and protocol has: pseudocode or formal description, complexity analysis, at least one worked example, and a test strategy. Prose descriptions of behavior without any of these are insufficient.

- **Cross-references form a web, not a list**
  ADRs reference invariants. Invariants reference tests. Tests reference performance budgets. Performance budgets reference the design point. The design point references first principles. A specification where sections exist in isolation is a collection of essays, not a DDIS spec.

- **The document is self-contained**
  A competent implementer with the spec and the spec alone — no oral tradition, no Slack threads, no "ask the architect" — can build a correct v1. If they cannot, the spec has failed.

- **Negative constraints are explicit**
  For every subsystem, the spec states what the subsystem must NOT do — the most plausible misinterpretations, the most tempting shortcuts, the behaviors that an implementer (especially an LLM) might reasonably infer but must not produce. Silence on what a system must not do is an invitation to hallucinate. (Enforced by INV-017, locked by ADR-008.)

- **Cross-references are standardized**
  All references use the standard DDIS reference syntax (§10.1, INV-021). Machine-parseable references enable automated validation and allow LLMs to navigate the spec without resolving ambiguous positional references.

### 0.1.3 Non-Goals (Explicit)

DDIS does not attempt:

- **To replace code.** A spec is not an implementation. It describes what to build, why, and how to verify it — not the literal source code. Design sketches illustrate intent; they are not copy-paste targets.
- **To eliminate judgment.** Implementers will make thousands of micro-decisions. DDIS constrains the macro-decisions (architecture, algorithms, invariants) so micro-decisions are locally safe.
- **To be a project management framework.** DDIS includes a Master TODO and phased roadmap, but these are execution aids for the spec's content, not a substitute for sprint planning or issue tracking.
- **To prescribe notation.** DDIS requires formal models but does not mandate TLA+, Alloy, Z, or any specific formalism. Pseudocode, state machine diagrams, mathematical notation, or "close to [language]" sketches are all acceptable if they are precise.
- **To guarantee correctness.** A DDIS-conforming spec dramatically reduces the chance of building the wrong thing. It cannot eliminate it. The spec is a contract for human (or LLM) intent, not a machine-checked proof.
- **To optimize for human-only consumption.** Where structural choices benefit LLM parsing at minor cost to human aesthetics (e.g., tables over flowing prose, explicit cross-references over implied ones), DDIS chooses the LLM-friendly option. Humans can still read it; LLMs can now parse it reliably.

## 0.2 First-Principles Derivation

### 0.2.1 What IS an Implementation Specification?

A specification is a function from intent to artifact:

```
Spec: (Problem, Constraints, Knowledge) → Document
where:
  Document enables: Implementer × Document → Correct_System
```

The quality of a specification is measured by one criterion: **does an implementer produce a correct system from it, without requiring information not in the document?**

This definition has consequences:

1. **Completeness over elegance.** A verbose spec that leaves nothing ambiguous is better than a terse spec that leaves critical details to inference. (But see INV-007: verbosity without structure is noise.)

2. **Decisions over descriptions.** The hardest part of building a system is not writing code — it is making the hundreds of design decisions that determine whether the code is correct. A spec that describes a system without recording why it is shaped that way is a snapshot, not a blueprint.

3. **Verifiability over trust.** Every claim in the spec must be testable. "The system is fast" is not verifiable. "Event ingestion completes in < 100µs p99 at the design point of 300 agents / 10K tasks, measured by Benchmark B-001" is verifiable.

4. **Exclusion over implication.** What the system must NOT do is as important as what it must do. An LLM implementer will fill gaps with plausible-sounding behavior; the spec must close those gaps explicitly. (See §0.2.3.)

### 0.2.2 The Causal Chain (Why DDIS Is Structured This Way)

DDIS prescribes a specific document structure because specifications fail in predictable ways, and each structural element prevents a specific failure mode:

| Failure Mode | Symptom | DDIS Element That Prevents It |
|---|---|---|
| Implementer builds the wrong abstraction | Core types don't fit the domain | First-principles formal model (§0.2) |
| Two implementers make incompatible choices | Modules don't compose | Architecture Decision Records (§0.6) |
| System works but violates a safety property | Subtle correctness bugs | Numbered invariants with tests (§0.5) |
| System is correct but too slow | Performance death by a thousand cuts | Performance budgets with benchmarks (§0.8) |
| Nobody knows if the system is "done" | Infinite refinement | Quality gates + Definition of Done (§0.7) |
| New contributor can't understand the system | Oral tradition required | Cross-reference web + glossary |
| Spec covers happy path but not edge cases | Production failures on unusual inputs | Worked examples + end-to-end traces (PART II) |
| Spec is so long nobody reads it | Shelfware | Proportional weight guide + voice guidance |
| LLM implementer hallucinates plausible behavior | Correct-looking but wrong implementation | Negative specifications + verification prompts (§3.8, §5.6) |
| LLM loses critical context in long spec | Invariant violations in later chapters | Structural redundancy at point of use (INV-020) |
| LLM implements subsystems in wrong order | Integration failures from missing dependencies | Implementation ordering directives (§5.7) |
| LLM cannot resolve ambiguous reference formats | Missed constraints during implementation | Standardized reference syntax (INV-021, §10.1) |
| System depends on another spec but contracts are implicit | Integration failures across spec boundaries | Composition protocol (§0.14) |

```
First Principles (formal model of the problem)
  ↓ justifies
Non-Negotiables + Invariants (what must always be true)
  ↓ constrained by
Architecture Decision Records (choices that could go either way)
  ↓ implemented via
Algorithms + Data Structures + Protocols (pseudocode, state machines)
  ↓ bounded by
Negative Specifications (what the system must NOT do)
  ↓ verified by
Test Strategies + Performance Budgets + Verification Prompts
  ↓ shipped via
Quality Gates + Master TODO (stop-ship criteria, execution checklist)
```

Every element in DDIS exists because removing it causes a specific, named failure. There are no decorative sections.

### 0.2.3 LLM Consumption Model

The primary implementer of a DDIS-conforming spec is an LLM. This section models how LLMs consume specifications, establishing the causal justification for INV-017 through INV-020 and ADR-008 through ADR-012.

**How LLMs process specifications:**

| LLM Behavior | Consequence for Spec Structure | DDIS Provision |
|---|---|---|
| Context windows are finite; early content may be attended to less in very long documents | Critical invariants must be restated at point of use, not only in §0.5 | INV-020: Restatement Freshness |
| LLMs hallucinate plausible details not in the spec | Every subsystem needs explicit "do NOT" constraints | INV-017: Negative Specification Coverage |
| LLMs over-index on examples; bad examples are actively harmful | Worked examples must be higher quality than for human-only specs | §5.2 quality criteria |
| LLMs struggle with implicit cross-references ("see above") | Explicit section numbers and invariant IDs are mandatory | INV-006, §10.1 |
| LLMs benefit from fixed formats (every ADR follows same template) | Structural predictability reduces output variance | INV-018: Structural Predictability |
| LLMs can be instructed via the spec itself | Meta-instructions ("implement X before Y") are valuable | §5.7: Meta-Instructions |
| LLMs handle tabular data better than dense prose | Tables preferred over paragraphs for structured data | §8.2 formatting conventions |
| LLMs cannot ask clarifying questions | The spec must anticipate and answer all architectural questions | INV-008, Gate 7 |
| LLMs benefit from self-check opportunities | Verification prompts per chapter enable self-validation | INV-018, §5.6 |
| LLMs process cross-references as lookup keys | Non-standard references are missed or misresolved | INV-021: Reference Syntax Standardization |

**The LLM-effectiveness litmus test**: For every structural element in a DDIS spec, ask: "If I gave this element to an LLM with no other context, would the LLM produce a more correct implementation than without it?" If yes, the element earns its place. If no, the element serves humans only and is P2 at best.

**Context budget model**: An LLM consuming a spec has a finite context window. The spec competes for context with the LLM's own reasoning and output generation. A spec that fills the entire context window leaves no room for the LLM to think. The modularization protocol (§0.13) exists to manage this budget. Even for monolithic specs, section length discipline (§0.8.2) and structural redundancy (INV-020) ensure that the LLM has the critical constraints available at every point in the document.

### 0.2.4 Fundamental Operations of a Specification

Every specification, regardless of domain, performs these operations:

| Operation | What It Does | DDIS Element |
|---|---|---|
| **Define** | Establish what the system IS, formally | First-principles model, formal types |
| **Constrain** | State what must always hold | Invariants, non-negotiables |
| **Decide** | Lock choices where alternatives exist | ADRs |
| **Describe** | Specify how components work | Algorithms, state machines, protocols |
| **Exemplify** | Show the system in action | Worked examples, end-to-end traces |
| **Bound** | Set measurable limits | Performance budgets, design point |
| **Verify** | Define how to confirm correctness | Test strategies, quality gates, verification prompts |
| **Exclude** | State what the system is NOT and must NOT do | Non-goals, negative specifications |
| **Sequence** | Order the work | Phased roadmap, implementation meta-instructions |
| **Lexicon** | Define terminology | Glossary |

## 0.3 Document Structure (Required)

A DDIS-conforming specification must contain the following structure. Sections may be renamed to fit the domain but the structural elements are mandatory unless explicitly marked [Optional].

```
PREAMBLE
  Design goal (one sentence)
  Core promise (user-facing, one sentence)
  Document note (about code sketches and where correctness lives)
  How to use this plan (numbered practical steps)

PART 0: EXECUTIVE BLUEPRINT
  §0.1  Executive Summary
  §0.2  First-Principles Derivation (formal model)
  §0.3  Architecture Overview (rings, layers, or crate map)
  §0.4  Workspace / Module Layout
  §0.5  Invariants (numbered: INV-001, INV-002, ...)
  §0.6  Architecture Decision Records (ADR-001, ADR-002, ...)
  §0.7  Quality Gates (stop-ship criteria) + Definition of Done
  §0.8  Performance Budgets + Design Point
  §0.9  Public API Surface (target sketches)
  §0.10 Open Questions (resolve early, track as ADRs)     [Optional]
  §0.11 Non-Negotiables (engineering contract)
  §0.12 Non-Goals (explicit exclusions)
  §0.13 Modularization Protocol (specs > context window)   [Conditional]
  §0.14 Composition Protocol (multi-spec systems)         [Conditional]

PART I: FOUNDATIONS
  First-principles derivation (full formal model)
  State machines for all stateful components
  Complexity analysis for fundamental operations
  "Why this architecture is inevitable" narrative

PART II: CORE IMPLEMENTATION (the heart of the spec)
  One chapter per major subsystem, each containing:
    - Formal types (data model)
    - Algorithm pseudocode
    - State machine (if stateful)
    - Invariants this subsystem must preserve
    - Negative specifications (what this subsystem must NOT do)
    - Worked example(s)
    - WHY NOT annotations on non-obvious choices
    - Test strategy
    - Performance budget for this subsystem
    - Cross-references to ADRs, invariants, other subsystems
    - Verification prompt (LLM self-check for this chapter)
  End-to-end trace (one worked scenario traversing ALL subsystems)

PART III: INTERFACES
  External protocol/API schemas
  Adapter specifications
  View-model / UI data contracts                           [Optional]

PART IV: OPERATIONS
  Testing strategy (taxonomy: unit, property, integration, stress, replay)
  Error taxonomy and handling strategy
  Operational playbook ("how this actually ships")
    - Phase -1: Decision spikes
    - Exit criteria per phase
    - Merge discipline
    - Minimal deliverables order
    - Implementation ordering directives
    - Immediate next steps (first PRs)
  Agent/tool compatibility notes                           [Optional]

APPENDICES
  A: Glossary (every domain term, cross-referenced)
  B: Risk Register (risks + mitigations)
  C: Storage / Wire Formats                                [Optional]
  D: Specification Error Taxonomy
  E: Benchmark Scenarios                                   [Optional]
  F: Reference Implementations / Extracted Code            [Optional]

PART X: MASTER TODO INVENTORY
  Checkboxable task list organized by subsystem
  Cross-referenced to phases, ADRs, and quality gates
```

### 0.3.1 Ordering Rationale

The ordering is not arbitrary. It follows the **dependency chain of understanding**:

1. First principles establish vocabulary and the formal model
2. Invariants constrain what the system may do
3. ADRs lock the choices within those constraints
4. Implementation chapters describe how, within those locked choices — including what NOT to do
5. Interfaces describe the system's boundaries
6. Operations describe how to build and verify it
7. Appendices provide reference material

An implementer reading top-to-bottom builds understanding incrementally. No section requires forward references to be understood (backward references are expected and encouraged).

// WHY THIS ORDER FOR LLMs: LLMs process documents sequentially. By placing invariants and ADRs before implementation chapters, the LLM has the constraints loaded into context before encountering the implementation details they constrain. This reduces hallucination of behavior that violates invariants.

## 0.4 This Standard's Architecture

DDIS has a simple ring architecture:

1. **Core Standard (sacred)**: The mandatory structural elements, their required contents, quality criteria, and relationships. (PART 0, PART I, PART II of this document.)

2. **Guidance (recommended)**: Voice, proportional weight, anti-patterns, worked examples. These improve spec quality but their absence does not make a spec non-conforming. (PART III of this document.)

3. **Tooling (optional)**: Checklists, templates, validation procedures. (PART IV, Appendices.)

## 0.5 Invariants of the DDIS Standard

Every DDIS-conforming specification must satisfy these invariants. Each invariant has an identifier, a plain-language statement, a semi-formal expression, a violation scenario, a validation method, and a WHY THIS MATTERS annotation.

---

**INV-001: Causal Traceability**

*Every implementation section traces to at least one ADR or invariant, which traces to the formal model.*

```
∀ section ∈ PART_II:
  ∃ adr ∈ ADRs ∪ inv ∈ Invariants:
    section.references(adr ∨ inv) ∧ (adr ∨ inv).derives_from(formal_model)
```

Violation scenario: An implementation chapter describes a caching layer with no ADR justifying the cache and no invariant the cache preserves. Six months later, nobody knows if the cache can be safely removed.

Validation: Manual audit. Pick 5 random implementation sections. For each, follow cross-references backward to an ADR or invariant, then to the formal model. If any chain breaks, INV-001 is violated.

// WHY THIS MATTERS: Without traceability, sections accumulate by accretion ("add a caching layer") without justification. The causal chain is the mechanism that prevents spec rot.

---

**INV-002: Decision Completeness**

*Every design choice where a reasonable alternative exists is captured in an ADR.*

```
∀ choice ∈ spec where ∃ alternative ∧ alternative.is_reasonable:
  ∃ adr ∈ ADRs: adr.covers(choice) ∧ adr.alternatives.contains(alternative)
```

Violation scenario: An implementation chapter prescribes a B-tree index without an ADR. A reviewer asks "why not a hash map?" and nobody can explain the tradeoff — the decision was made implicitly.

Validation: Adversarial review. A reviewer reads each implementation section and asks "could this reasonably be done differently?" If yes and no ADR exists, INV-002 is violated.

// WHY THIS MATTERS: Implicit decisions are the primary source of architectural drift. When an LLM encounters an undocumented choice, it may "improve" it by choosing differently — breaking invariants the original choice was protecting.

---

**INV-003: Invariant Falsifiability**

*Every invariant can be violated by a concrete scenario and detected by a named test.*

```
∀ inv ∈ Invariants:
  ∃ scenario: scenario.violates(inv) ∧
  ∃ test ∈ TestStrategy: test.detects(scenario)
```

Violation scenario: An invariant states "the system shall be performant" with no violation scenario and no test. It passes vacuously because no concrete failure can be identified.

Validation: For each invariant, construct a counterexample (a state or sequence of events that would violate it). If no such counterexample can be constructed, the invariant is either trivially true (remove it) or too vague (sharpen it).

// WHY THIS MATTERS: Unfalsifiable invariants create false confidence. They appear in the invariant count but constrain nothing. An LLM implementing against them will either ignore them (correct but wasteful) or hallucinate a testable interpretation (dangerous).

---

**INV-004: Algorithm Completeness**

*Every described algorithm includes: pseudocode, complexity analysis, at least one worked example, and error/edge case handling.*

```
∀ algorithm ∈ spec:
  algorithm.has(pseudocode) ∧
  algorithm.has(complexity_analysis) ∧
  algorithm.has(worked_example) ∧
  algorithm.has(edge_cases)
```

Violation scenario: A spec describes a "conflict resolution algorithm" in prose — "when two agents claim the same task, the system resolves the conflict" — but provides no pseudocode, no complexity analysis, no worked example of a specific conflict, and no edge case handling for three-way conflicts.

Validation: Mechanical check. Scan each algorithm section for the four required components. Missing components violate INV-004.

// WHY THIS MATTERS: Prose-only algorithm descriptions are the #1 source of LLM hallucination. The LLM will invent an algorithm that matches the prose but violates unstated invariants. Pseudocode pins the implementation; worked examples verify understanding; edge cases close gaps.

---

**INV-005: Performance Verifiability**

*Every performance claim is tied to a specific benchmark scenario, a design point, and a measurement methodology.*

```
∀ perf_claim ∈ spec:
  ∃ benchmark: perf_claim.measured_by(benchmark) ∧
  ∃ design_point: perf_claim.valid_at(design_point) ∧
  benchmark.has(methodology)
```

Violation scenario: A spec claims "the scheduler is fast enough for real-time use" without defining what "fast enough" means, what "real-time" means, what workload is assumed, or how to measure it. An implementer ships a scheduler that takes 50ms per dispatch — fine for some definitions of "real-time," unacceptable for others.

Validation: For each performance number, locate the benchmark that measures it. If the benchmark doesn't exist or doesn't describe how to run it, INV-005 is violated.

// WHY THIS MATTERS: Unmeasured performance claims are wishes. They cannot be verified during implementation, cannot be regression-tested, and cannot be used to make tradeoff decisions.

---

**INV-006: Cross-Reference Density**

*The specification contains a cross-reference web where no section is an island.*

```
∀ section ∈ spec (excluding Preamble, Glossary):
  section.outgoing_references.count ≥ 1 ∧
  section.incoming_references.count ≥ 1
```

Violation scenario: An implementation chapter for "Notification Service" contains no references to any invariant, ADR, or other chapter. It could be deleted without breaking any cross-reference, suggesting it is disconnected from the spec's causal chain.

Validation: Build a directed graph of cross-references. Every non-trivial section must have at least one inbound and one outbound edge. Orphan sections violate INV-006.

// WHY THIS MATTERS: Cross-references are the mechanism that prevents a spec from devolving into a collection of independent essays. They force the author to think about how each section serves the whole. For LLMs, explicit cross-references are the only reliable way to connect constraints to the sections they govern — LLMs cannot infer implicit connections.

---

**INV-007: Signal-to-Noise Ratio**

*Every section earns its place by serving at least one other section or preventing a named failure mode.*

```
∀ section ∈ spec:
  ∃ justification:
    (section.serves(other_section) ∨ section.prevents(named_failure_mode))
```

Violation scenario: A spec contains a 200-line "History of Event Sourcing" section that no other section references, no invariant depends on, and no implementer needs. It adds noise without signal.

Validation: For each section, state in one sentence why removing it would make the spec worse. If you cannot, remove the section.

// WHY THIS MATTERS: Every line in a spec competes for the implementer's attention — and for context window budget. Noise dilutes the signal from critical sections, increasing the chance that an LLM misses a constraint.

---

**INV-008: Self-Containment**

*The specification, combined with the implementer's general programming competence and domain knowledge available in public references, is sufficient to build a correct v1.*

```
∀ implementation_question Q:
  spec.answers(Q) ∨
  Q.answerable_from(general_competence ∪ public_references)
```

Violation scenario: A spec references "the standard conflict resolution protocol" without defining it or citing a public reference. The implementer must ask the architect what "standard" means — the spec has delegated a critical detail to oral tradition.

Validation: Give the spec to a competent engineer (or LLM) unfamiliar with the project. Track every question they ask. If questions reveal information that should be in the spec, INV-008 is violated.

// WHY THIS MATTERS: An LLM cannot ask clarifying questions. Every gap in the spec becomes a hallucination opportunity. Self-containment is the primary defense against LLM-generated incorrectness.

---

**INV-009: Glossary Coverage**

*Every domain-specific term used in the specification is defined in the glossary.*

```
∀ term ∈ spec where term.is_domain_specific:
  ∃ entry ∈ Glossary: entry.defines(term)
```

Violation scenario: A spec uses "saga" throughout to mean a specific coordination pattern, but the glossary never defines it. An LLM implementer uses the general distributed-systems meaning of "saga" (compensating transactions), which differs from the spec's intended meaning (long-running workflow).

Validation: Extract all non-common-English terms from the spec. Check each against the glossary.

// WHY THIS MATTERS: LLMs have strong priors about common technical terms. If the spec uses a term with a domain-specific meaning that differs from the common meaning, the LLM will use the common meaning unless the glossary explicitly overrides it.

---

**INV-010: State Machine Completeness**

*Every state machine defines all states, all transitions, all guards, and behavior for invalid transitions.*

```
∀ sm ∈ StateMachines:
  sm.has(all_states) ∧
  sm.has(all_transitions) ∧
  sm.has(guards_per_transition) ∧
  sm.has(invalid_transition_policy)
```

Violation scenario: A task state machine defines states {Ready, InProgress, Done} and transitions {start, complete} but omits the guard on `start` (what if the task's dependencies aren't met?) and says nothing about what happens if `complete` is called on a Ready task.

Validation: For each state machine, enumerate the state × event cross-product. Every cell must either name a transition or explicitly state "invalid — [policy]."

// WHY THIS MATTERS: Incomplete state machines are the second most common source of LLM implementation errors (after missing negative specifications). An LLM encountering an unspecified state×event combination will invent a "reasonable" behavior that may violate invariants.

---

**INV-011: Module Completeness** [Conditional — modular specs only]

*An LLM receiving a properly assembled bundle can implement the module's subsystem without information from any other module's implementation content.*

```
∀ module ∈ modules:
  let bundle = ASSEMBLE(module)
  ∀ implementation_question Q about module's subsystem:
    bundle.answers(Q) ∨ Q.answerable_from(general_competence)
```

Violation scenario: The Scheduler module references EventStore's internal ring buffer layout to determine batching strategy, but the ring buffer details live only in the EventStore module (not in the constitution or shared types).

Validation: Give a bundle (not the full spec) to an LLM. Track questions that require information from another module's implementation. Any such question violates INV-011.

// WHY THIS MATTERS: If module completeness fails, the modularization protocol provides no benefit. The entire value proposition is that bundles are sufficient.

---

**INV-012: Cross-Module Isolation** [Conditional — modular specs only]

*Modules reference each other only through constitutional elements (invariants, ADRs, shared types). No module contains direct references to another module's internal sections, algorithms, or data structures.*

```
∀ module_a, module_b ∈ modules where module_a ≠ module_b:
  ∀ ref ∈ module_a.outbound_references:
    ref.target ∉ module_b.internal_sections ∧
    ref.target ∈ {constitution, shared_types, invariants, ADRs}
```

Violation scenario: The TUI Renderer module says "use the same batching strategy as the EventStore module's flush_batch() function."

Validation: Mechanical (CHECK-7 in §0.13.11). Semantic: review for implicit references that bypass the constitution.

// WHY THIS MATTERS: If modules reference each other's internals, Module A's bundle needs Module B's implementation — defeating the purpose of modularization. The constitution is the "header file"; modules are "implementation files" that are never directly included. (Locked by ADR-007.)

---

**INV-013: Invariant Ownership Uniqueness** [Conditional — modular specs only]

*Every application invariant is maintained by exactly one module (or explicitly by the system constitution). No invariant is unowned or multiply-owned.*

```
∀ inv ∈ invariant_registry:
  (inv.owner = "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 0)
  ∨ (inv.owner ≠ "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 1)
```

Violation scenario: Both EventStore and SnapshotManager list APP-INV-017 in their maintains declarations. Which module's tests are authoritative for that invariant?

Validation: Mechanical (CHECK-1 in §0.13.11).

// WHY THIS MATTERS: Ownership uniqueness prevents accountability gaps. If two modules both claim to maintain an invariant, neither takes full responsibility for its test coverage.

---

**INV-014: Bundle Budget Compliance** [Conditional — modular specs only]

*Every assembled bundle fits within the hard ceiling defined in the manifest's context budget.*

```
∀ module ∈ modules:
  line_count(ASSEMBLE(module)) ≤ context_budget.hard_ceiling_lines
```

Violation scenario: Scheduler module grows to 3,500 lines. With 1,200-line constitutional context, the bundle is 4,700 lines — under the 5,000 hard ceiling but over the 4,000 target (WARN). If the bundle reaches 5,100 lines, INV-014 is violated (ERROR, assembly fails).

Validation: Mechanical (CHECK-5 in §0.13.11). Run the assembly script; it validates budget compliance automatically.

// WHY THIS MATTERS: The modularization protocol exists to keep bundles within LLM context budget. Budget violations mean the modularization added complexity without delivering the benefit.

---

**INV-015: Declaration-Definition Consistency** [Conditional — modular specs only]

*Every invariant declaration in the system constitution is a faithful summary of its full definition in the domain constitution.*

```
∀ inv ∈ invariant_registry:
  let decl = system_constitution.declaration(inv)
  let defn = full_definition(inv)
  decl.id = defn.id ∧
  decl.one_line is_faithful_summary_of defn.statement
```

Violation scenario: System constitution declares "APP-INV-017: Event log is append-only" but the Storage domain definition now says "append-only except during compaction windows." An LLM implementing a different domain sees only the declaration and codes against the wrong contract.

Validation: Semi-mechanical. Extract declaration/definition pairs, present to reviewer for semantic consistency.

// WHY THIS MATTERS: Divergence between tiers means different modules are implemented against different understandings of the same invariant. The declaration is the API; the definition is the implementation — they must agree.

---

**INV-016: Manifest-Spec Synchronization** [Conditional — modular specs only]

*The manifest accurately reflects the current state of all spec files.*

```
∀ path ∈ manifest.all_referenced_paths: file_exists(path)
∀ inv ∈ manifest.all_referenced_invariants: inv ∈ system_constitution
∀ module_file ∈ filesystem("modules/"): module_file ∈ manifest
```

Violation scenario: Author adds `modules/new_feature.md` but forgets to add it to the manifest. The assembly script never produces a bundle for it. The RALPH loop never improves it.

Validation: Mechanical (CHECK-9 in §0.13.11).

// WHY THIS MATTERS: The manifest is the single source of truth for module topology. A file that exists but isn't in the manifest is invisible to all tooling — assembly, validation, improvement loops, cascade analysis.

---

**INV-017: Negative Specification Coverage**

*Every implementation chapter includes explicit "do NOT" constraints for the most plausible misinterpretations of that subsystem's behavior.*

```
∀ chapter ∈ implementation_chapters:
  chapter.has(negative_specifications) ∧
  chapter.negative_specifications.count ≥ 2 ∧
  ∀ neg ∈ chapter.negative_specifications:
    neg.is_plausible_misinterpretation ∧ neg.states_what_not_to_do
```

Violation scenario: An EventStore implementation chapter describes the append-only log but includes no negative specifications. An LLM implementer adds a "helpful" compaction routine that rewrites old events to save space — a plausible optimization that violates the append-only invariant. A negative specification stating "do NOT implement log compaction or event rewriting" would have prevented this.

Validation: For each implementation chapter, check that at least 2 negative specifications exist. For each negative specification, verify that it addresses a plausible misinterpretation (not an absurd one). Test: give the chapter without negative specs to an LLM and observe what it adds unprompted — those additions are candidates for negative specifications.

// WHY THIS MATTERS: LLMs fill specification gaps with plausible behavior. Negative specifications close the most dangerous gaps — the ones where the plausible behavior violates invariants. This is the single highest-leverage improvement for LLM implementation correctness. (Locked by ADR-008.)

---

**INV-018: Structural Predictability**

*Every element of the same type follows the same format template throughout the specification.*

```
∀ type ∈ {invariant, ADR, algorithm, worked_example, negative_spec}:
  ∃ template(type) ∧
  ∀ instance ∈ spec.elements_of(type):
    instance.follows(template(type))
```

Violation scenario: ADR-001 through ADR-005 follow the Problem/Options/Decision/Consequences/Tests template, but ADR-006 uses a different format with "Rationale" instead of "Decision" and omits the "Tests" section. An LLM writing a new conforming spec reproduces the inconsistency, sometimes using one format, sometimes the other.

Validation: Extract all elements of each type. Compare their structure against the declared template (§3.4 for invariants, §3.5 for ADRs, etc.). Deviations violate INV-018.

// WHY THIS MATTERS: LLMs learn from structural patterns in the spec. Inconsistent formatting within a spec degrades the LLM's ability to produce consistently formatted output. Fixed formats reduce variance in LLM output quality. (Locked by ADR-009.)

---

**INV-019: Implementation Ordering Explicitness**

*Dependencies between subsystems for implementation ordering are explicitly stated with rationale.*

```
∀ subsystem_a, subsystem_b ∈ spec where subsystem_a.depends_on(subsystem_b):
  ∃ ordering_directive ∈ spec:
    ordering_directive.states(subsystem_b before subsystem_a) ∧
    ordering_directive.has(rationale)
```

Violation scenario: A spec describes both an EventStore and a Scheduler. The Scheduler depends on EventStore for event persistence, but this dependency is never stated. An LLM implements the Scheduler first, invents a mock EventStore API that doesn't match the actual EventStore's interface, and the two subsystems don't integrate.

Validation: Extract all subsystem dependency relationships. For each dependency, verify that an explicit ordering directive exists (in §6.1.4 or §5.7 meta-instructions). Missing directives violate INV-019.

// WHY THIS MATTERS: LLMs cannot infer implementation ordering from implicit dependencies the way experienced engineers can. Explicit ordering prevents integration failures from subsystems implemented against incompatible assumptions.

---

**INV-020: Restatement Freshness**

*When a key invariant or constraint is restated at point of use (structural redundancy), the restatement is semantically identical to the canonical definition.*

```
∀ restatement ∈ spec where restatement.restates(canonical_definition):
  restatement.semantic_content = canonical_definition.semantic_content
```

Violation scenario: §0.5 defines INV-003 as "same event sequence → identical final state." An implementation chapter restates this as "events are processed deterministically" — a subtle weakening that omits the requirement for identical state. An LLM implementing from the chapter uses the weaker interpretation.

Validation: Collect all restatements (identified by phrases like "recall INV-NNN" or "as required by INV-NNN"). Compare each to its canonical source. Semantic divergence violates INV-020.

// WHY THIS MATTERS: Structural redundancy is valuable for LLM consumption — it ensures critical constraints are present at point of use. But stale restatements are worse than no restatement, because they create contradictions within the spec. The LLM may resolve the contradiction in the wrong direction.

---

**INV-021: Reference Syntax Standardization**

*All cross-references in the specification use one of the standard DDIS reference forms.*

```
∀ reference ∈ spec:
  reference.format ∈ {
    "(see §N.M)",                    // section reference
    "(INV-NNN)",                     // invariant reference
    "(ADR-NNN)",                     // ADR reference
    "(Benchmark B-NNN)",             // benchmark reference
    "(Glossary: "term")",            // glossary reference
    "(Gate N)",                      // quality gate reference
    "(EXT:spec-name:INV-NNN)",       // cross-spec reference (§0.14)
    "(validated by INV-NNN)",        // validation reference
    "(locked by ADR-NNN)",           // decision lock reference
    "(measured by Benchmark B-NNN)", // measurement reference
    "(defined in Glossary: "term")", // glossary definition reference
  }
```

Violation scenario: A spec uses "as discussed in the architecture section" instead of "(see §0.3)". An LLM implementing from the spec cannot locate the referenced content and either hallucinates a plausible interpretation or ignores the reference entirely.

Validation: Mechanical check. Extract all reference-like phrases from the spec. Flag any that don't match one of the standard forms. Informal references ("see above", "as mentioned earlier", "the previous section") are violations.

// WHY THIS MATTERS: Standardized references serve as lookup keys for LLMs. An LLM can reliably resolve "(INV-003)" to the invariant definition. It cannot reliably resolve "the determinism invariant we discussed earlier." Machine-parseable references also enable automated graph construction, stale restatement detection (INV-020), and orphan detection (INV-006). (Locked by ADR-011.)

---

**INV-022: Conditional Section Compliance**

*Every section marked [Conditional] or [Optional] has an explicit trigger condition, and specs that meet the trigger include the section.*

```
∀ section ∈ spec where section.is_conditional:
  section.has(trigger_condition) ∧
  trigger_condition.is_testable ∧
  (trigger_condition.met → section.present) ∧
  (¬trigger_condition.met → section.marked_as_not_applicable)
```

Violation scenario: A spec for a multi-service system omits §0.13 (Modularization Protocol) even though it exceeds 4,000 lines — the trigger condition is met but the section is absent. An LLM implementing from the spec has no guidance on how to consume the oversized document and begins losing context in later chapters.

Validation: For each conditional section in §0.3, check: (1) trigger condition is stated, (2) the spec either includes the section or explicitly states "not applicable because [condition not met]."

// WHY THIS MATTERS: Conditional sections create an implicit contract — "you need this if X." Without explicit triggers, authors omit sections they need or include sections they don't. LLMs benefit from explicit applicability logic because they can skip inapplicable sections confidently rather than guessing. (Locked by ADR-011.)

---