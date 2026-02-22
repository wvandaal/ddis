# DDIS: Decision-Driven Implementation Specification Standard

## Version 3.0 — A Self-Bootstrapping Meta-Specification

> Design goal: **A formal standard for writing implementation specifications that are precise enough for an LLM or junior engineer to implement correctly without guessing, while remaining readable enough that a senior engineer would choose to read them voluntarily.**

> Core promise: A specification conforming to DDIS contains everything needed to implement the described system — architecture, algorithms, invariants, decisions, test strategies, performance budgets, negative constraints, and execution plan — in a single cohesive document where every section earns its place by serving the sections around it, without requiring the implementer to guess at unstated requirements.

> Document note (important):
> This standard is **self-bootstrapping**: it is written in the format it defines.
> Every structural element prescribed by DDIS is demonstrated in this document.
> Where this document says "the spec must include X," this document includes X — about itself.
> Code blocks are design sketches for illustration. The correctness contract lives in the
> invariants, not in any particular syntax.
> **LLM implementers**: treat invariants as ground truth. When a code sketch contradicts an invariant, the invariant wins.

> How to use this standard (practical):
> 1) Read **PART 0** once end-to-end: understand what DDIS requires, why, and how elements connect.
> 2) Lock your spec's **churn-magnets** via ADRs before writing implementation sections.
> 3) Write your spec following the **Document Structure** (§0.3), using PART II as the element-by-element reference.
> 4) For each implementation chapter, include **negative specifications** (what the subsystem must NOT do) and a **verification prompt** (self-check for the implementer). See §3.8 and §5.6.
> 5) Validate against the **Quality Gates** (§0.7) — including Gate 7 (LLM Implementation Readiness) and Gate 8 (Specification Testability) — and the **Completeness Checklist** (Part X) before considering the spec "done."
> 6) Treat the **cross-reference web** as a product requirement, not polish — it is the mechanism that makes the spec cohere. Cross-references must **restate substance**, not just cite IDs (INV-018). Use **machine-readable syntax** (§10.3) to enable automated validation.
> 7) If your spec exceeds **2,500 lines** or your target LLM's context window, read **§0.13 (Modularization Protocol)** and decompose into a manifest-driven module structure.
> 8) If your system depends on another DDIS-specified system, follow the **Composability Protocol** (§0.2.5) for cross-spec references.

---

# PART 0: EXECUTIVE BLUEPRINT

## 0.1 Executive Summary

DDIS (Decision-Driven Implementation Specification) is a standard for writing technical specifications that bridge the gap between architectural vision and correct implementation.

Most specifications fail in one of two ways: they are too abstract (the implementer must guess at critical details) or too mechanical (they prescribe code without explaining why, making evolution impossible). DDIS avoids both failure modes by requiring a **causal chain** from first principles through decisions to implementation details, where every element justifies its existence by serving the elements around it.

DDIS 2.0 added a third failure axis: **LLM hallucination**. When a large language model implements from a spec, it fills unspecified gaps with plausible behavior from its training data. DDIS introduced negative specifications, verification prompts, meta-instructions, and structural redundancy to close these gaps.

DDIS 3.0 addresses a fourth axis: **specification testability and composability**. Real systems are built from multiple specs that must reference each other. And specifications themselves need automated validation — not just manual gate reviews. DDIS 3.0 adds machine-readable cross-references (INV-022, ADR-013), a composability protocol (§0.2.5), automated specification testing (§12.3), ADR confidence levels (ADR-012), and incremental authoring support (§11.3).

DDIS synthesizes techniques from several traditions — Architecture Decision Records (Nygard), Design by Contract (Meyer), temporal invariants (Lamport), performance budgeting (game engine development), test-driven specification, and LLM-optimized document engineering — into a unified document structure. The synthesis is the contribution: these techniques are well-known individually but rarely composed into a single coherent standard.

### 0.1.1 What DDIS Is

DDIS is a document standard. It specifies:

