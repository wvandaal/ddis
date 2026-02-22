# DDIS: Decision-Driven Implementation Specification Standard

## Version 1.0 — A Self-Bootstrapping Meta-Specification

> Design goal: **A formal standard for writing implementation specifications that are precise enough for an LLM or junior engineer to implement correctly without guessing, while remaining readable enough that a senior engineer would choose to read them voluntarily.**

> Core promise: A specification conforming to DDIS contains everything needed to implement the described system — architecture, algorithms, invariants, decisions, test strategies, performance budgets, and execution plan — in a single cohesive document where every section earns its place by serving the sections around it.

> Document note (important):
> This standard is **self-bootstrapping**: it is written in the format it defines.
> Every structural element prescribed by DDIS is demonstrated in this document.
> Where this document says "the spec must include X," this document includes X — about itself.
> Code blocks are design sketches for illustration. The correctness contract lives in the
> invariants, not in any particular syntax.

> How to use this standard (practical):
> 1) Read **PART 0** once end-to-end: understand what DDIS requires, why, and how elements connect.
> 2) Lock your spec's **churn-magnets** via ADRs before writing implementation sections.
> 3) Write your spec following the **Document Structure** (§0.3), using PART II as the element-by-element reference.
> 4) Validate against the **Quality Gates** (§0.7) and the **Completeness Checklist** (Part X) before considering the spec "done."
> 5) Treat the **cross-reference web** as a product requirement, not polish — it is the mechanism that makes the spec cohere.
> 6) If your spec exceeds **2,500 lines** or your target LLM's context window, read **§0.13 (Modularization Protocol)** and decompose into a manifest-driven module structure.

---

# PART 0: EXECUTIVE BLUEPRINT

## 0.1 Executive Summary

DDIS (Decision-Driven Implementation Specification) is a standard for writing technical specifications that bridge the gap between architectural vision and correct implementation.

Most specifications fail in one of two ways: they are too abstract (the implementer must guess at critical details) or too mechanical (they prescribe code without explaining why, making evolution impossible). DDIS avoids both failure modes by requiring a **causal chain** from first principles through decisions to implementation details, where every element justifies its existence by serving the elements around it.

DDIS synthesizes techniques from several traditions — Architecture Decision Records (Nygard), Design by Contract (Meyer), temporal invariants (Lamport), performance budgeting (game engine development), and test-driven specification — into a unified document structure. The synthesis is the contribution: these techniques are well-known individually but rarely composed into a single coherent standard.

### 0.1.1 What DDIS Is

DDIS is a document standard. It specifies:

- What structural elements a specification must contain
- How those elements must relate to each other (the cross-reference web)
- What quality criteria each element must meet
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

### 0.1.3 Non-Goals (Explicit)

DDIS does not attempt:

- **To replace code.** A spec is not an implementation. It describes what to build, why, and how to verify it — not the literal source code. Design sketches illustrate intent; they are not copy-paste targets.
- **To eliminate judgment.** Implementers will make thousands of micro-decisions. DDIS constrains the macro-decisions (architecture, algorithms, invariants) so micro-decisions are locally safe.
- **To be a project management framework.** DDIS includes a Master TODO and phased roadmap, but these are execution aids for the spec's content, not a substitute for sprint planning or issue tracking.
- **To prescribe notation.** DDIS requires formal models but does not mandate TLA+, Alloy, Z, or any specific formalism. Pseudocode, state machine diagrams, mathematical notation, or "close to [language]" sketches are all acceptable if they are precise.
- **To guarantee correctness.** A DDIS-conforming spec dramatically reduces the chance of building the wrong thing. It cannot eliminate it. The spec is a contract for human (or LLM) intent, not a machine-checked proof.

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

1. **Completeness over elegance.** A verbose spec that leaves nothing ambiguous is better than a terse spec that leaves critical details to inference. (But see INV-07: verbosity without structure is noise.)

2. **Decisions over descriptions.** The hardest part of building a system is not writing code — it is making the hundreds of design decisions that determine whether the code is correct. A spec that describes a system without recording why it is shaped that way is a snapshot, not a blueprint.

3. **Verifiability over trust.** Every claim in the spec must be testable. "The system is fast" is not verifiable. "Event ingestion completes in < 100µs p99 at the design point of 300 agents / 10K tasks, measured by Benchmark B-001" is verifiable.

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

```
First Principles (formal model of the problem)
  ↓ justifies
Non-Negotiables + Invariants (what must always be true)
  ↓ constrained by
Architecture Decision Records (choices that could go either way)
  ↓ implemented via
Algorithms + Data Structures + Protocols (pseudocode, state machines)
  ↓ verified by
Test Strategies + Performance Budgets (how we know it's correct and fast)
  ↓ shipped via
Quality Gates + Master TODO (stop-ship criteria, execution checklist)
```

Every element in DDIS exists because removing it causes a specific, named failure. There are no decorative sections.

### 0.2.3 Fundamental Operations of a Specification

Every specification, regardless of domain, performs these operations:

| Operation | What It Does | DDIS Element |
|---|---|---|
| **Define** | Establish what the system IS, formally | First-principles model, formal types |
| **Constrain** | State what must always hold | Invariants, non-negotiables |
| **Decide** | Lock choices where alternatives exist | ADRs |
| **Describe** | Specify how components work | Algorithms, state machines, protocols |
| **Exemplify** | Show the system in action | Worked examples, end-to-end traces |
| **Bound** | Set measurable limits | Performance budgets, design point |
| **Verify** | Define how to confirm correctness | Test strategies, quality gates |
| **Exclude** | State what the system is NOT | Non-goals, scope boundaries |
| **Sequence** | Order the work | Phased roadmap, decision spikes, first PRs |
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
  §0.13 Modularization Protocol (specs > context window)        [Conditional]

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
    - Worked example(s)
    - WHY NOT annotations on non-obvious choices
    - Test strategy
    - Performance budget for this subsystem
    - Cross-references to ADRs, invariants, other subsystems
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
    - Immediate next steps (first PRs)
  Agent/tool compatibility notes                           [Optional]

APPENDICES
  A: Glossary (every domain term, cross-referenced)
  B: Risk Register (risks + mitigations)
  C: Storage / Wire Formats                                [Optional]
  D: Benchmark Scenarios                                   [Optional]
  E: Reference Implementations / Extracted Code            [Optional]

PART X: MASTER TODO INVENTORY
  Checkboxable task list organized by subsystem
  Cross-referenced to phases, ADRs, and quality gates
```

### 0.3.1 Ordering Rationale

The ordering is not arbitrary. It follows the **dependency chain of understanding**:

1. First principles establish vocabulary and the formal model
2. Invariants constrain what the system may do
3. ADRs lock the choices within those constraints
4. Implementation chapters describe how, within those locked choices
5. Interfaces describe the system's boundaries
6. Operations describe how to build and verify it
7. Appendices provide reference material

An implementer reading top-to-bottom builds understanding incrementally. No section requires forward references to be understood (backward references are expected and encouraged).

## 0.4 This Standard's Architecture

DDIS has a simple ring architecture:

1. **Core Standard (sacred)**: The mandatory structural elements, their required contents, quality criteria, and relationships. (PART 0, PART I, PART II of this document.)

2. **Guidance (recommended)**: Voice, proportional weight, anti-patterns, worked examples. These improve spec quality but their absence does not make a spec non-conforming. (PART III of this document.)

3. **Tooling (optional)**: Checklists, templates, validation procedures. (PART IV, Appendices.)

## 0.5 Invariants of the DDIS Standard

Every DDIS-conforming specification must satisfy these invariants. Each invariant has an identifier, a plain-language statement, a formal expression, and a validation method.

---

**INV-001: Causal Traceability**

*Every implementation section traces to at least one ADR or invariant, which traces to the formal model.*

```
∀ section ∈ PART_II:
  ∃ adr ∈ ADRs ∪ inv ∈ Invariants:
    section.references(adr ∨ inv) ∧ (adr ∨ inv).derives_from(formal_model)
```

Validation: Manual audit. Pick 5 random implementation sections. For each, follow cross-references backward to an ADR or invariant, then to the formal model. If any chain breaks, INV-001 is violated.

// WHY THIS MATTERS: Without traceability, sections accumulate by accretion ("add a caching layer") without justification. Six months later, nobody knows if the caching layer can be removed.

---

**INV-002: Decision Completeness**

*Every design choice where a reasonable alternative exists is captured in an ADR.*

```
∀ choice ∈ spec where ∃ alternative ∧ alternative.is_reasonable:
  ∃ adr ∈ ADRs: adr.covers(choice) ∧ adr.alternatives.contains(alternative)
```

Validation: Adversarial review. A reviewer reads each implementation section and asks "could this reasonably be done differently?" If yes and no ADR exists, INV-002 is violated.

---

**INV-003: Invariant Falsifiability**

*Every invariant can be violated by a concrete scenario and detected by a named test.*

```
∀ inv ∈ Invariants:
  ∃ scenario: scenario.violates(inv) ∧
  ∃ test ∈ TestStrategy: test.detects(scenario)
```

Validation: For each invariant, construct a counterexample (a state or sequence of events that would violate it). If no such counterexample can be constructed, the invariant is either trivially true (remove it) or too vague (sharpen it).

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

Validation: Mechanical check. Scan each algorithm section for the four required components.

---

**INV-005: Performance Verifiability**

*Every performance claim is tied to a specific benchmark scenario, a design point, and a measurement methodology.*

```
∀ perf_claim ∈ spec:
  ∃ benchmark: perf_claim.measured_by(benchmark) ∧
  ∃ design_point: perf_claim.valid_at(design_point) ∧
  benchmark.has(methodology)
```

Validation: For each performance number, locate the benchmark that measures it. If the benchmark doesn't exist or doesn't describe how to run it, INV-005 is violated.

---

**INV-006: Cross-Reference Density**

*The specification contains a cross-reference web where no section is an island.*

```
∀ section ∈ spec (excluding Preamble, Glossary):
  section.outgoing_references.count ≥ 1 ∧
  section.incoming_references.count ≥ 1
