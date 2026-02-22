# Module: Core Framework
<!-- domain: spec-science -->
<!-- maintains: INV-001, INV-002, INV-003, INV-004, INV-005, INV-006, INV-007, INV-008, INV-009, INV-010 -->
<!-- interfaces_with: INV-011, INV-012, INV-013, INV-014, INV-015, INV-016 -->
<!-- adjacent: modularization-protocol, element-specifications -->
<!-- budget: 750 lines -->

## Negative Specifications
- This module MUST NOT define modularization-specific procedures (assembly, cascade, validation checks) — those belong to the modularization-protocol module
- This module MUST NOT contain element-by-element authoring guidance or anti-pattern catalogs — those belong to element-specifications and guidance-and-practice
- This module MUST NOT prescribe specific notation systems (TLA+, Alloy, Z) — DDIS is notation-agnostic per §0.1.3

---

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

- **Causal chain is unbroken** — Every implementation detail traces back through a decision, through an invariant, to a first principle.
- **Decisions are explicit and locked** — Every design choice that could reasonably go another way is captured in an ADR with genuine alternatives considered.
- **Invariants are falsifiable** — Every invariant can be violated by a concrete scenario and detected by a concrete test.
- **No implementation detail is unsupported** — Every algorithm, data structure, state machine, and protocol has: pseudocode, complexity analysis, at least one worked example, and a test strategy.
- **Cross-references form a web, not a list** — ADRs reference invariants. Invariants reference tests. Tests reference performance budgets. The design point references first principles.
- **The document is self-contained** — A competent implementer with the spec alone can build a correct v1.

### 0.1.3 Non-Goals (Explicit)

- **To replace code.** A spec is not an implementation.
- **To eliminate judgment.** DDIS constrains macro-decisions so micro-decisions are locally safe.
- **To be a project management framework.** The Master TODO and roadmap are execution aids, not sprint planning.
- **To prescribe notation.** DDIS requires formal models but does not mandate any specific formalism.
- **To guarantee correctness.** The spec is a contract for intent, not a machine-checked proof.

## 0.2 First-Principles Derivation

### 0.2.1 What IS an Implementation Specification?

A specification is a function from intent to artifact:

```
Spec: (Problem, Constraints, Knowledge) → Document
where:
  Document enables: Implementer × Document → Correct_System
```

The quality of a specification is measured by one criterion: **does an implementer produce a correct system from it, without requiring information not in the document?**

Consequences:

1. **Completeness over elegance.** A verbose spec that leaves nothing ambiguous is better than a terse spec that leaves critical details to inference. (But see INV-007: verbosity without structure is noise.)
2. **Decisions over descriptions.** The hardest part is making the hundreds of design decisions that determine correctness.
3. **Verifiability over trust.** Every claim must be testable.

### 0.2.2 The Causal Chain

| Failure Mode | Symptom | DDIS Element That Prevents It |
|---|---|---|
| Implementer builds the wrong abstraction | Core types don't fit the domain | First-principles formal model (§0.2) |
| Two implementers make incompatible choices | Modules don't compose | Architecture Decision Records (§0.6) |
| System works but violates a safety property | Subtle correctness bugs | Numbered invariants with tests (§0.5) |
| System is correct but too slow | Performance death by a thousand cuts | Performance budgets with benchmarks (§0.8) |
| Nobody knows if the system is "done" | Infinite refinement | Quality gates + Definition of Done (§0.7) |
| New contributor can't understand the system | Oral tradition required | Cross-reference web + glossary |
| Spec covers happy path but not edge cases | Production failures on unusual inputs | Worked examples + end-to-end traces |
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

### 0.2.3 Fundamental Operations of a Specification

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
  §0.13 Modularization Protocol (specs > context window)   [Conditional]

PART I: FOUNDATIONS
PART II: CORE IMPLEMENTATION (per subsystem)
PART III: INTERFACES
PART IV: OPERATIONS
APPENDICES (Glossary, Risks, Formats, Benchmarks)
PART X: MASTER TODO INVENTORY
```

### 0.3.1 Ordering Rationale

The ordering follows the **dependency chain of understanding**: first principles → invariants → ADRs → implementation → interfaces → operations → appendices. An implementer reading top-to-bottom builds understanding incrementally. No section requires forward references to be understood.

## 0.4 This Standard's Architecture

DDIS has a simple ring architecture:

1. **Core Standard (sacred)**: The mandatory structural elements, their required contents, quality criteria, and relationships.
2. **Guidance (recommended)**: Voice, proportional weight, anti-patterns, worked examples. Absence does not make a spec non-conforming.
3. **Tooling (optional)**: Checklists, templates, validation procedures.

## 0.5 Invariants of the DDIS Standard

**INV-001: Causal Traceability**
*Every implementation section traces to at least one ADR or invariant, which traces to the formal model.*
```
∀ section ∈ PART_II:
  ∃ adr ∈ ADRs ∪ inv ∈ Invariants:
    section.references(adr ∨ inv) ∧ (adr ∨ inv).derives_from(formal_model)