- What structural elements a specification must contain
- How those elements must relate to each other (the cross-reference web)
- What quality criteria each element must meet
- How to validate that a specification is complete — both manually and automatically
- How to structure elements so that LLM implementers produce correct output on the first pass
- How multiple DDIS specs compose when systems depend on each other

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
  ADRs reference invariants. Invariants reference tests. Tests reference performance budgets. Cross-references restate the substance of what they reference ([[INV-018|substance restated at point of use]]), not just the identifier.

- **The document is self-contained**
  A competent implementer with the spec and the spec alone — no oral tradition, no Slack threads, no "ask the architect" — can build a correct v1. If they cannot, the spec has failed.

- **LLM implementers succeed without hallucinating**
  An LLM reading any implementation chapter (with the glossary and restated invariants) can produce a correct implementation without inventing requirements not in the spec. Negative specifications (§3.8) explicitly close the gap between what is specified and what an LLM might plausibly add. (Justified by §0.2.3; validated by Gate 7.)

- **The specification is automatically testable**
  Cross-reference integrity, proportional weight, invariant completeness, and reference staleness can be validated by tooling, not just human review. (Justified by §0.2.5; validated by Gate 8.)

### 0.1.3 Non-Goals (Explicit)

DDIS does not attempt:

- **To replace code.** A spec describes what to build, why, and how to verify it — not the literal source code. Design sketches illustrate intent; they are not copy-paste targets.
- **To eliminate judgment.** Implementers will make thousands of micro-decisions. DDIS constrains the macro-decisions so micro-decisions are locally safe.
- **To be a project management framework.** DDIS includes a Master TODO and phased roadmap, but these are execution aids, not a substitute for sprint planning.
- **To prescribe notation.** DDIS requires formal models but does not mandate TLA+, Alloy, Z, or any specific formalism. Any precise formalism is acceptable.
- **To guarantee correctness.** A DDIS-conforming spec dramatically reduces the chance of building the wrong thing. It cannot eliminate it.
- **To prevent all LLM hallucination.** Negative specifications and verification prompts reduce hallucination for the most plausible misinterpretations. They cannot prevent all possible LLM errors — only the structurally predictable ones.
- **To prescribe tooling.** Machine-readable cross-reference syntax (§10.3) enables tooling but DDIS does not mandate any specific validation tool.

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

1. **Completeness over elegance.** A verbose spec that leaves nothing ambiguous is better than a terse spec that leaves critical details to inference. (But see [[INV-007|every section earns its place]]: verbosity without structure is noise.)

2. **Decisions over descriptions.** The hardest part of building a system is not writing code — it is making the hundreds of design decisions that determine whether the code is correct. A spec that describes a system without recording why it is shaped that way is a snapshot, not a blueprint.

3. **Verifiability over trust.** Every claim in the spec must be testable. "The system is fast" is not verifiable. "Event ingestion completes in < 100µs p99 at the design point of 300 agents / 10K tasks, measured by Benchmark B-001" is verifiable.

4. **Exclusion over implication.** For LLM implementers, what a spec does NOT say is as dangerous as what it says incorrectly. An LLM will fill unspecified gaps with plausible behavior. Explicit exclusions (negative specifications) are a safety mechanism, not optional polish. (Justified by §0.2.3.)

5. **Automation over ceremony.** Validation that can be automated should be automated. Manual gate reviews catch semantic issues; automated testing catches structural issues. Both are required. (New in 3.0; justified by §0.2.5.)

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
| LLM adds features not in the spec | Hallucinated requirements | Negative specifications (§3.8, [[INV-017|negative spec per chapter]]) |
| LLM loses context mid-chapter | Implements against stale invariants | Structural redundancy at point of use ([[INV-018|restate substance]]) |
| LLM implements in wrong order | Integration failures | Meta-instructions with ordering (§5.7, [[INV-020|explicit sequencing]]) |
| LLM doesn't self-check | Subtle invariant violations ship | Verification prompts per chapter (§5.6, [[INV-019|self-check]]) |
| Cross-refs rot silently | Stale restatements mislead | Machine-readable refs + automated testing (§10.3, [[INV-022|parseable refs]]) |
| Dependent specs contradict | Integration failures across systems | Composability protocol (§0.2.5) |