```

Validation: Build a directed graph of cross-references. Every non-trivial section must have at least one inbound and one outbound edge. Orphan sections violate INV-006.

// WHY THIS MATTERS: Cross-references are the mechanism that prevents a spec from devolving into a collection of independent essays. They force the author to think about how each section serves the whole.

---

**INV-007: Signal-to-Noise Ratio**

*Every section earns its place by serving at least one other section or preventing a named failure mode.*

```
∀ section ∈ spec:
  ∃ justification:
    (section.serves(other_section) ∨ section.prevents(named_failure_mode))
```

Validation: For each section, state in one sentence why removing it would make the spec worse. If you cannot, remove the section.

---

**INV-008: Self-Containment**

*The specification, combined with the implementer's general programming competence and domain knowledge available in public references, is sufficient to build a correct v1.*

```
∀ implementation_question Q:
  spec.answers(Q) ∨
  Q.answerable_from(general_competence ∪ public_references)
```

Validation: Give the spec to a competent engineer unfamiliar with the project. Track every question they ask. If questions reveal information that should be in the spec, INV-008 is violated.

---

**INV-009: Glossary Coverage**

*Every domain-specific term used in the specification is defined in the glossary.*

```
∀ term ∈ spec where term.is_domain_specific:
  ∃ entry ∈ Glossary: entry.defines(term)