```
Validation: Pick 5 random implementation sections. Follow cross-references backward. If any chain breaks, INV-001 is violated.

**INV-002: Decision Completeness**
*Every design choice where a reasonable alternative exists is captured in an ADR.*
Validation: Adversarial review — ask "could this reasonably be done differently?" for each implementation section.

**INV-003: Invariant Falsifiability**
*Every invariant can be violated by a concrete scenario and detected by a named test.*
Validation: Construct a counterexample for each invariant.

**INV-004: Algorithm Completeness**
*Every described algorithm includes: pseudocode, complexity analysis, at least one worked example, and error/edge case handling.*
Validation: Mechanical check for four required components.

**INV-005: Performance Verifiability**
*Every performance claim is tied to a specific benchmark scenario, a design point, and a measurement methodology.*
Validation: Locate the benchmark for each performance number.

**INV-006: Cross-Reference Density**
*No section is an island — every non-trivial section has at least one inbound and one outbound reference.*
Validation: Build a directed graph. Orphan sections violate INV-006.

**INV-007: Signal-to-Noise Ratio**
*Every section earns its place by serving at least one other section or preventing a named failure mode.*
Validation: State in one sentence why removing each section would make the spec worse.

**INV-008: Self-Containment**
*The specification, combined with general programming competence, is sufficient to build a correct v1.*
Validation: Give spec to unfamiliar engineer, track every question revealing missing information.

**INV-009: Glossary Coverage**
*Every domain-specific term used in the specification is defined in the glossary.*
Validation: Extract non-common-English terms, check against glossary.

**INV-010: State Machine Completeness**
*Every state machine defines all states, all transitions, all guards, and behavior for invalid transitions.*
Validation: Enumerate state × event cross-product; every cell must be filled.

## 0.6 Architecture Decision Records

### ADR-001: Document Structure Is Fixed, Not Flexible
**Decision**: Fixed structure. The value of DDIS is that a reader who has seen one DDIS spec can navigate any other. (Validated by INV-001, INV-006.)

### ADR-002: Invariants Must Be Falsifiable, Not Merely True
**Decision**: Falsifiable invariants — formal enough to test, informal enough to read. Each includes plain-language statement, semi-formal expression, violation scenario, and validation method.
// WHY NOT full formal verification? Because the goal is implementation correctness by humans and LLMs, not machine-checked proofs.

### ADR-003: Cross-References Are Mandatory, Not Optional Polish
**Decision**: Required. Every non-trivial section must have inbound and outbound references. (Validated by INV-006.)

### ADR-004: Self-Bootstrapping as Validation Strategy
**Decision**: Self-bootstrapping — this document is both the standard and its first conforming instance.

### ADR-005: Voice Is Specified, Not Left to Author Preference
**Decision**: Voice guidance specified — technically precise but human, the voice of a senior engineer explaining to a peer they respect.

## 0.7 Quality Gates

**Gate 1: Structural Conformance** — All required elements from §0.3 present.
**Gate 2: Causal Chain Integrity** — Five random sections trace backward without breaks. (Validates INV-001.)
**Gate 3: Decision Coverage** — Zero "obvious alternatives" uncovered by ADRs. (Validates INV-002.)
**Gate 4: Invariant Falsifiability** — Every invariant has a constructible counterexample. (Validates INV-003.)
**Gate 5: Cross-Reference Web** — No orphan sections; graph is connected. (Validates INV-006.)
**Gate 6: Implementation Readiness** — Implementer can begin without clarifying questions about architecture, algorithms, data models, or invariants.

### Definition of Done (for this standard)

DDIS 1.0 is "done" when: this document passes Gates 1–6 applied to itself; at least one non-trivial spec has been written conforming to DDIS; the Glossary covers all DDIS-specific terminology.

## 0.8 Performance Budgets

### Specification Size Budgets

| System Complexity | Target Spec Length | Rationale |
|---|---|---|
| Small (< 5K LOC target) | 500–1,500 lines | Formal model + invariants + key ADRs |
| Medium (5K–50K LOC target) | 1,500–5,000 lines | Full DDIS treatment |
| Large (> 50K LOC target) | 5,000–15,000 lines | May split into sub-specs |

### Proportional Weight Guide

| Section | % of Total | Why |
|---|---|---|
| Preamble + PART 0 | 15–20% | Dense: formal model, invariants, ADRs, quality gates |
| PART I: Foundations | 8–12% | First principles, state machines, complexity analysis |
| PART II: Core Implementation | 35–45% | THE HEART |
| PART III: Interfaces | 8–12% | API schemas, adapters |
| PART IV: Operations | 10–15% | Testing, operational playbook |
| Appendices + Part X | 10–15% | Reference material |

### Authoring Time Budgets

| Element | Expected Time | Notes |
|---|---|---|
| First-principles model | 2–4 hours | Hardest part |
| One invariant | 15–30 minutes | Including violation scenario |
| One ADR | 30–60 minutes | Including alternative analysis |
| One implementation chapter | 2–4 hours | Including algorithm, examples, tests |
| End-to-end trace | 1–2 hours | Requires all subsystems drafted first |
| Glossary | 1–2 hours | Best done last |

## 0.9 Public API Surface

DDIS exposes: Document Structure Template (§0.3), Element Specifications (PART II), Quality Criteria (§0.5, §0.7), Voice and Style Guide, Anti-Pattern Catalog, and Completeness Checklist (Part X).

## PART I: Foundations

### 1.1 A Specification as a State Machine

```
States:
  Skeleton  → Drafted → Threaded → Gated → Validated → Living
Invalid: Skeleton → Gated, Drafted → Validated
Living → Drafted (partial regression on gap discovery)
```

### 1.2 Completeness Properties

**Safety**: No contradictory prescriptions between sections.
**Liveness**: The spec eventually answers every architectural question.

### 1.3 Complexity of Specification Elements

| Element | Authoring | Reading | Verification |
|---|---|---|---|
| Invariant | O(domain_understanding) | O(1) | O(1) per counterexample |
| ADR | O(alternatives × depth) | O(alternatives) | O(1) |
| Algorithm | O(complexity × edge_cases) | O(pseudocode_length) | O(worked_examples) |
| Cross-reference | O(1) | O(1) | O(sections²) for full graph |
| End-to-end trace | O(subsystems × interactions) | O(subsystems) | O(1) |