```
First Principles (formal model of the problem)
  ↓ justifies
Non-Negotiables + Invariants (what must always be true)
  ↓ constrained by
Architecture Decision Records (choices that could go either way)
  ↓ implemented via
Algorithms + Data Structures + Protocols (pseudocode, state machines)
  ↓ bounded by
Negative Specifications (what must NOT be built)
  ↓ verified by
Test Strategies + Performance Budgets + Verification Prompts
  ↓ validated by
Automated Spec Tests + Quality Gates (structural + semantic checks)
  ↓ shipped via
Master TODO (execution checklist)
```

Every element in DDIS exists because removing it causes a specific, named failure. There are no decorative sections.

### 0.2.3 LLM Consumption Model

DDIS formally recognizes that the primary implementer is often a large language model. This section provides the theoretical foundation for all LLM-specific provisions in the standard.

**The Hallucination Gap.** Given a spec S describing system Σ, define the *specified space* as the set of behaviors S explicitly prescribes or excludes. The *implementation space* is the set of all behaviors an implementer might produce. The *hallucination gap* is:

```
H(S) = implementation_space(S) - specified_space(S)
```

For human implementers, H(S) is filled by judgment, experience, and questions to the architect. For LLMs, H(S) is filled by training data — plausible but unvalidated behavior. DDIS aims to minimize H(S) through negative specifications that explicitly close the most dangerous regions.

**Three Principles of LLM-Optimized Specification:**

**L1: Minimize the hallucination gap.** Every implementation chapter includes negative specifications (§3.8) that explicitly exclude the most plausible misinterpretations. The spec author thinks adversarially: "What would an LLM add that I didn't ask for?" (Validated by [[INV-017|negative spec per chapter]].)

**L2: Context-window self-sufficiency.** Each implementation chapter is self-contained for LLM consumption. Cross-references restate the substance of the referenced constraint, not just its ID. An LLM processing a chapter in isolation — without access to earlier chapters — can still produce correct output. (Validated by [[INV-018|substance restated at point of use]].)

**L3: Active verification over passive specification.** Each implementation chapter ends with a verification prompt (§5.6) the LLM can use to self-check its output. Passive specification says "the system must do X." Active verification says "check: does your implementation do X? Does it avoid Y?" (Validated by [[INV-019|verification prompt per chapter]].)

**LLM-Specific Failure Modes and Mitigations:**

| LLM Failure Mode | Mitigation | DDIS Element |
|---|---|---|
| Fills unspecified gaps with training data | Negative specifications | §3.8, [[INV-017|negative spec per chapter]] |
| Loses context in long documents | Structural redundancy at point of use | [[INV-018|substance restated]], ADR-009 |
| Over-indexes on examples | High quality bar for worked examples | §5.2 |
| Cannot follow "see above" | Explicit section refs with substance | [[INV-018|substance restated]], Chapter 10 |
| Implements in arbitrary order | Meta-instructions with ordering | §5.7, [[INV-020|explicit sequencing]] |
| Does not self-check | Verification prompts | §5.6, [[INV-019|self-check per chapter]] |
| Produces hedged, vague prose | Voice guidance counteracts | §8.1 |
| Interprets ambiguous terms randomly | Glossary with disambiguation | [[INV-009|glossary coverage]], §7.1 |

### 0.2.4 Fundamental Operations of a Specification

Every specification, regardless of domain, performs these operations:

| Operation | What It Does | DDIS Element |
|---|---|---|
| **Define** | Establish what the system IS, formally | First-principles model, formal types |
| **Constrain** | State what must always hold | Invariants, non-negotiables |
| **Decide** | Lock choices where alternatives exist | ADRs (with confidence level) |
| **Describe** | Specify how components work | Algorithms, state machines, protocols |
| **Exemplify** | Show the system in action | Worked examples, end-to-end traces |
| **Bound** | Set measurable limits | Performance budgets, design point |
| **Verify** | Define how to confirm correctness | Test strategies, quality gates, verification prompts |
| **Exclude** | State what the system is NOT | Non-goals, negative specifications |
| **Sequence** | Order the work | Phased roadmap, meta-instructions |
| **Lexicon** | Define terminology | Glossary |
| **Compose** | Reference external specs | Composability protocol (§0.2.5) |

