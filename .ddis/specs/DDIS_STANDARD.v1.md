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
| **Causal chain** | The traceable path from a first principle through an invariant and/or ADR to an implementation detail. (See §0.2.2, INV-001) |
| **Churn-magnet** | A decision that, if left open, causes the most downstream rework. ADRs should prioritize locking churn-magnets. (See §3.5) |
| **Comparison block** | A side-by-side ❌/✅ comparison of a rejected and chosen approach with quantified reasoning. (See §5.5) |
| **Cross-reference** | An explicit link between two sections of the spec, forming part of the reference web. (See Chapter 10, INV-006) |
| **DDIS** | Decision-Driven Implementation Specification. This standard. |
| **Decision spike** | A time-boxed experiment that de-risks an unknown and produces an ADR. (See §6.1.1) |
| **Design point** | The specific hardware, workload, and scale scenario against which performance budgets are validated. (See §3.7) |
| **End-to-end trace** | A worked scenario that traverses all major subsystems, showing data at each boundary. (See §5.3) |
| **Exit criterion** | A specific, testable condition that must hold for a phase to be considered complete. (See §6.1.2) |
| **Falsifiable** | A property of an invariant: it can be violated by a concrete scenario and detected by a concrete test. (See INV-003, ADR-002) |
| **First principles** | The formal model of the problem domain from which the architecture derives. (See §3.3) |
| **Formal model** | A mathematical or pseudo-mathematical definition of the system as a state machine or function. (See §0.2.1) |
| **Gate** | A quality gate: a stop-ship predicate that must be true before the project can proceed. (See §3.6) |
| **Invariant** | A numbered, falsifiable property that must hold at all times during system operation. (See §3.4) |
| **Living spec** | A specification in active use, being updated as implementation reveals gaps. (See §13.1) |
| **Master TODO** | A checkboxable task inventory cross-referenced to subsystems, phases, and ADRs. (See §7.3) |
| **Non-goal** | Something the system explicitly does not attempt. (See §3.2) |
| **Non-negotiable** | A philosophical commitment stronger than an invariant — defines what the system IS. (See §3.1) |
| **Operational playbook** | A chapter covering how the spec gets converted into shipped software. (See §6.1) |
| **Proportional weight** | Line budget guidance preventing bloat in some sections and starvation in others. (See §0.8.2) |
| **Self-bootstrapping** | A property of this standard: it is written in the format it defines. (See ADR-004) |
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
- [x] Invariants numbered and falsifiable (§0.5, INV-001 through INV-010)
- [x] ADRs with genuine alternatives (§0.6, ADR-001 through ADR-005)
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