```

Validation: Extract all non-common-English terms from the spec. Check each against the glossary.

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

Validation: For each state machine, enumerate the state × event cross-product. Every cell must either name a transition or explicitly state "invalid — [policy]."

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

## 0.6 Architecture Decision Records

### ADR-001: Document Structure Is Fixed, Not Flexible

#### Problem

Should DDIS prescribe a fixed document structure, or allow authors to organize freely as long as content requirements are met?

#### Options

A) **Fixed structure** (prescribed section ordering and hierarchy)
- Pros: Predictable for readers; mechanical completeness checking; easier to teach.
- Cons: May feel rigid; some domains fit the structure better than others.

B) **Content requirements only** (prescribe what, not where)
- Pros: Flexibility; authors can organize by whatever axis makes sense.
- Cons: Every spec is a unique snowflake; readers must re-learn structure each time; harder to validate.

C) **Fixed skeleton with flexible interior** (prescribed top-level parts, flexible chapter organization within)
- Pros: Balance of predictability and flexibility.
- Cons: The "flexible interior" often means "no structure at all."

#### Decision

**Option A: Fixed structure.** The value of DDIS is that a reader who has seen one DDIS spec can navigate any other DDIS spec. This is worth the cost of occasionally awkward section placement.

The structure may be renamed (e.g., "Kernel Invariants" instead of "Invariants") and domain-specific sections may be added within any PART, but the required elements (§0.3) must appear, and the PART ordering must be preserved.

#### Consequences

- Authors must sometimes figure out where a domain-specific concept "lives" in the DDIS structure
- Readers gain predictability and can skip to known locations
- Validation tools can check structural conformance mechanically

#### Tests

- (Validated by INV-001, INV-006) If an author places content in an unexpected location, cross-references will either break or become strained, surfacing the misplacement.

---

### ADR-002: Invariants Must Be Falsifiable, Not Merely True

#### Problem

Should invariants be aspirational properties ("the system should be fast") or formal contracts with concrete violation scenarios?

#### Options

A) **Aspirational invariants** (state desired properties in natural language)
- Pros: Easy to write; captures intent.
- Cons: Cannot be tested; cannot be violated; useless for verification.

B) **Formal invariants with proof obligations** (TLA+-style temporal logic)
- Pros: Machine-checkable; mathematically rigorous.
- Cons: Requires formal methods expertise; most implementers can't read them; high authoring cost.

C) **Falsifiable invariants** (formal enough to test, informal enough to read)
- Pros: Each invariant has a concrete counterexample and a test; readable by working engineers.
- Cons: Not machine-checkable; relies on human judgment for completeness.

#### Decision

**Option C: Falsifiable invariants.** Every invariant must include: a plain-language statement, a semi-formal expression (pseudocode, predicate logic, or precise English), a violation scenario (how could this break?), and a validation method (how do we test it?).

// WHY NOT Option B? Because the goal is implementation correctness by humans and LLMs, not machine-checked proofs. The authoring cost of full formal verification exceeds the benefit for most systems. If a domain requires machine-checked invariants, the DDIS spec can reference the external formal model.

#### Consequences

- Invariants are immediately actionable as test cases
- The violation scenario forces the author to think adversarially
- Some subtle properties may be hard to express in this format

#### Tests

- (Validated by INV-003) Every invariant in a DDIS spec must have a constructible counterexample.

---

### ADR-003: Cross-References Are Mandatory, Not Optional Polish

#### Problem

Should cross-references between sections be recommended or required?

#### Options

A) **Recommended** — encourage authors to add cross-references where helpful.
B) **Required** — every non-trivial section must have inbound and outbound references.

#### Decision

**Option B: Required.** Cross-references are the mechanism that transforms a collection of sections into a unified specification. Without them, sections exist in isolation and the causal chain (INV-001) cannot be verified.

#### Consequences

- Higher authoring cost (every section requires thinking about its relationships)
- Much higher reader value (any section can be understood in context)
- Enables graph-based validation of spec completeness

#### Tests

- (Validated by INV-006) Build the reference graph; no orphan sections.

---

### ADR-004: Self-Bootstrapping as Validation Strategy

#### Problem

How do we validate that the DDIS standard itself is coherent and complete?

#### Options

A) **External validation** — write the standard in prose, validate by review.
B) **Self-bootstrapping** — write the standard in its own format, validate by self-conformance.

#### Decision

**Option B: Self-bootstrapping.** This document is both the standard and its first conforming instance. If the standard is unclear, the author discovers this while attempting to apply it to itself. If the standard is incomplete, the self-application reveals the gap.

// WHY NOT Option A? Because a standard that cannot be applied to itself is suspect. If the structure is good enough for implementation specs, it is good enough for a meta-spec. Self-application is the ultimate dog-fooding.

#### Consequences

- The standard is simultaneously more trustworthy (tested by self-application) and more complex (meta-level and object-level interleave)
- Readers may initially find the self-referential nature disorienting
- The document serves as both reference and example

#### Tests

- This document passes its own Quality Gates (§0.7) and Completeness Checklist (Part X).

---

### ADR-005: Voice Is Specified, Not Left to Author Preference

#### Problem

Should DDIS prescribe the writing voice of conforming specifications?

#### Options

A) **No voice guidance** — let authors write in whatever tone suits them.
B) **Voice guidance** — specify tone, provide examples, define anti-patterns.

#### Decision

**Option B: Voice guidance.** Specifications fail when they are either too dry to read or too casual to trust. DDIS prescribes a specific voice: technically precise but human, the voice of a senior engineer explaining their system to a peer they respect. (See §2.1 for full guidance.)

#### Consequences

- Specs feel more unified and readable
- Authors must sometimes revise natural writing habits
- LLMs benefit significantly from explicit voice guidance (reduces generic boilerplate)

#### Tests

- Qualitative review: sample 5 sections, assess whether each sounds like a senior engineer talking to a peer. If any sounds like a textbook, marketing copy, or bureaucratic report, the voice is wrong.

---

### ADR-006: Tiered Constitution over Flat Root [Conditional — modular specs only]

#### Problem

When a DDIS spec is modular for context-window compliance (§0.13), constitutional context must accompany every module bundle. How should this constitutional context be structured?

#### Options

A) **Flat root** — one file containing everything (all invariant definitions, all ADR analysis, all shared types).
- Pros: Simple; one file to maintain; no tier logic.
- Cons: Doesn't scale past ~20 invariants / ~10 ADRs. Against a FrankenTUI-scale spec (25 invariants, 15 ADRs, 4,800 lines), the flat root alone is ~1,500 lines, leaving only 2,500 for the module.

B) **Two-tier** — system constitution (full definitions) + modules.
- Pros: Simple; works for small modular specs (< 20 invariants, system constitution ≤ 400 lines).
- Cons: System constitution grows linearly with invariant count; exceeds budget at medium scale.

C) **Three-tier** — system constitution (declarations only, 200–400 lines) + domain constitution (full definitions, 200–500 lines per domain) + cross-domain deep context (0–600 lines, per-module) + module.
- Pros: Scales to large specs; domain grouping is already present in well-architected systems (double duty); no duplication between tiers.
- Cons: One additional level of indirection; requires domain identification.

#### Decision

**Option C as the full protocol, with Option B as a blessed simplification** for small specs (< 20 invariants, system constitution ≤ 400 lines). The `tier_mode` field in the manifest selects between them. This avoids forcing three-tier complexity on specs that don't need it while providing a clear upgrade path.

// WHY NOT Option A? At FrankenTUI scale, the flat root consumes 30–37% of the context budget before the module even starts. That's not "context management" — it's context waste.

#### Consequences

- Authors must identify 2–5 architectural domains when modularization (usually obvious from the architecture overview)
- Two-tier specs can migrate to three-tier without restructuring modules (§0.13.14)
- The domain boundary serves double duty: isolation mechanism in the architecture and context management mechanism in the spec

#### Tests

- (Validated by INV-014) Bundle budget compliance confirms that the chosen tier mode keeps bundles within ceiling.
- (Validated by INV-011) Module completeness confirms that the constitutional context in each bundle is sufficient.

---

### ADR-007: Cross-Module References Through Constitution Only [Conditional — modular specs only]

#### Problem

When a DDIS spec is modular, how should modules reference content in other modules?

#### Options

A) **Direct references** — "see section 7.3 in the Scheduler module."
- Pros: Natural; mirrors how monolithic cross-references work.
- Cons: Creates invisible dependencies between modules. If Module A references Module B's internals, Module A's bundle needs Module B — defeating the purpose of modularization. Violates INV-011.

B) **Through constitution only** — Module A references APP-INV-032, which lives in the constitution. Module A never references Module B's internal sections.
- Pros: Enforces isolation mechanically; the constitution is the "header file" and modules are "implementation files"; bundles are self-contained.
- Cons: Authors must extract all cross-module contracts into the constitution; can feel indirect for tightly coupled subsystems.

#### Decision

**Option B: Through constitution only.** INV-012 enforces this mechanically. Cross-module contracts are expressed as invariants or shared types in the constitution, never as references to another module's algorithms, state machines, or data structures.

// WHY NOT Option A? It breaks INV-011 (module completeness). If Module A references Module B's internals, Module A's bundle needs Module B's implementation content — the very thing modularization was designed to avoid.

#### Consequences

- All cross-module contracts must be elevated to the constitution (invariants, shared types, or interface descriptions)
- Modules become truly self-contained implementation units
- Tight coupling between subsystems becomes visible in the constitution's interface surface

#### Tests

- (Validated by INV-012) Mechanical check (CHECK-7 in §0.13.11) scans modules for direct cross-module references.
- (Validated by INV-011) LLM bundle sufficiency test confirms modules don't need each other's content.

---

## 0.7 Quality Gates

A DDIS-conforming specification is "done" when all quality gates pass. Gates are ordered by priority; a failing Gate 1 makes Gates 2–6 irrelevant.

**Gate 1: Structural Conformance**
All required elements from §0.3 are present. Mechanical check.

**Gate 2: Causal Chain Integrity**
Five randomly selected implementation sections trace backward to the formal model without breaks. (Validates INV-001.)

**Gate 3: Decision Coverage**
An adversarial reviewer identifies zero "obvious alternatives" not covered by an ADR. (Validates INV-002.)

**Gate 4: Invariant Falsifiability**
Every invariant has a constructible counterexample and a named test. (Validates INV-003.)

**Gate 5: Cross-Reference Web**
The reference graph has no orphan sections and the graph is connected. (Validates INV-006.)

**Gate 6: Implementation Readiness**
A competent implementer (or LLM), given only the spec and public references, can begin implementing without asking clarifying questions about architecture, algorithms, data models, or invariants. Questions about micro-level implementation details (variable names, error message wording) are acceptable.

### Modularization Quality Gates [Conditional — modular specs only]

In addition to Gates 1–6, modular specs must pass these gates. A failing Gate M-1 makes Gates M-2 through M-5 irrelevant.

**Gate M-1: Consistency Checks**
All nine mechanical checks (CHECK-1 through CHECK-9 in §0.13.11) pass with zero errors. (Validates INV-012, INV-013, INV-014, INV-016.)

**Gate M-2: Bundle Budget Compliance**
Every assembled bundle is under the hard ceiling. Fewer than 20% of bundles exceed the target line count. (Validates INV-014.)

**Gate M-3: LLM Bundle Sufficiency**
An LLM receiving one assembled bundle produces zero questions that require another module's implementation content. Tested on at least 2 representative modules. (Validates INV-011.)

**Gate M-4: Declaration-Definition Faithfulness**
Every Tier 1 invariant declaration is a faithful summary of its Tier 2 full definition. (Validates INV-015.)

**Gate M-5: Cascade Simulation**
A simulated change to one invariant correctly identifies all affected modules via the cascade protocol (§0.13.12). (Validates INV-016 and the manifest's invariant registry.)

### Definition of Done (for this standard)

DDIS 1.0 is "done" when:
- This document passes Gates 1–6 applied to itself
- At least one non-trivial specification has been written conforming to DDIS and the author reports that the standard was sufficient (no structural gaps required working around)
- The Glossary (Appendix A) covers all DDIS-specific terminology

## 0.8 Performance Budgets (for Specifications, Not Software)

Specifications have performance characteristics too. A spec that takes 40 hours to read is too long. A spec that takes 2 hours to read probably omits critical details.

### 0.8.1 Specification Size Budgets

| System Complexity | Target Spec Length | Rationale |
|---|---|---|
| Small (single crate, < 5K LOC target) | 500–1,500 lines | Enough for formal model + invariants + key ADRs |
| Medium (multi-crate, 5K–50K LOC target) | 1,500–5,000 lines | Full DDIS treatment |
| Large (multi-service, > 50K LOC target) | 5,000–15,000 lines | May split into sub-specs linked by a master |

### 0.8.2 Proportional Weight Guide

Not all PART sections are equal. The following proportions prevent bloat in some areas and starvation in others. These are guidelines — domain-specific specs may adjust by ±20%.

| Section | % of Total | Why |
|---|---|---|
| Preamble + PART 0 | 15–20% | Dense: formal model, invariants, ADRs, quality gates |
| PART I: Foundations | 8–12% | First principles, state machines, complexity analysis |
| PART II: Core Implementation | 35–45% | THE HEART: algorithms, data structures, protocols, examples |
| PART III: Interfaces | 8–12% | API schemas, adapters, external contracts |
| PART IV: Operations | 10–15% | Testing, operational playbook, roadmap |
| Appendices + Part X | 10–15% | Reference material, glossary, master TODO |

### 0.8.3 Authoring Time Budgets

These are rough guides for experienced authors, validated against the FrankenTUI and Swarm Kernel specs:

| Element | Expected Authoring Time | Notes |
|---|---|---|
| First-principles model | 2–4 hours | Hardest part; requires deep domain understanding |
| One invariant (high quality) | 15–30 minutes | Including violation scenario and test strategy |
| One ADR (high quality) | 30–60 minutes | Including genuine alternative analysis |
| One implementation chapter | 2–4 hours | Including algorithm, examples, test strategy |
| End-to-end trace | 1–2 hours | Requires all subsystems to be drafted first |
| Glossary | 1–2 hours | Best done last, by extracting terms from the full spec |

---

## 0.9 Public API Surface (of DDIS Itself)

DDIS exposes the following "API" to specification authors:

1. **Document Structure Template** (§0.3) — the skeleton to fill in.
2. **Element Specifications** (PART II) — what each structural element must contain.
3. **Quality Criteria** (§0.5 invariants, §0.7 gates) — how to validate conformance.
4. **Voice and Style Guide** (PART III, §2.1) — how to write well within the structure.
5. **Anti-Pattern Catalog** (PART III, §2.3) — what bad specs look like.
6. **Completeness Checklist** (Part X) — mechanical conformance validation.

---

## 0.10 Open Questions (for DDIS 2.0)

1. **Machine-readable cross-references**: Should DDIS define a syntax for cross-references that enables automated graph construction? (Currently left to author convention.)

2. **Multi-document specs**: For very large systems, how should sub-specs reference each other? What invariants apply across spec boundaries?

3. **Spec evolution**: How should a DDIS spec handle versioning? When an ADR is superseded, what happens to sections that referenced the old decision?

4. **Formal verification bridge**: Should DDIS define a pathway from falsifiable invariants to machine-checked properties for safety-critical systems?

---

## 0.13 Modularization Protocol [Conditional]

This section is REQUIRED when the monolithic specification exceeds 4,000 lines or when the target context window (model-dependent) cannot hold the full spec plus a meaningful working budget for LLM reasoning. It is OPTIONAL but recommended for specs between 2,500–4,000 lines.

> Namespace note: INV-001 through INV-016 and ADR-001 through ADR-007 are DDIS meta-standard invariants/ADRs (defined in this standard). Application specs using DDIS define their OWN invariant namespace (e.g., APP-INV-001) — never reuse the meta-standard's INV-NNN space. Examples in this section use APP-INV-NNN to demonstrate this convention.

### 0.13.1 The Scaling Problem

A DDIS spec's value depends on the implementer holding sufficient context to produce correct output without guessing. When the spec exceeds the implementer's context window, two failure modes emerge:

1. **Truncation**: The LLM silently drops content from the beginning of the context, losing invariants and the formal model — the very elements that prevent hallucination.

2. **Naive splitting**: Arbitrary file splits break cross-references, orphan invariants from the sections they constrain, and force the LLM to guess at contracts defined in unseen sections.

The modularization protocol prevents both failures by defining a principled decomposition with formal completeness guarantees. (Motivated by INV-008: Self-Containment.)

### 0.13.2 Core Concepts

**Monolith**: A DDIS spec that exists as a single document. All specs start as monoliths. Most small-to-medium specs remain monoliths.

**Module**: A self-contained unit of the spec covering one major subsystem. Each module corresponds to one chapter of PART II in the monolithic structure. A module is never read alone — it is always assembled into a bundle with the appropriate constitutional context.

**Constitution**: The cross-cutting material that constrains all modules. Contains the formal model, invariants, ADRs, quality gates, architecture overview, glossary, and performance budgets. Organized in tiers to manage its own size.

**Domain**: An architectural grouping of related modules that share tighter coupling with each other than with modules in other domains. Domains correspond to rings, layers, or crate groups in the architecture overview.

**Bundle**: The assembled document sent to an LLM for implementation. Always contains: system constitution + domain constitution + cross-domain deep context + the module itself. A bundle is the unit of LLM consumption.

**Manifest**: A machine-readable YAML file that declares all modules, their domain membership, invariant ownership, cross-module interfaces, and assembly rules. The manifest is the single source of truth for the assembly script.

(All terms defined in Glossary, Appendix A.)

### 0.13.3 The Tiered Constitution

The constitution is organized in three tiers to prevent it from becoming a bottleneck itself. Each tier has a hard line budget, a clear scope, and NO overlapping content between tiers. (Locked by ADR-006.)

```
+--------------------------------------------------------------+
| TIER 1: System Constitution (200-400 lines, always)          |
|  - Design goal, core promise, non-negotiables, non-goals     |
|  - Architecture overview + domain/module manifest summary     |
|  - ALL invariants as DECLARATIONS (ID + 1-line + owner)      |
|  - ALL ADR decisions as DECLARATIONS (ID + 1-line + choice)  |
|  - Glossary (terms + 1-line definitions)                     |
|  - Quality gates (summaries only)                            |
|  - Context budget table                                      |
|  SCOPE: System-wide orientation. Knows WHAT exists, not HOW. |
+--------------------------------------------------------------+
| TIER 2: Domain Constitution (200-500 lines, per-domain)      |
|  - Domain formal model (subset of full system model)         |
|  - FULL DEFINITIONS for invariants owned by this domain      |
|  - FULL ANALYSIS for ADRs decided within this domain         |
|  - Cross-domain interface contracts (this domain's surface)  |
|  - Domain-level performance budgets                          |
|  SCOPE: Everything needed to work in this domain.            |
|  NOTE: Content here is NOT duplicated in Tier 3.             |
+--------------------------------------------------------------+
| TIER 3: Cross-Domain Deep Context (0-600 lines, per-module)   |
|  - Full definitions for OTHER-domain invariants this module   |
|    INTERFACES with (not in this module's Tier 2)              |
|  - Full ADR specs from OTHER domains that affect this module  |
|  - Interface contracts with adjacent modules in OTHER domains |
|  - Shared types defined in OTHER domains used by this module  |
|  SCOPE: Cross-domain context ONLY. Zero overlap with Tier 2. |
|  NOTE: If module has no cross-domain interfaces, Tier 3 is    |
|  EMPTY. This is common and correct.                          |
+--------------------------------------------------------------+
| MODULE (800-3,000 lines)                                      |
|  - Module header (ownership, interfaces, negative specs)      |
|  - Full PART II content for this subsystem                   |
|  - All implementation detail for one major subsystem         |
|  SCOPE: What to build for this subsystem.                    |
+--------------------------------------------------------------+

Assembled bundle: Tier 1 + Tier 2 + Tier 3 + Module
Target budget:    1,200 - 4,500 lines per bundle
Hard ceiling:     5,000 lines (must fit in context with reasoning room)
```

// WHY THREE TIERS? Two tiers (root + module) works for systems with < 20 invariants and < 10 ADRs. Beyond that, the root itself exceeds budget. Three tiers add one level of indirection — domain grouping — which is already present in any well-architected system. The domain boundary serves double duty: it was already an isolation mechanism in the architecture, now it is also a context management mechanism. See ADR-006.

### 0.13.4 Invariant Declarations vs. Definitions

The critical mechanism that makes the tiered constitution work. An invariant has two representations:

**Declaration** (Tier 1, always present, ~1 line):
```
APP-INV-017: Event log is append-only -- Owner: EventStore -- Domain: Storage
```

**Definition** (Tier 2, in the owning domain's constitution, ~10-20 lines):
```
**APP-INV-017: Event Log Append-Only**

*Events, once written, are never modified or deleted.*

  ∀ event ∈ EventLog, ∀ t1 < t2:
    event ∈ EventLog(t1) → event ∈ EventLog(t2) ∧ event(t1) = event(t2)

Violation scenario: A compaction routine rewrites old events to save space,
silently changing event payloads. Replay produces different state.

Validation: Write 1000 events, snapshot the log, run any operation, compare
log prefix byte-for-byte.

// WHY THIS MATTERS: Append-only is the foundation of deterministic replay.
// Without it, APP-INV-003 (replay determinism) is impossible.
```

**Inclusion rules — which tier provides which level of detail:**

| Module's relationship to invariant     | Tier 1      | Tier 2 (own domain)              | Tier 3 (cross-domain)  |
|---------------------------------------|-------------|----------------------------------|------------------------|
| Module MAINTAINS this invariant        | Declaration | Full definition (already present) | — (same domain rule)  |
| INTERFACES, invariant in SAME domain  | Declaration | Full definition (already present) | —                     |
| INTERFACES, invariant in OTHER domain | Declaration | —                               | Full definition        |
| No relationship                       | Declaration | —                               | —                     |

Key insight: a module's maintained invariants are ALWAYS in its own domain (enforced by CHECK-4 in §0.13.11). Therefore Tier 2 always covers them. Tier 3 ONLY adds cross-domain content. This eliminates all duplication between tiers.

The same pattern applies to ADRs: declarations in Tier 1 always, full analysis in the domain that decided them, cross-domain inclusion in Tier 3 only when a module in another domain implements or is affected by the ADR.

### 0.13.5 Module Header (Required per Module)

Every module begins with a structured header that makes the module self-describing. The header uses application-level invariant identifiers (APP-INV-NNN), not the DDIS meta-standard's INV-NNN identifiers:

```yaml
# Module Header: [Module Name]
# Domain: [Domain Name]
# Maintains: APP-INV-017, APP-INV-018, APP-INV-019
# Interfaces: APP-INV-003 (via EventStore), APP-INV-032 (via Scheduler)
# Implements: APP-ADR-003, APP-ADR-011
# Adjacent modules: EventStore (read types), Scheduler (publish events)
# Assembly: Tier 1 + Storage domain + cross-domain deep (Coordination interfaces)
#
# NEGATIVE SPECIFICATION (what this module must NOT do):
# - Must NOT directly access TUI rendering state (use event bus)
# - Must NOT bypass the reservation system for file writes
# - Must NOT assume event ordering beyond the guarantees in APP-INV-017
# - Must NOT implement its own serialization (use shared codec from APP-ADR-011)
```

The module header is consumed by:
1. **The assembly script** — to determine what context to include in the bundle
2. **The LLM implementer** — to understand scope boundaries before reading
3. **The RALPH loop** — to determine module dependencies for improvement ordering

### 0.13.6 Cross-Module Reference Rules

**Rule 1: Cross-module references go through the constitution, never direct.** (Enforced by INV-012, locked by ADR-007.)

```
BAD:  "See section 7.3 in the Scheduler chapter for the dispatch algorithm"
GOOD: "This subsystem publishes SchedulerReady events (see APP-INV-032,
       maintained by the Scheduler module)"
```

The invariant lives in the constitution. Both modules can reference it without needing each other's content. The LLM implementing Module A never sees Module B's internals — only the contract (invariant) that Module B must satisfy.

**Rule 2: Shared types are defined in the constitution, not in any module.**

If two modules both use `TaskId` or `EventPayload`, the type definition lives in the domain constitution (Tier 2) or the system constitution (Tier 1), not in either module. Modules reference the type; they don't define it.

**Rule 3: The end-to-end trace is a special module.**

The end-to-end trace (§5.3) is the one element that legitimately crosses all module boundaries. It is stored as its own module file with a special header:

```yaml
# Module Header: End-to-End Trace
# Domain: cross-cutting
# Maintains: (none — this module validates, it doesn't implement)
# Interfaces: ALL application invariants
# Purpose: Integration validation, not implementation
# Assembly: Tier 1 + ALL domain constitutions (no Tier 3 needed)
#
# BUDGET NOTE: With 3 domains at ~400 lines each + ~350 lines Tier 1,
# constitutional overhead is ~1,550 lines. The trace itself must fit in
# ~3,450 lines (5,000 ceiling) or ~2,450 lines (4,000 target).
# Sufficient because the trace has NO implementation detail.
```

### 0.13.7 Modularization Decision Flowchart

```
Is spec > 4,000 lines?
  |-- No  -> Is spec > 2,500 lines AND target context < 8K lines?
  |           |-- No  -> MONOLITH (no modularization needed, stop here)
  |           +-- Yes -> MODULE (recommended)
  +-- Yes -> MODULE (required)
             |
             How many invariants + ADRs total?
             |-- < 20 total AND system constitution fits in <= 400 lines
             |    -> TWO-TIER (see §0.13.7.1)
             +-- >= 20 total OR system constitution > 400 lines
                  -> Does system have natural domain boundaries?
                     |-- Yes -> THREE-TIER (standard protocol)
                     +-- No  -> Refactor architecture to create domain
                                boundaries, then THREE-TIER
```

#### 0.13.7.1 Two-Tier Simplification

For small modular specs, the domain tier can be skipped. In two-tier mode:

- **Tier 1 (System Constitution)**: Contains BOTH declarations AND full definitions for all invariants and ADRs (since there are few enough to fit in ≤ 400 lines).
- **Tier 2 (Domain Constitution)**: SKIPPED. Does not exist in the file layout.
- **Tier 3 (Cross-Domain Deep)**: SKIPPED. Not needed because all full definitions are already in Tier 1.
- **Module**: Unchanged.

Assembly in two-tier mode: `system_constitution + module → bundle`.

The manifest uses `tier_mode: two-tier` to signal this to the assembly script. If the spec grows beyond the two-tier threshold, migrate to three-tier by extracting domain constitutions (see Migration Procedure §0.13.14).

### 0.13.8 File Layout

```
spec-project/
|-- manifest.yaml                     # Single source of truth for assembly
|-- constitution/
|   |-- system.md                     # Tier 1: always included
|   +-- domains/                      # Tier 2: one per domain (absent in two-tier)
|       |-- storage.md
|       |-- coordination.md
|       +-- presentation.md
|-- deep/                             # Tier 3: one per module (only if cross-domain)
|   |-- scheduler.md                  # Cross-domain context for scheduler module
|   +-- integration_tests.md          # Cross-domain context for integration module
|   # NOTE: modules with no cross-domain interfaces have NO file here.
|   # The assembly script treats missing deep/ file as empty Tier 3.
|-- modules/                           # One per subsystem
|   |-- event_store.md
|   |-- snapshot_manager.md
|   |-- scheduler.md
|   |-- reservation_manager.md
|   |-- tui_renderer.md
|   |-- widget_system.md
|   +-- end_to_end_trace.md           # Special cross-cutting module
|-- bundles/                          # Generated by assembly (gitignored)
|   |-- event_store_bundle.md
|   |-- scheduler_bundle.md
|   +-- ...
+-- .beads/                           # Gap/module tracking (if beads enabled)
    +-- beads.db
```

### 0.13.9 Manifest Schema

```yaml
# manifest.yaml — Single source of truth for DDIS module assembly
ddis_version: "1.0"
spec_name: "Example System"
tier_mode: "three-tier"               # "two-tier" or "three-tier"

context_budget:
  target_lines: 4000                  # Preferred max (WARN if exceeded)
  hard_ceiling_lines: 5000            # Absolute max (ERROR if exceeded)
  reasoning_reserve: 0.25             # Fraction reserved for LLM reasoning

constitution:
  system: "constitution/system.md"    # Tier 1: always required
  domains:                            # Tier 2: absent if tier_mode = "two-tier"
    storage:
      file: "constitution/domains/storage.md"
      description: "Event store, snapshots, persistence layer"
    coordination:
      file: "constitution/domains/coordination.md"
      description: "Scheduling, reservations, task DAG"
    presentation:
      file: "constitution/domains/presentation.md"
      description: "TUI rendering, widgets, layout engine"

modules:
  event_store:
    file: "modules/event_store.md"
    domain: storage
    maintains: [APP-INV-003, APP-INV-017, APP-INV-018]
    interfaces: [APP-INV-001, APP-INV-005]
    implements: [APP-ADR-003, APP-ADR-011]
    adjacent: [snapshot_manager, scheduler]
    deep_context: null                # null = no cross-domain context needed
    negative_specs:
      - "Must NOT directly access TUI rendering state"
      - "Must NOT bypass reservation system for writes"

  scheduler:
    file: "modules/scheduler.md"
    domain: coordination
    maintains: [APP-INV-022, APP-INV-023, APP-INV-024]
    interfaces: [APP-INV-003, APP-INV-017]  # In Storage domain = cross-domain!
    implements: [APP-ADR-005, APP-ADR-008]
    adjacent: [reservation_manager, event_store]
    deep_context: "deep/scheduler.md"       # HAS cross-domain context
    negative_specs:
      - "Must NOT hold hard locks (advisory only per APP-ADR-005)"
      - "Must NOT read TUI state directly"

  end_to_end_trace:
    file: "modules/end_to_end_trace.md"
    domain: cross-cutting
    maintains: []
    interfaces: all
    implements: []
    adjacent: all
    deep_context: null                      # Gets ALL Tier 2 instead
    negative_specs: []

invariant_registry:
  APP-INV-001: { owner: system, domain: system, description: "Causal traceability" }
  APP-INV-003: { owner: event_store, domain: storage, description: "Replay determinism" }
  APP-INV-017: { owner: event_store, domain: storage, description: "Append-only log" }
  APP-INV-022: { owner: scheduler, domain: coordination, description: "Fair scheduling" }
  # ... (abbreviated for illustration — real manifests list all invariants)
```

### 0.13.10 Assembly Rules

The assembly script reads the manifest and produces one bundle per module.

**Three-tier assembly (tier_mode: three-tier):**

```
ASSEMBLE(module_name):
  module = manifest.modules[module_name]
  bundle = []

  # Tier 1: Always included
  bundle.append(read(manifest.constitution.system))

  # Tier 2: Domain constitution
  if module.domain == "cross-cutting":
    for domain in manifest.constitution.domains:
      bundle.append(read(domain.file))
  else:
    bundle.append(read(manifest.constitution.domains[module.domain].file))

  # Tier 3: Cross-domain deep context (only if file exists)
  if module.deep_context is not null:
    bundle.append(read(module.deep_context))

  # The module itself
  bundle.append(read(module.file))

  # Budget validation (INV-014)
  total_lines = sum(line_count(section) for section in bundle)
  if total_lines > manifest.context_budget.hard_ceiling_lines:
    ERROR("Bundle {module_name}: {total_lines} lines exceeds ceiling "
          "{hard_ceiling_lines}. INV-014 VIOLATED.")
  elif total_lines > manifest.context_budget.target_lines:
    WARN("Bundle {module_name}: {total_lines} lines exceeds target "
         "{target_lines}.")

  write(bundles/{module_name}_bundle.md, join(bundle))
```

**Two-tier assembly (tier_mode: two-tier):**

```
ASSEMBLE(module_name):
  module = manifest.modules[module_name]
  bundle = []

  # Tier 1 contains FULL definitions in two-tier mode
  bundle.append(read(manifest.constitution.system))
  # No Tier 2, no Tier 3
  bundle.append(read(module.file))

  validate_budget(bundle, module_name)
  write(bundles/{module_name}_bundle.md, join(bundle))
```

### 0.13.11 Consistency Validation

Nine mechanical checks. All implementable by a validation script.

**CHECK-1: Invariant ownership completeness**
```
∀ inv ∈ invariant_registry:
  (inv.owner = "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 0)
  ∨ (inv.owner ≠ "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 1)
```
Remediation: Assign unowned invariant or remove duplicate owner.

**CHECK-2: Interface consistency**
```
∀ s ∈ modules, ∀ inv ∈ s.interfaces (where s.interfaces ≠ "all"):
  (∃ other ∈ modules : inv ∈ other.maintains ∧ other ≠ s)
  ∨ invariant_registry[inv].owner = "system"
```
Remediation: Add invariant to appropriate maintains list or register as system-owned.

**CHECK-3: Adjacency symmetry**
```
∀ a ∈ modules, ∀ b ∈ a.adjacent
  (where a.adjacent ≠ "all" ∧ b.adjacent ≠ "all"):
    a.name ∈ manifest.modules[b].adjacent
```
Remediation: Add missing adjacency entry.

**CHECK-4: Domain membership consistency**
```
∀ s ∈ modules (where s.domain ≠ "cross-cutting"),
  ∀ inv ∈ s.maintains:
    invariant_registry[inv].domain = s.domain
    ∨ invariant_registry[inv].domain = "system"
```
Remediation: Move invariant to module's domain or move module to invariant's domain.

**CHECK-5: Budget compliance**
```
∀ s ∈ modules:
  line_count(ASSEMBLE(s)) ≤ context_budget.hard_ceiling_lines
```
Remediation: Reduce module size, move content to constitution, or split module. (Validates INV-014.)

**CHECK-6: No orphan invariants**
```
∀ inv ∈ invariant_registry:
  ∃ s ∈ modules : inv ∈ s.maintains ∨ inv ∈ s.interfaces
```
Remediation: Add invariant to a module's interfaces or remove from registry.

**CHECK-7: Cross-module reference isolation**
```
∀ module_file ∈ module_files:
  ¬contains(module_file, pattern matching direct module-to-module references)
```
Remediation: Replace direct references with constitutional references. (Validates INV-012.)

**CHECK-8: Deep context correctness (three-tier only)**
```
∀ s ∈ modules (where s.domain ≠ "cross-cutting"):
  let xd = {inv ∈ s.interfaces :
    invariant_registry[inv].domain ≠ s.domain
    ∧ invariant_registry[inv].domain ≠ "system"}
  (count(xd) > 0 ⟹ s.deep_context ≠ null)
  ∧ (count(xd) = 0 ⟹ s.deep_context = null)
```
Remediation: Create missing deep context file or remove unnecessary one.

**CHECK-9: File existence**
```
∀ path ∈ manifest.all_referenced_paths: file_exists(path)
∀ module_file ∈ filesystem("modules/"): module_file ∈ manifest.modules.*.file
```
Remediation: Create missing file or correct manifest path. Second clause catches module files that exist on disk but are missing from the manifest. (Validates INV-016.)

### 0.13.12 Cascade Protocol

When constitutional content changes, affected modules must be re-validated.

**Blast radius by change type:**

| Change                          | Blast Radius                     |
|---------------------------------|----------------------------------|
| Invariant wording changed       | Modules maintaining or interfacing |
| ADR superseded                  | Modules implementing that ADR     |
| New invariant added             | Module assigned as owner          |
| Shared type changed             | Same-domain + cross-domain users |
| Non-negotiable changed          | ALL modules                       |
| Glossary term redefined         | All modules using that term       |

**Cascade workflow (with beads):**

```
1. Author changes APP-INV-017 in constitution/domains/storage.md
2. Run: ddis_validate.sh --check-cascade APP-INV-017
3. Script queries manifest for affected modules:
   - event_store (maintains APP-INV-017) → MUST re-validate
   - snapshot_manager (interfaces APP-INV-017) → SHOULD re-validate
   - scheduler (interfaces APP-INV-017 via deep) → SHOULD re-validate
4. Script creates/reopens br issues for affected modules
   Label: cascade:APP-INV-017, priority by blast radius
5. bv --robot-plan shows improvement order
6. Re-run assembly, re-validate affected modules
```

**Cascade workflow (without beads — manifest-only fallback):**

```
1-3. Same as above.
4. Script prints affected modules to stdout:
   MUST:   event_store
   SHOULD: snapshot_manager, scheduler
5. Re-run assembly, manually re-validate affected modules
```

Both paths use the same manifest query. Beads adds persistence and ordering; the manifest provides the data either way.

### 0.13.13 Quality Gate Extensions

Modular specs must pass the additional quality gates defined in §0.7 (Gates M-1 through M-5) in addition to the base DDIS gates (Gates 1 through 6).

### 0.13.14 Monolith-to-Module Migration Procedure

**Step 1: Identify domains.**
Group PART II chapters into 2–5 domains based on architectural boundaries.

**Step 2: Extract system constitution.**
From monolith to `constitution/system.md`: preamble, PART 0 sections, all invariant DECLARATIONS, all ADR DECLARATIONS, glossary (1-line definitions), quality gates, non-negotiables, non-goals.

**Step 3: Extract domain constitutions.**
For each domain to `constitution/domains/{domain}.md`: domain formal model, full invariant definitions owned by domain, full ADR analysis decided in domain, cross-domain interface contracts, domain performance budgets.

**Step 4: Extract modules.**
For each PART II chapter to `modules/{subsystem}.md`: add module header (§0.13.5), include implementation content, convert cross-module direct references to constitutional references (hardest step — see INV-012).

**Step 5: Create cross-domain deep context files.**
For each module interfacing with other-domain invariants: create `deep/{module}.md` with full definitions for cross-domain invariants, interface contracts, shared types.

**Step 6: Build manifest.**
Create `manifest.yaml` with all module entries, invariant registry, context budget.

**Step 7: Validate.**
Run `ddis_validate.sh` — all nine checks must pass.

**Step 8: Extract end-to-end trace.**
Create `modules/end_to_end_trace.md` as cross-cutting module. Verify bundle fits within budget.

**Step 9: LLM validation.**
Give 2+ bundles to an LLM. Zero questions requiring other module's implementation.

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
  Validated   — External implementer confirms readiness (Gate 6)
  Living      — In use, being updated as implementation reveals gaps

Transitions:
  Skeleton  →[author fills sections]→     Drafted
  Drafted   →[author adds cross-refs]→    Threaded
  Threaded  →[gates 1-5 pass]→            Gated
  Gated     →[gate 6 passes]→             Validated
  Validated →[implementation begins]→     Living
  Living    →[gap discovered]→            Drafted (partial regression)

Invalid transitions:
  Skeleton → Gated          (cannot pass gates with empty sections)
  Drafted → Validated       (cannot validate without cross-references)
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
  ◇(spec.answers(Q))  // "eventually" means by the time the spec is Validated
```

### 1.3 Complexity of Specification Elements

| Element | Authoring Complexity | Reading Complexity | Verification Complexity |
|---|---|---|---|
| Invariant | O(domain_understanding) | O(1) per invariant | O(1) per invariant (construct counterexample) |
| ADR | O(alternatives × analysis_depth) | O(alternatives) per ADR | O(1) per ADR (check that alternatives are genuine) |
| Algorithm | O(algorithm_complexity × edge_cases) | O(pseudocode_length) | O(worked_examples) |
| Cross-reference | O(1) per reference | O(1) per reference | O(sections²) for full graph analysis |
| End-to-end trace | O(subsystems × interactions) | O(subsystems) | O(1) (follow the trace) |

The quadratic cost of cross-reference verification is why automated tooling (Open Question §0.10.1) would be valuable.

---

# PART II: CORE STANDARD — ELEMENT SPECIFICATIONS

This is the heart of DDIS. Each section specifies one structural element: what it must contain, what quality criteria it must meet, how it relates to other elements, and what it looks like when done well versus done badly.

## Chapter 2: Preamble Elements

### 2.1 Design Goal

**What it is**: A single sentence (≤ 30 words) that states the system's reason for existing.

**Required properties**:
- States the core value proposition, not the implementation
- Uses bold for emphasis on the 3–5 key properties
- Readable by a non-technical stakeholder

**Quality criteria**: A reader who sees only the design goal should be able to decide whether this system is relevant to them.

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

**Anti-pattern**: "The system provides robust, scalable, enterprise-grade coordination." ← Meaningless buzzwords.

**Good example** (FrankenTUI): "ftui is designed so you can build a Claude Code / Codex-class agent harness UI without flicker, without cursor corruption, and without sacrificing native scrollback."

---

### 2.3 Document Note

**What it is**: A short disclaimer (2–4 sentences) about the nature of code blocks and where the correctness contract lives.

**Why it exists**: Without this note, implementers treat code blocks as copy-paste targets. When the pseudocode has a typo or uses a slightly wrong API, they copy the bug. The document note redirects trust from code to invariants and tests.

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

**Quality criteria**: A new team member reading only this section knows exactly how to engage with the document.

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
- An implementer could imagine a situation where violating it would be tempting (e.g., "just skip replay validation in dev mode — it's slow")
- The non-negotiable clearly says: no, even then
- It is not a restatement of a technical invariant; it is a commitment

**Relationship to invariants**: Non-negotiables are the "why" that justifies groups of invariants. "Deterministic replay is real" (non-negotiable) justifies INV-003: "Same event log → identical state" (invariant). The non-negotiable is the commitment; the invariant is the testable manifestation.

---

### 3.2 Non-Goals

**What it is**: A list of 5–10 things the system explicitly does NOT attempt.

**Why it exists**: Scope creep is the most common spec failure. Non-goals are the immune system. They give implementers permission to say "that's out of scope" when stakeholders request features that violate the system's boundaries.

**Quality criteria for each non-goal**:
- Someone has actually asked for this (or will), making the exclusion non-obvious
- The non-goal explains briefly why it's excluded (not just "not in scope" but why not)

**Anti-pattern**: "Non-goal: Building a quantum computer." ← Nobody asked for this. Non-goals should exclude things that are tempting, not absurd.

---

### 3.3 First-Principles Derivation

**What it is**: The formal model from which the entire architecture derives. This is the section that makes every other section feel *inevitable* rather than *asserted*.

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
```

**Quality criteria for each ADR**:
- **Genuine alternatives**: Each option must have a real advocate. If Option B is a strawman nobody would choose, it is not a genuine alternative. The test: would a competent engineer in a different context reasonably choose Option B?
- **Concrete tradeoffs**: Pros and cons cite specific, measurable properties — not vague qualities like "simpler" or "more robust."
- **Consequential decision**: The choice materially affects the system. If swapping Option A for Option B would require < 1 day of refactoring, it's not an ADR — it's a local implementation choice.

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

**Churn-magnets**: After all ADRs are written, add a brief section identifying which decisions cause the most downstream rework if changed. These are the decisions to lock first and spike earliest (see §4.2, Phase -1).

---

### 3.6 Quality Gates

**What it is**: 4–8 stop-ship criteria, ordered by priority.

**Required properties per gate**:
- A gate is a **predicate**, not a task. It is either passing or failing at any point in time.
- Each gate references specific invariants or test suites.
- Gates are ordered such that a failing Gate N makes Gate N+1 irrelevant.

**Quality criteria**: A project manager should be able to assess gate status in < 30 minutes using the referenced tests.

---

### 3.7 Performance Budgets and Design Point

**What it is**: A table of performance targets anchored to a specific design point (hardware, workload, scale).

**Required components**:

1. **Design point**: The specific scenario these budgets apply to. E.g., "M1 Max, 300 concurrent agents, 10K tasks, 60Hz TUI refresh."

2. **Budget table**: Operation → target → measurement method.

3. **Measurement harness description**: How to run the benchmarks (at minimum, benchmark names and what they simulate).

4. **Adjustment guidance**: "These are validated against the design point. If your design point differs, adjust with reasoning — but document the new targets and re-validate."

**Quality criteria**: An implementer can run the benchmarks and get a pass/fail signal without asking anyone.

**Anti-pattern**: "The system should be fast enough for real-time use." ← No number, no design point, no measurement method.

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
- State × Event table (what happens for every combination)
- Guard conditions on transitions
- Invalid transition policy (ignore? error? log?)
- Entry/exit actions

**Quality criteria**: The state × event table has no empty cells. Every cell either names a transition or explicitly says "no transition" or "error."

### 4.3 Complexity Analysis

**What it is**: Complexity bounds for every fundamental operation defined in the first-principles model.

**Required**: Big-O bounds with constants where they matter for the design point. "O(n) where n = active_agents, expected ≤ 300" is more useful than "O(n)."

---

## Chapter 5: PART II Elements

### 5.1 Implementation Chapters

**What it is**: One chapter per major subsystem. This is where the spec earns its value.

**Required components per chapter**:

1. **Purpose statement** (2–3 sentences): What this subsystem does and why it exists. References the formal model.

2. **Formal types**: Data structures with memory layout analysis where relevant. Include `// WHY NOT` annotations on non-obvious choices (see §5.5).

3. **Algorithm pseudocode**: Every non-trivial algorithm, in pseudocode or "close to [language]" sketches. Include complexity analysis inline.

4. **State machine** (if stateful): Full state machine per §4.2.

5. **Invariants preserved**: Which INV-NNN this subsystem is responsible for maintaining.

6. **Worked example(s)**: At least one concrete scenario showing the subsystem in action with specific values, not variables.

7. **Edge cases and error handling**: What happens when inputs are malformed, resources are exhausted, or invariants are threatened.

8. **Test strategy**: What kinds of tests (unit, property, integration, replay, stress) cover this subsystem.

9. **Performance budget**: The subsystem's share of the overall performance budget.

10. **Cross-references**: To ADRs, invariants, other subsystems, the formal model.

**Quality criteria**: An implementer could build this subsystem from this chapter alone, without reading any other chapter. (They would need to read other chapters to understand how subsystems compose, but each chapter is self-contained for its subsystem.)

---

### 5.2 Worked Examples

**What it is**: A concrete scenario with specific values (not variables) showing the subsystem processing a realistic input.

**Required properties**:
- Uses concrete values: `task_id = T-042`, not "some task"
- Shows state before, the operation, and state after
- Includes at least one non-trivial aspect (an edge case, a conflict, a boundary condition)

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

**Why it exists**: Individual subsystem examples prove each piece works. The end-to-end trace proves the pieces fit together. Many bugs live at subsystem boundaries. (Validated by INV-001.)

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

---

## Chapter 6: PART IV Elements

### 6.1 Operational Playbook

**What it is**: A chapter that prevents the most common failure mode of detailed specs: infinite refinement without shipping.

**Required sections**:

#### 6.1.1 Phase -1: Decision Spikes

Before building anything, run tiny experiments that de-risk the hardest unknowns. Each spike produces an ADR.

**Required per spike**:
- What question it answers
- Maximum time budget (typically 1–3 days)
- Exit criterion: one ADR capturing decision + rationale + consequences

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

The order in which subsystems should be built, chosen to maximize the "working subset" at each stage. The first deliverable should be a minimal system that exercises the core loop, not a complete system missing its core.

#### 6.1.5 Immediate Next Steps (First PRs)

The literal first 5–6 things to implement, in dependency order. Not strategic. Tactical. This converts the spec from "a plan to study" into "a plan to execute starting now."

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

---

### 6.3 Error Taxonomy

**What it is**: A classification of errors the system can encounter, with handling strategy per class.

**Required properties**:
- Each error class has a severity (fatal, degraded, recoverable, ignorable)
- Each error class has a handling strategy (crash, retry, degrade, log-and-continue)
- Cross-references to invariants: which invariants might be threatened by each error class

---

## Chapter 7: Appendix Elements

### 7.1 Glossary

**What it is**: Every domain-specific term, defined in 1–3 sentences with a cross-reference to where it's formally specified.

**Required properties**:
- Alphabetized
- Each entry includes (see §X.Y) pointing to the formal definition
- Terms that have both a common meaning and a domain-specific meaning clearly distinguish the two

**Anti-pattern**: Defining "task" as "a unit of work." Define it as "a node in the task DAG representing a discrete, assignable unit of implementation work with explicit dependencies, acceptance criteria, and at most one assigned agent at any time (see §7.2, INV-012)."

---

### 7.2 Risk Register

**What it is**: Top 5–10 risks to the project, each with a concrete mitigation.

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
- Uses humor sparingly and only when it clarifies ("this is where most TUIs become flaky")
- Never uses marketing language ("enterprise-grade", "cutting-edge", "revolutionary")
- Never uses bureaucratic language ("it is recommended that", "the system shall")

**Calibration examples**:

```
✅ GOOD: "The kernel loop is single-threaded by design — not because concurrency is
hard, but because serialization through the event log is the mechanism that gives
us deterministic replay for free."

❌ BAD (academic): "The kernel loop utilizes a single-threaded architecture paradigm
to facilitate deterministic replay capabilities within the event-sourced persistence
layer."

❌ BAD (casual): "We made the kernel single-threaded and it's awesome!"

❌ BAD (bureaucratic): "It is recommended that the kernel loop shall be implemented
in a single-threaded manner to support the deterministic replay requirement as
specified in section 4.3.2.1."
```

### 8.2 Formatting Conventions

- **Bold** for terms being defined, non-negotiable properties, and emphasis on critical warnings
- `Code` for types, function names, file names, and anything that would appear in source code
- `// Comments` for inline justifications and WHY NOT annotations
- Tables for structured data (operations, budgets, comparisons)
- Blockquotes for the preamble elements only
- ASCII diagrams preferred over external image references (the spec should be readable in any text editor)

### 8.3 Anti-Pattern Catalog

Every DDIS element has bad and good examples defined in its specification (PART II). This section collects cross-cutting anti-patterns that affect multiple elements:

**Anti-pattern: The Hedge Cascade**
```
❌ "It might be worth considering the possibility of potentially using a
single-threaded loop, which could arguably provide some benefits in terms
of determinism, although this would need to be validated."
✅ "The kernel loop is single-threaded. This gives us deterministic replay.
See ADR-003 for the throughput analysis that confirms this is sufficient."
```

**Anti-pattern: The Orphan Section**
A section that references nothing and is referenced by nothing. It may contain good content, but if it's disconnected from the web, it's carrying dead weight. Either connect it or remove it.

**Anti-pattern: The Trivial Invariant**
"INV-042: The system uses UTF-8 encoding." This is either enforced by the language/platform (not worth an invariant) or so fundamental it belongs in Non-Negotiables, not the invariant list.

**Anti-pattern: The Strawman ADR**
```
❌ Options:
  A) Our chosen approach (clearly the best)
  B) A terrible approach nobody would choose
  Decision: A, obviously.
```
Every option in an ADR must have a genuine advocate — a competent engineer who, in a different context, would choose it.

**Anti-pattern: The Percentage-Free Performance Budget**
"The system should respond quickly." Without a number, a design point, and a measurement method, this is a wish, not a budget.

**Anti-pattern: The Spec That Requires Oral Tradition**
If an implementer must ask the architect a question that the spec should have answered, the spec has a gap. Track these questions during implementation and patch them back into the spec (see Living state, §1.1).

---

## Chapter 9: Proportional Weight Deep Dive

### 9.1 Identifying the Heart

Every system has a "heart" — the 2–3 subsystems where most complexity and most bugs live. In the proportional weight guide, these subsystems should receive 40–50% of the PART II line budget.

**How to identify the heart**:
- Which subsystems have the most invariants?
- Which subsystems have the most ADRs?
- Which subsystems appear in the most cross-references?
- If you had to cut the spec in half, which subsystems would you keep?

### 9.2 Signals of Imbalanced Weight

- A subsystem with 5 invariants and 50 lines of spec is **starved**
- A subsystem with 1 invariant and 500 lines of spec is **bloated**
- PART 0 longer than PART II means the spec is top-heavy (more framing than substance)
- Appendices longer than PART II means reference material is displacing implementation spec

---

## Chapter 10: Cross-Reference Patterns

### 10.1 Reference Syntax

DDIS does not mandate a specific syntax, but recommends consistent conventions. Common patterns:

```
(see §3.2)                    — section reference
(validated by INV-004)        — invariant reference
(locked by ADR-003)           — decision reference
(measured by Benchmark B-001) — performance reference
(defined in Glossary: "task") — glossary reference
```

### 10.2 Reference Density Targets

| Section Type | Minimum Outbound References |
|---|---|
| Implementation chapter | 3 (at least: one ADR, one invariant, one other chapter) |
| ADR | 2 (at least: one invariant, one implementation chapter) |
| Invariant | 1 (at least: one test or validation method) |
| Performance budget | 2 (at least: one benchmark, one design point) |
| Test strategy | 2 (at least: one invariant, one implementation chapter) |

---

# PART IV: OPERATIONS

## Chapter 11: Applying DDIS to a New Project

### 11.1 The Authoring Sequence

Write sections in this order (not document order) to minimize rework:

1. **Design goal + Core promise** (forces you to articulate the value)
2. **First-principles formal model** (forces you to understand the domain)
3. **Non-negotiables** (forces you to commit to what matters)
4. **Invariants** (forces you to formalize the commitments)
5. **ADRs** (forces you to lock the controversial decisions)
6. **Implementation chapters** — heaviest subsystems first (the "heart")
7. **End-to-end trace** (reveals gaps in subsystem interfaces)
8. **Performance budgets** (anchors the implementation to measurable targets)
9. **Test strategies** (turns invariants into executable verification)
10. **Cross-references** (weaves the web)
11. **Glossary** (extract terms from the complete spec)
12. **Master TODO** (convert the spec into an execution plan)
13. **Operational playbook** (how to start building)

### 11.2 Common Mistakes in First DDIS Specs

1. **Writing implementation chapters before ADRs.** You'll rewrite them when you discover the ADRs imply different choices.

2. **Writing the glossary first.** You don't know your terminology until you've written the spec. Write it last.

3. **Treating the end-to-end trace as optional.** It's the single most effective quality check. Write it.

4. **Under-investing in WHY NOT annotations.** Every non-obvious choice needs one. The first maintainer will thank you.

5. **Skipping the anti-patterns.** Show what bad output looks like. LLMs especially benefit from negative examples.

---

## Chapter 12: Validating a DDIS Specification

### 12.1 Self-Validation Checklist

Before declaring a spec complete, the author should:

1. Pick 5 random implementation sections. Trace each backward to the formal model. Did any chain break?
2. Read each ADR's "alternatives" section. Would a competent engineer genuinely choose any rejected option? If not, the ADR is a strawman.
3. For each invariant, spend 60 seconds trying to construct a violation scenario. If you can't, the invariant is either trivially true or too vague.
4. Build the cross-reference graph (mentally or on paper). Are there orphan sections?
5. Read the spec as if you were an implementer seeing it for the first time. Where did you have to guess?

### 12.2 External Validation

The strongest validation is giving the spec to an implementer (or LLM) and tracking:
- Questions they ask that the spec should have answered (→ gaps)
- Incorrect implementations that the spec didn't prevent (→ ambiguities)
- Sections they skipped because they couldn't understand them (→ voice/clarity issues)

---

## Chapter 13: Evolving a DDIS Specification

### 13.1 The Living Spec

Once implementation begins, the spec enters the Living state (§1.1). In this state:

- **Gaps discovered during implementation** are patched back into the spec, not into oral tradition or issue trackers. The spec remains the single source of architectural truth.
- **ADRs may be superseded.** When an ADR is reversed, mark the old ADR as "Superseded by ADR-NNN" and update all cross-references. Do not delete the old ADR — its reasoning is historical record.
- **New invariants may be added.** Implementation often reveals properties that weren't obvious during design. Add them with full INV-NNN format.
- **Performance budgets may be revised.** If a budget is consistently unachievable, either the budget or the design must change. Document which, and why.

### 13.2 Spec Versioning

DDIS recommends a simple versioning scheme: `Major.Minor` where:
- **Major** increments when the formal model or a non-negotiable changes
- **Minor** increments when ADRs, invariants, or implementation chapters are added or revised

---

# APPENDICES

## Appendix A: Glossary

| Term | Definition |
|---|---|
| **ADR** | Architecture Decision Record. A structured record of a design choice, including alternatives considered and rationale. (See §3.5) |
| **Bundle** | The assembled document sent to an LLM for implementation of a single module. Contains: Tier 1 + Tier 2 + Tier 3 + Module. The unit of LLM consumption in modular specs. (See §0.13.2, §0.13.10) |
| **Cascade protocol** | The procedure for identifying and re-validating modules affected by a change to constitutional content. (See §0.13.12) |
| **Causal chain** | The traceable path from a first principle through an invariant and/or ADR to an implementation detail. (See §0.2.2, INV-001) |
| **Churn-magnet** | A decision that, if left open, causes the most downstream rework. ADRs should prioritize locking churn-magnets. (See §3.5) |
| **Comparison block** | A side-by-side ❌/✅ comparison of a rejected and chosen approach with quantified reasoning. (See §5.5) |
| **Constitution** | The cross-cutting material that constrains all modules in a modular spec. Organized in tiers: system (Tier 1), domain (Tier 2), cross-domain deep (Tier 3). (See §0.13.3) |
| **Cross-reference** | An explicit link between two sections of the spec, forming part of the reference web. (See Chapter 10, INV-006) |
| **DDIS** | Decision-Driven Implementation Specification. This standard. |
| **Decision spike** | A time-boxed experiment that de-risks an unknown and produces an ADR. (See §6.1.1) |
| **Declaration** | A compact (1-line) summary of an invariant or ADR in the system constitution (Tier 1). Contrasts with the full definition in the domain constitution (Tier 2). (See §0.13.4) |
| **Deep context** | Tier 3 of the constitution: cross-domain invariant definitions, ADR specs, and interface contracts needed by a specific module. Zero overlap with Tier 2. (See §0.13.3) |
| **Definition** | The full specification of an invariant or ADR in the domain constitution (Tier 2), including formal expression, violation scenario, and validation method. (See §0.13.4) |
| **Design point** | The specific hardware, workload, and scale scenario against which performance budgets are validated. (See §3.7) |
| **Domain** | An architectural grouping of related modules sharing tighter coupling with each other than with modules in other domains. Corresponds to rings, layers, or crate groups. (See §0.13.2) |
| **Domain constitution** | Tier 2 of the constitution: full invariant definitions and ADR analysis for one architectural domain. (See §0.13.3) |
| **End-to-end trace** | A worked scenario that traverses all major subsystems, showing data at each boundary. In modular specs, stored as a special cross-cutting module. (See §5.3, §0.13.6) |
| **Exit criterion** | A specific, testable condition that must hold for a phase to be considered complete. (See §6.1.2) |
| **Falsifiable** | A property of an invariant: it can be violated by a concrete scenario and detected by a concrete test. (See INV-003, ADR-002) |
| **First principles** | The formal model of the problem domain from which the architecture derives. (See §3.3) |
| **Formal model** | A mathematical or pseudo-mathematical definition of the system as a state machine or function. (See §0.2.1) |
| **Gate** | A quality gate: a stop-ship predicate that must be true before the project can proceed. (See §3.6) |
| **Invariant** | A numbered, falsifiable property that must hold at all times during system operation. (See §3.4) |
| **Living spec** | A specification in active use, being updated as implementation reveals gaps. (See §13.1) |
| **Manifest** | A machine-readable YAML file declaring all modules, their domain membership, invariant ownership, cross-module interfaces, and assembly rules. The single source of truth for module assembly. (See §0.13.9) |
| **Master TODO** | A checkboxable task inventory cross-referenced to subsystems, phases, and ADRs. (See §7.3) |
| **Monolith** | A DDIS spec that exists as a single document, as opposed to a modular spec. All specs start as monoliths. (See §0.13.2) |
| **Non-goal** | Something the system explicitly does not attempt. (See §3.2) |
| **Non-negotiable** | A philosophical commitment stronger than an invariant — defines what the system IS. (See §3.1) |
| **Operational playbook** | A chapter covering how the spec gets converted into shipped software. (See §6.1) |
| **Proportional weight** | Line budget guidance preventing bloat in some sections and starvation in others. (See §0.8.2) |
| **Self-bootstrapping** | A property of this standard: it is written in the format it defines. (See ADR-004) |
| **Module** | A self-contained unit of a modular spec covering one major subsystem. Corresponds to one PART II chapter. Always assembled into a bundle with constitutional context. (See §0.13.2, §0.13.5) |
| **Module header** | A structured YAML-format block at the start of each module declaring its domain, maintained invariants, interfaces, adjacent modules, and negative specifications. (See §0.13.5) |
| **System constitution** | Tier 1 of the constitution: compact declarations of all invariants and ADRs, plus system-wide orientation (design goal, non-negotiables, glossary summaries). Always included in every bundle. (See §0.13.3) |
| **Three-tier mode** | The standard modularization configuration: system constitution (Tier 1) + domain constitution (Tier 2) + cross-domain deep context (Tier 3) + module. (See §0.13.7, ADR-006) |
| **Two-tier mode** | A simplified modularization configuration for small specs (< 20 invariants): system constitution (full definitions) + module. No domain or deep context tiers. (See §0.13.7.1) |
| **Voice** | The writing style prescribed by DDIS: technically precise but human. (See §8.1) |
| **WHY NOT annotation** | An inline comment explaining why a non-obvious alternative was rejected. (See §5.4) |
| **Worked example** | A concrete scenario with specific values showing a subsystem in action. (See §5.2) |

---

## Appendix B: Risk Register

| # | Risk | Impact | Mitigation | Detection |
|---|---|---|---|---|
| 1 | Standard is too prescriptive, authors feel constrained | Low adoption | Non-goals clearly state what DDIS doesn't attempt; [Optional] elements provide flexibility | Author feedback; compare time-to-first-spec across teams |
| 2 | Standard is too verbose, specs become shelfware | Implementers don't read the spec | Proportional weight guide limits bloat; voice guide keeps prose readable | Track "questions that the spec should have answered" during implementation |
| 3 | Cross-reference requirement is burdensome | Authors skip references, violating INV-006 | Authoring sequence (§11.1) defers cross-references to step 10 so they're added systematically, not incrementally | Reference graph analysis during validation |
| 4 | Self-bootstrapping creates circular confusion | Readers can't distinguish meta-level from object-level | Document note and consistent use of "this standard" vs "a conforming specification" | Reader feedback on first encounter |
| 5 | No automated tooling exists for validation | Quality gates require manual effort | Completeness checklist (Part X) makes manual checks systematic | Track time-to-validate; prioritize tooling if > 2 hours |

---

## Appendix C: Quick-Reference Card

For experienced DDIS authors who need a reminder, not the full standard:

```
PREAMBLE: Design goal → Core promise → Document note → How to use
PART 0:   Summary → First principles → Architecture → Layout →
          Invariants → ADRs → Gates → Budgets → API → Non-negotiables → Non-goals
PART I:   Formal model → State machines → Complexity
PART II:  [Per subsystem: types → algorithm → state machine → invariants →
          example → WHY NOT → tests → budget → cross-refs]
          End-to-end trace (crosses all subsystems)
PART III: Protocol schemas → Adapters → UI contracts
PART IV:  Test taxonomy → Error taxonomy → Operational playbook
          (spikes → exit criteria → merge discipline → deliverable order → first PRs)
APPENDICES: Glossary → Risks → Formats → Benchmarks
PART X:   Master TODO (checkboxable, by subsystem)

Every invariant: ID + statement + formal + violation + test + why
Every ADR: problem + options (genuine) + decision + WHY NOT + consequences + tests
Every algorithm: pseudocode + complexity + example + edge cases
Cross-refs: web, not list. No orphan sections.
Voice: senior engineer to respected peer. No hedging. No marketing. No bureaucracy.
```

---

# PART X: MASTER TODO INVENTORY

## A) Meta-Standard Validation
- [x] Self-bootstrapping: this document uses the format it defines
- [x] Preamble elements: design goal, core promise, document note, how to use
- [x] Non-negotiables defined (§0.1.2)
- [x] Non-goals defined (§0.1.3)
- [x] First-principles derivation (§0.2)
- [x] Document structure prescribed (§0.3)
- [x] Invariants numbered and falsifiable (§0.5, INV-001 through INV-010, plus INV-011 through INV-016 for modularization)
- [x] ADRs with genuine alternatives (§0.6, ADR-001 through ADR-005, plus ADR-006 and ADR-007 for modularization)
- [x] Quality gates defined (§0.7)
- [x] Performance budgets (§0.8 — for spec authoring, not software)
- [x] Proportional weight guide (§0.8.2)

## B) Element Specifications
- [x] Preamble elements specified (Chapter 2)
- [x] PART 0 elements specified (Chapter 3)
- [x] PART I elements specified (Chapter 4)
- [x] PART II elements specified (Chapter 5)
- [x] PART IV elements specified (Chapter 6)
- [x] Appendix elements specified (Chapter 7)
- [x] Anti-pattern catalog (§8.3)
- [x] Cross-reference patterns (Chapter 10)

## C) Guidance
- [x] Voice and style guide (Chapter 8)
- [x] Proportional weight deep dive (Chapter 9)
- [x] Authoring sequence (§11.1)
- [x] Common mistakes (§11.2)
- [x] Validation procedure (Chapter 12)
- [x] Evolution guidance (Chapter 13)

## D) Reference Material
- [x] Glossary (Appendix A)
- [x] Risk register (Appendix B)
- [x] Quick-reference card (Appendix C)

## E) Validation
- [x] INV-001 (Causal Traceability): Every element specification traces to the formal model via the failure mode table (§0.2.2)
- [x] INV-003 (Falsifiability): Each invariant has a violation scenario and validation method
- [x] INV-006 (Cross-Reference Density): Sections reference each other throughout
- [x] INV-007 (Signal-to-Noise): Each section serves a named purpose in the failure mode table
- [ ] INV-008 (Self-Containment): Requires external validation — give this standard to a first-time author and track their questions
- [ ] Gate 6 (Implementation Readiness): Requires a non-trivial spec to be written conforming to DDIS

## F) Modularization Protocol
- [x] Modularization protocol integrated (§0.13) with 14 subsections
- [x] INV-011 through INV-016 present with violation scenarios and validation methods
- [x] ADR-006 (Tiered Constitution) and ADR-007 (Cross-Module References) with genuine alternatives
- [x] Quality gates M-1 through M-5 defined (§0.7)
- [x] Tiered constitution model specified: Tier 1 (declarations), Tier 2 (domain definitions), Tier 3 (cross-domain deep)
- [x] Manifest schema documented with full YAML example (§0.13.9)
- [x] Assembly rules specified for both two-tier and three-tier modes (§0.13.10)
- [x] All 9 consistency checks defined with formal expressions (§0.13.11, CHECK-1 through CHECK-9)
- [x] Cascade protocol documented with and without beads fallback (§0.13.12)
- [x] Migration procedure: monolith to modular, 9 steps (§0.13.14)
- [x] Module header format specified with namespace distinction (§0.13.5)
- [x] Cross-module reference rules formalized (§0.13.6)
- [x] Modularization decision flowchart with two-tier simplification (§0.13.7)
- [ ] Gate M-3 (LLM Bundle Sufficiency): Requires external validation — give 2+ bundles to an LLM
- [ ] Tooling: ddis_assemble.sh implementing §0.13.10
- [ ] Tooling: ddis_validate.sh implementing §0.13.11

---

## Conclusion

DDIS synthesizes techniques from several well-established traditions:

1. **From Architecture Decision Records** (Nygard): The Problem → Options → Decision → Consequences structure that makes design choices explicit and reviewable.

2. **From Design by Contract** (Meyer): The invariant-first approach where system properties are stated formally before implementation details.

3. **From Formal Specification** (Lamport): The use of state machines, temporal properties, and formal models as the foundation from which architecture derives.

4. **From Game Engine Development**: Performance budgets tied to specific design points with concrete measurement methodologies.

5. **From Test-Driven Development**: The requirement that every property be testable and every algorithm include worked examples and edge cases.

6. **From the FrankenTUI specification** (which prompted this standard): The causal chain structure, the cross-reference web, the WHY NOT annotations, the comparison blocks, the voice guidance, the operational playbook, and the self-imposed discipline of a document where every section earns its place.

The result is a specification standard that is:

- **Decision-driven**: Architecture emerges from locked decisions, not assertions
- **Invariant-anchored**: Correctness is defined before implementation
- **Falsifiable throughout**: Every claim can be tested
- **Self-validating**: Quality gates and the completeness checklist provide mechanical conformance checking
- **Self-bootstrapping**: This document is both the standard and its first conforming instance

*DDIS: Where rigor meets readability — and specifications become implementations.*