### 0.2.5 Composability Model

When System B depends on System A, and both have DDIS specs, those specs must reference each other without creating hidden dependencies or contradictions.

**The Composability Problem.** A DDIS spec is self-contained ([[INV-008|spec answers all implementation questions]]). But when System B's spec says "uses System A's event API," System B's spec now depends on System A's spec. If System A's spec changes, System B's spec may silently break.

**Composability Principles:**

**C1: External specs are referenced by stable contract, not internal section.** System B references System A's published invariants and API surface, never its internal implementation chapters.

```
BAD:  "See System A spec, §7.3 for the event schema"
GOOD: "Consumes events conforming to System A's APP-INV-017
       (append-only event log, schema v2.1)"
```

**C2: Cross-spec invariants are declared explicitly.** When System B depends on System A's invariant, System B's spec declares this as an *external dependency* with the invariant substance restated:

```
External Dependencies:
- System A, APP-INV-017: Event log is append-only (events, once written,
  are never modified or deleted). Schema version: 2.1.
  Impact if violated: System B's replay mechanism fails.
```

**C3: Version pinning.** Cross-spec references pin to a specific spec version. "System A spec v2.3, APP-INV-017" — not just "System A's event invariant."

**C4: Composition validation.** When both specs are available, check: (a) every external dependency references an invariant that exists in the target spec at the pinned version, (b) no external dependency contradicts the target spec's negative specifications.

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
  §0.2  First-Principles Derivation (formal model + LLM consumption model)
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
  §0.14 External Dependencies (cross-spec composability)        [Conditional]

PART I: FOUNDATIONS
  First-principles derivation (full formal model)
  State machines for all stateful components
  Complexity analysis for fundamental operations
  End-to-end trace (of the spec's own authoring process, for meta-specs)

PART II: CORE IMPLEMENTATION (the heart of the spec)
  One chapter per major subsystem, each containing:
    - Formal types (data model)
    - Algorithm pseudocode
    - State machine (if stateful)
    - Invariants this subsystem must preserve (with substance restated)
    - Negative specifications (what this subsystem must NOT do)
    - Worked example(s)
    - WHY NOT annotations on non-obvious choices
    - Test strategy
    - Performance budget for this subsystem
    - Cross-references to ADRs, invariants, other subsystems (with substance)
    - Meta-instructions (implementation ordering guidance)
    - Verification prompt (self-check for implementer)
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
  D: Specification Error Taxonomy
  E: Benchmark Scenarios                                   [Optional]
  F: Reference Implementations / Extracted Code            [Optional]

PART X: MASTER TODO INVENTORY
  Checkboxable task list organized by subsystem
  Cross-referenced to phases, ADRs, and quality gates
```

### 0.3.1 Ordering Rationale

The ordering follows the **dependency chain of understanding**:

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

// WHY NOT separate PART naming for meta-standard vs domain specs? This standard is self-bootstrapping (ADR-004): it uses its own structure. PART III in domain specs is "Interfaces" — in this meta-standard, the "interface" DDIS exposes is its guidance to authors. The structural roles are analogous even though the content differs.

## 0.5 Invariants of the DDIS Standard

Every DDIS-conforming specification must satisfy these invariants. Each invariant has an identifier, a plain-language statement, a formal expression, a violation scenario, and a validation method.

---

**INV-001: Causal Traceability**

*Every implementation section traces to at least one ADR or invariant, which traces to the formal model.*

```
∀ section ∈ PART_II:
  ∃ adr ∈ ADRs ∪ inv ∈ Invariants:
    section.references(adr ∨ inv) ∧ (adr ∨ inv).derives_from(formal_model)
```

Violation scenario: An implementation chapter describes a caching layer with no ADR justifying the caching decision and no invariant constraining cache behavior. Six months later, nobody knows if the cache can be removed.

Validation: Pick 5 random implementation sections. For each, follow cross-references backward to an ADR or invariant, then to the formal model. If any chain breaks, INV-001 is violated.

// WHY THIS MATTERS: Without traceability, sections accumulate by accretion without justification.

---

**INV-002: Decision Completeness**

*Every design choice where a reasonable alternative exists is captured in an ADR.*

```
∀ choice ∈ spec where ∃ alternative ∧ alternative.is_reasonable:
  ∃ adr ∈ ADRs: adr.covers(choice) ∧ adr.alternatives.contains(alternative)
```

Violation scenario: The spec uses event sourcing but no ADR compares it to CRUD or state-based persistence. A new team member spends a week refactoring to CRUD before discovering the implicit decision.

Validation: Adversarial review. A reviewer reads each implementation section and asks "could this reasonably be done differently?" If yes and no ADR exists, INV-002 is violated.

---

**INV-003: Invariant Falsifiability**

*Every invariant can be violated by a concrete scenario and detected by a named test.*

```
∀ inv ∈ Invariants:
  ∃ scenario: scenario.violates(inv) ∧
  ∃ test ∈ TestStrategy: test.detects(scenario)
```

Violation scenario: An invariant states "the system is reliable" with no concrete violation scenario and no test. It cannot be violated because it cannot be tested.

Validation: For each invariant, construct a counterexample. If no counterexample can be constructed, the invariant is either trivially true (remove it) or too vague (sharpen it).

---

**INV-004: Algorithm Completeness**

*Every described algorithm includes: pseudocode, complexity analysis, at least one worked example, and error/edge case handling.*

```
∀ algorithm ∈ spec:
  algorithm.has(pseudocode) ∧ algorithm.has(complexity_analysis) ∧
  algorithm.has(worked_example) ∧ algorithm.has(edge_cases)
```

Violation scenario: An algorithm section describes "the scheduler picks the highest-priority ready task" in prose but provides no pseudocode, no complexity bound, and no example of what happens when two tasks have equal priority.

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

Violation scenario: The spec claims "event ingestion is fast" with no benchmark, no design point, and no measurement method.

Validation: For each performance number, locate the benchmark. If the benchmark doesn't exist or doesn't describe how to run it, INV-005 is violated.

---

**INV-006: Cross-Reference Density**

*The specification contains a cross-reference web where no section is an island.*

```
∀ section ∈ spec (excluding Preamble, Glossary):
  section.outgoing_references.count ≥ 1 ∧
  section.incoming_references.count ≥ 1
```

Violation scenario: An implementation chapter for the "notification subsystem" has no references to any ADR, invariant, or other chapter. It exists in isolation.

Validation: Build a directed graph of cross-references. Every non-trivial section must have at least one inbound and one outbound edge. When using machine-readable syntax ([[INV-022|parseable cross-refs]]), this check is fully automated.

// WHY THIS MATTERS: Cross-references are the mechanism that prevents a spec from devolving into a collection of independent essays.

---

**INV-007: Signal-to-Noise Ratio**

*Every section earns its place by serving at least one other section or preventing a named failure mode.*

```
∀ section ∈ spec:
  ∃ justification:
    (section.serves(other_section) ∨ section.prevents(named_failure_mode))
```

Violation scenario: A section titled "Historical Context" provides interesting background but is never referenced by any implementation section, ADR, or invariant.

Validation: For each section, state in one sentence why removing it would make the spec worse. If you cannot, remove the section.

---

**INV-008: Self-Containment**

*The specification, combined with the implementer's general programming competence and domain knowledge available in public references, is sufficient to build a correct v1.*

```
∀ implementation_question Q:
  spec.answers(Q) ∨ Q.answerable_from(general_competence ∪ public_references)
```

Violation scenario: The spec references "the standard event replay protocol" without defining it, and no public standard exists. The implementer must guess.

Validation: Give the spec to a competent engineer unfamiliar with the project. Track every question they ask. If questions reveal information that should be in the spec, INV-008 is violated.

---

**INV-009: Glossary Coverage**

*Every domain-specific term used in the specification is defined in the glossary.*

```
∀ term ∈ spec where term.is_domain_specific:
  ∃ entry ∈ Glossary: entry.defines(term)
```

Violation scenario: The spec uses "advisory lock" throughout but the glossary doesn't define it. An LLM interprets it as a POSIX flock rather than the spec's intended meaning (a soft reservation with no enforcement).

Validation: Extract all non-common-English terms from the spec. Check each against the glossary.

---

**INV-010: State Machine Completeness**

*Every state machine defines all states, all transitions, all guards, and behavior for invalid transitions.*

```
∀ sm ∈ StateMachines:
  sm.has(all_states) ∧ sm.has(all_transitions) ∧
  sm.has(guards_per_transition) ∧ sm.has(invalid_transition_policy)
```

Violation scenario: A task state machine defines states {Ready, InProgress, Done} and transitions {Ready→InProgress, InProgress→Done} but doesn't specify what happens if a "complete" event arrives when the task is in Ready state.

Validation: For each state machine, enumerate the state × event cross-product. Every cell must either name a transition or explicitly state "invalid — [policy]."

---

**INV-017: Negative Specification Coverage**

*Every implementation chapter includes at least one negative specification — an explicit "must NOT" constraint that prevents the most plausible misinterpretation an LLM or inexperienced implementer would make.*

```
∀ chapter ∈ implementation_chapters:
  chapter.negative_specs.count ≥ 1 ∧
  ∀ neg ∈ chapter.negative_specs: neg.is_plausible_misinterpretation
```

Violation scenario: A scheduler implementation chapter describes task dispatching but never states "must NOT hold hard locks" or "must NOT read wall-clock time." An LLM implements mutex-based scheduling with timestamps, breaking the advisory-only concurrency model and deterministic replay.

Validation: For each implementation chapter, check that at least one "must NOT" constraint exists. For each negative spec, verify it describes a plausible (not absurd) misinterpretation.

// WHY THIS MATTERS: LLMs fill unspecified space with plausible behavior from training data. Negative specifications are the primary defense against hallucinated requirements. (Justified by LLM Consumption Model §0.2.3; locked by ADR-008.)

---

**INV-018: Structural Redundancy at Point of Use**

*Every cross-reference restates the substance of the referenced constraint, not just its identifier. An implementer reading a single chapter can understand the constraints without navigating to their source definitions.*

```
∀ ref ∈ cross_references where ref.target ∈ {Invariants, ADRs}:
  ref.includes_substance = true
```

Violation scenario: An implementation chapter says "Preserves INV-003" with no description. An LLM implementing in isolation doesn't know what INV-003 requires and either ignores it or hallucinates its content.

Validation: Scan all cross-references to invariants and ADRs. Each must include a one-sentence summary of the substance. "Preserves [[INV-003|same event sequence → identical final state]]" passes. "Preserves INV-003" alone fails.

// WHY THIS MATTERS: LLMs may process chapters in isolation or lose early context in long documents. ID-only references are useless without the definition in context. This trades DRY for LLM self-sufficiency per chapter. (Locked by ADR-009.)

---

**INV-019: Verification Prompt Coverage**

*Every implementation chapter ends with a verification prompt: a structured self-check referencing specific invariants, negative specifications, and worked examples from that chapter.*

```
∀ chapter ∈ implementation_chapters:
  chapter.has(verification_prompt) ∧
  chapter.verification_prompt.references_invariants ∧
  chapter.verification_prompt.references_negative_specs
```

Violation scenario: An implementation chapter for the event store has no verification prompt. The LLM produces code that subtly violates the append-only invariant by allowing compaction. No self-check catches this during generation.

Validation: For each implementation chapter, verify a verification prompt exists and that it references at least one invariant and one negative specification from the same chapter.

// WHY THIS MATTERS: Verification prompts convert passive specification into active self-checking. An LLM that follows a verification prompt catches errors during generation rather than at review. (Justified by §0.2.3, Principle L3; locked by ADR-010.)

---

**INV-020: Meta-Instruction Explicitness**

*Every implementation chapter with ordering dependencies includes meta-instructions that make the implementation sequence and strategy explicit.*

```
∀ chapter ∈ implementation_chapters where chapter.has_ordering_dependencies:
  chapter.has(meta_instructions) ∧
  ∀ mi ∈ chapter.meta_instructions: mi.has(rationale)
```

Violation scenario: The storage layer chapter has implicit dependencies (event log must exist before snapshots, read path depends on both) but provides no implementation ordering guidance. An LLM implements snapshots first, then discovers it can't test them without the event log.

Validation: For each implementation chapter, check whether ordering dependencies exist (via cross-reference analysis). If they do, verify meta-instructions are present with rationale.

// WHY THIS MATTERS: LLMs implement in whatever order they encounter content. Without explicit ordering, dependent subsystems may be implemented before their dependencies, causing integration failures. (Justified by §0.2.3.)

---

**INV-021: Proportional Weight Compliance**

*The specification's section lengths conform to the proportional weight guide (§0.8.2) within a tolerance band, ensuring no section is starved or bloated relative to its structural role.*

```
∀ part ∈ {PART_0, PART_I, PART_II, PART_III, PART_IV, APPENDICES}:
  |actual_weight(part) - target_weight(part)| ≤ tolerance(part)
where:
  tolerance(part) = max(10%, 0.5 × target_weight(part))
```

Violation scenario: PART II (the heart of the spec) contains 15% of the document while PART 0 contains 55%. The implementation chapters — where most bugs are prevented — are starved of detail while the executive blueprint bloats with content that belongs elsewhere.

Validation: Compute line counts per PART. Compare against §0.8.2 targets. Deviations exceeding the tolerance band require a WHY NOT annotation justifying the imbalance.

// WHY THIS MATTERS: Weight imbalance is the #1 structural signal of a spec that has drifted from its purpose. Bloated overviews with thin implementation chapters produce incomplete implementations. (New in 3.0.)

// SELF-BOOTSTRAPPING NOTE: This meta-standard's PART 0 exceeds the prescribed 15-20% target because the standard's own invariants, ADRs, and modularization protocol constitute its core content — what would be distributed across implementation chapters in a domain spec. This deviation is acknowledged and justified per this invariant's tolerance mechanism.

---

**INV-022: Machine-Readable Cross-References**

*Cross-references use a parseable syntax that enables automated graph construction, staleness detection, and density validation.*

```
∀ ref ∈ cross_references where ref.target ∈ {Invariants, ADRs, Sections}:
  ref.matches(machine_readable_pattern) ∧
  ref.target_id.exists_in(spec)
```

Machine-readable pattern: `[[TARGET-ID|substance]]` where TARGET-ID is an invariant (INV-NNN), ADR (ADR-NNN), or section (§N.N) identifier.

Violation scenario: A spec uses inconsistent reference styles — some "see INV-003," others "(INV-003: determinism)," others "per the determinism invariant." Automated tooling cannot build a reference graph because references are unparseable.

Validation: Regex scan for `\[\[.+?\|.+?\]\]` pattern. Every cross-reference to an invariant or ADR must match. Every TARGET-ID must resolve to an existing element. (Automated by Gate 8.)

// WHY THIS MATTERS: Without parseable references, cross-reference validation (INV-006), staleness detection (Appendix D), and density checks are manual-only — meaning they don't happen in practice. Machine-readable refs turn spec quality from a review concern into a CI concern. (New in 3.0; locked by ADR-013.)

---

**INV-011: Module Completeness** [Conditional — modular specs only]

*An LLM receiving a properly assembled bundle can implement the module's subsystem without information from any other module's implementation content.*

```
∀ module ∈ modules:
  let bundle = ASSEMBLE(module)
  ∀ implementation_question Q about module's subsystem:
    bundle.answers(Q) ∨ Q.answerable_from(general_competence)
```

Violation scenario: The Scheduler module references EventStore's internal ring buffer layout to determine batching strategy, but the ring buffer details live only in the EventStore module.

Validation: Give a bundle (not the full spec) to an LLM. Track questions that require information from another module's implementation. Any such question violates INV-011.

// WHY THIS MATTERS: If module completeness fails, the modularization protocol provides no benefit.

---

**INV-012: Cross-Module Isolation** [Conditional — modular specs only]

*Modules reference each other only through constitutional elements (invariants, ADRs, shared types). No module contains direct references to another module's internal sections.*

```
∀ module_a, module_b ∈ modules where module_a ≠ module_b:
  ∀ ref ∈ module_a.outbound_references:
    ref.target ∉ module_b.internal_sections ∧
    ref.target ∈ {constitution, shared_types, invariants, ADRs}
```

Violation scenario: The TUI Renderer module says "use the same batching strategy as the EventStore module's flush_batch() function."

Validation: Mechanical (CHECK-7 in §0.13.11). Semantic: review for implicit references that bypass the constitution.

// WHY THIS MATTERS: If modules reference each other's internals, Module A's bundle needs Module B's implementation — defeating modularization. (Locked by ADR-007.)

---

**INV-013: Invariant Ownership Uniqueness** [Conditional — modular specs only]

*Every application invariant is maintained by exactly one module (or explicitly by the system constitution).*

```
∀ inv ∈ invariant_registry:
  (inv.owner = "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 0)
  ∨ (inv.owner ≠ "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 1)
```

Violation scenario: Both EventStore and SnapshotManager list APP-INV-017 in their maintains declarations. Which module's tests are authoritative?

Validation: Mechanical (CHECK-1 in §0.13.11).

// WHY THIS MATTERS: Ownership uniqueness prevents accountability gaps.

---

**INV-014: Bundle Budget Compliance** [Conditional — modular specs only]

*Every assembled bundle fits within the hard ceiling defined in the manifest's context budget.*

```
∀ module ∈ modules:
  line_count(ASSEMBLE(module)) ≤ context_budget.hard_ceiling_lines
```

Violation scenario: Scheduler module grows to 3,500 lines. With 1,200-line constitutional context, the bundle is 4,700 lines — under the 5,000 hard ceiling but over the 4,000 target.

Validation: Mechanical (CHECK-5 in §0.13.11).

// WHY THIS MATTERS: Budget violations mean the modularization added complexity without delivering the benefit.

---

**INV-015: Declaration-Definition Consistency** [Conditional — modular specs only]

*Every invariant declaration in the system constitution is a faithful summary of its full definition.*

```
∀ inv ∈ invariant_registry:
  let decl = system_constitution.declaration(inv)
  let defn = full_definition(inv)
  decl.id = defn.id ∧ decl.one_line is_faithful_summary_of defn.statement
```

Violation scenario: System constitution declares "APP-INV-017: Event log is append-only" but the full definition now says "append-only except during compaction windows." An LLM implementing a different domain sees only the declaration and codes against the wrong contract.

Validation: Semi-mechanical. Extract declaration/definition pairs, present to reviewer for semantic consistency.

// WHY THIS MATTERS: Divergence between tiers means different modules implement against different understandings of the same invariant.

---

**INV-016: Manifest-Spec Synchronization** [Conditional — modular specs only]

*The manifest accurately reflects the current state of all spec files.*

```
∀ path ∈ manifest.all_referenced_paths: file_exists(path)
∀ inv ∈ manifest.all_referenced_invariants: inv ∈ system_constitution
∀ module_file ∈ filesystem("modules/"): module_file ∈ manifest
```

Violation scenario: Author adds `modules/new_feature.md` but forgets to add it to the manifest. The assembly script never produces a bundle for it.

Validation: Mechanical (CHECK-9 in §0.13.11).

// WHY THIS MATTERS: A file not in the manifest is invisible to all tooling.
