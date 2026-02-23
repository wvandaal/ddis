---
module: system-constitution
domain: system
tier: 1
description: >
  System Constitution (Tier 1) — included in EVERY bundle.
  Contains orientation, first principles, invariant/ADR/gate declarations,
  and essential glossary. Sufficient to understand WHAT exists; modules
  provide HOW.
ddis_version: "3.0"
tier_mode: two-tier
---

# DDIS: Decision-Driven Implementation Specification Standard

## Version 3.0 — A Self-Bootstrapping Meta-Specification

> Design goal: **A formal standard for writing implementation specifications that are precise enough for an LLM or junior engineer to implement correctly without guessing, while remaining readable enough that a senior engineer would choose to read them voluntarily.**

> Core promise: A DDIS-conforming specification contains everything needed to implement the described system — architecture, algorithms, invariants, decisions, test strategies, performance budgets, and execution plan — in a single cohesive document where every section earns its place by serving the sections around it, and where explicit constraints prevent LLM hallucination at every decision point.

> Document note (important):
> This standard is **self-bootstrapping**: it is written in the format it defines.
> Every structural element prescribed by DDIS is demonstrated in this document.
> Where this document says "the spec must include X," this document includes X — about itself.
> Code blocks are design sketches for illustration. The correctness contract lives in the
> invariants, not in any particular syntax.

> How to use this standard (practical):
> 1) Read **PART 0** end-to-end: understand what DDIS requires, why, and how elements connect.
> 2) Lock your spec's **churn-magnets** via ADRs before writing implementation sections.
> 3) Write your spec following the **Document Structure** (§0.3), using PART II as the element-by-element reference.
> 4) Validate against the **Quality Gates** (§0.7) and the **Completeness Checklist** (Part X) before considering the spec "done."
> 5) Treat the **cross-reference web** as a product requirement, not polish — it is the mechanism that makes the spec cohere.
> 6) If your spec exceeds **2,500 lines** or your target LLM's context window, read **§0.13 (Modularization Protocol)** and decompose into a manifest-driven module structure.
> 7) If the primary implementer is an **LLM**, ensure every implementation chapter includes negative specifications (§3.8), verification prompts (§5.6), and meta-instructions (§5.7). Read §0.2.2 for the formal consumption model.
> 8) Verify that every element specification chapter includes a **verification prompt block** (§5.6, INV-020).

---

# PART 0: EXECUTIVE BLUEPRINT

## 0.1 Executive Summary

DDIS (Decision-Driven Implementation Specification) is a standard for writing technical specifications that bridge architectural vision and correct implementation. The primary optimization target is **LLM consumption**: the primary implementer will be a large language model.

Most specifications fail in one of two ways: too abstract (the implementer guesses at critical details) or too mechanical (they prescribe code without explaining why, making evolution impossible). DDIS avoids both by requiring a **causal chain** from first principles through decisions to implementation details.

When the implementer is an LLM, a third failure mode emerges: the LLM **hallucinates** plausible details not in the spec, or **forgets** invariants defined far from the implementation section. DDIS addresses this with structural provisions woven throughout: negative specifications (§3.8), structural redundancy at point of use (INV-018), verification prompts (§5.6), and meta-instructions (§5.7). (Locked by ADR-008.)

DDIS synthesizes Architecture Decision Records (Nygard), Design by Contract (Meyer), temporal invariants (Lamport), performance budgeting (game engine development), and test-driven specification into a unified document structure.

### 0.1.1 What DDIS Is

DDIS is a document standard. It specifies:

- What structural elements a specification must contain
- How those elements must relate to each other (the cross-reference web)
- What quality criteria each element must meet
- How to validate that a specification is complete
- How to structure elements for optimal LLM consumption (§0.2.2)

DDIS is domain-agnostic. It applies to any system where correctness matters and multiple people (or LLMs) will implement from the spec.

### 0.1.2 Non-Negotiables (Engineering Contract)

These are not aspirations; they are the contract. If any are violated, a document is not a DDIS-conforming specification.

- **Causal chain is unbroken**
  Every implementation detail traces back through a decision, through an invariant, to a first principle. If you cannot trace a section's ancestry to the formal model, the section is unjustified.

- **Decisions are explicit and locked**
  Every design choice that could reasonably go another way is captured in an ADR with genuine alternatives considered. "We chose X" without "we rejected Y because Z" is not a decision — it is an assertion.

- **Invariants are falsifiable**
  Every invariant can be violated by a concrete scenario and detected by a concrete test. An invariant that cannot be tested is a wish, not a contract.

- **No implementation detail is unsupported**
  Every algorithm, data structure, state machine, and protocol has: pseudocode or formal description, complexity analysis, at least one worked example, and a test strategy.

- **Cross-references form a web, not a list**
  ADRs reference invariants. Invariants reference tests. Tests reference performance budgets. Performance budgets reference the design point. The design point references first principles.

- **The document is self-contained**
  A competent implementer with the spec and the spec alone can build a correct v1. If they cannot, the spec has failed.

- **Negative specifications prevent hallucination**
  Every implementation chapter states what the subsystem must NOT do, not merely what it must do. This is the primary defense against LLM hallucination and human assumption. (See §3.8, INV-017.)

### 0.1.3 Non-Goals (Explicit)

DDIS does not attempt:

- **To replace code.** A spec describes what to build, why, and how to verify it — not the literal source code.
- **To eliminate judgment.** DDIS constrains macro-decisions so micro-decisions are locally safe.
- **To be a project management framework.** The Master TODO and phased roadmap are execution aids, not a substitute for sprint planning.
- **To prescribe notation.** DDIS requires formal models but does not mandate TLA+, Alloy, Z, or any specific formalism.
- **To guarantee correctness.** A DDIS-conforming spec dramatically reduces the chance of building the wrong thing but cannot eliminate it.
- **To optimize for a specific LLM.** DDIS provisions target structural properties benefiting all transformer-based models, not prompt-engineering tricks for a particular model family.

## 0.2 First-Principles Derivation

### 0.2.1 What IS an Implementation Specification?

A specification is a function from intent to artifact:

```
Spec: (Problem, Constraints, Knowledge) -> Document
where:
  Document enables: Implementer x Document -> Correct_System
```

The quality of a specification is measured by one criterion: **does an implementer produce a correct system from it, without requiring information not in the document?**

Consequences:

1. **Completeness over elegance.** A verbose spec that leaves nothing ambiguous beats a terse spec that leaves critical details to inference. (But see INV-007: verbosity without structure is noise.)

2. **Decisions over descriptions.** The hardest part of building a system is making the hundreds of design decisions that determine whether the code is correct. A spec that describes without recording why is a snapshot, not a blueprint.

3. **Verifiability over trust.** Every claim must be testable. "The system is fast" is not verifiable. "Event ingestion completes in < 100us p99 at the design point of 300 agents / 10K tasks, measured by Benchmark B-001" is verifiable.

### 0.2.2 LLM Consumption Model

An LLM consuming a DDIS spec operates under constraints fundamentally different from a human reader. This model is the formal justification for INV-017 through INV-020, ADR-008 through ADR-011, and Gate 7.

**LLM implementer constraints and DDIS mitigations:**

| Constraint | Failure Mode | DDIS Mitigation |
|---|---|---|
| Fixed context window | Spec competes with reasoning for token budget | Modularization (§0.13); proportional weight (§0.8.2) |
| No random access | Cannot "flip back" to check a definition | Structural redundancy: restate key invariants at point of use (INV-018) |
| Hallucination tendency | Fills gaps with plausible but incorrect details | Negative specifications: explicit "DO NOT" per subsystem (§3.8, INV-017) |
| Example over-indexing | Treats worked examples as authoritative templates | Quality bar for examples higher than human specs; anti-patterns mandatory |
| Implicit reference failure | "See above" resolves to wrong or lost context | All cross-refs use explicit §X.Y, INV-NNN, ADR-NNN identifiers (INV-006) |
| No clarification channel | Cannot ask the architect a question mid-implementation | Self-containment at chapter granularity (INV-008), not just document level |
| Instruction-following capability | Can execute embedded directives | Verification prompts (§5.6) and meta-instructions (§5.7) |

**Formal model of LLM consumption:**

```
LLM_Implement: (Spec_Fragment, Context_Budget) -> Implementation

where:
  Correctness = f(
    completeness(Spec_Fragment),
    absence_of_hallucination_triggers,
    explicit_negative_constraints,
    structural_redundancy_at_point_of_use
  )

  hallucination_triggers = {
    gap: exists question Q: not Spec_Fragment.answers(Q) and Q.is_architectural,
    ambiguity: exists statement S: |interpretations(S)| > 1,
    implicit_reference: exists ref R: R.target not_in Spec_Fragment
  }
```

**Consequence 1:** A spec exceeding the context window is equivalent to an incomplete spec. This motivates the modularization protocol (§0.13).

**Consequence 2:** A spec lacking negative specifications will produce implementations with plausible but unauthorized behaviors. This motivates INV-017 and §3.8.

**Consequence 3:** A spec relying on cross-references without restating critical context at point of use will produce subtle inconsistencies. This motivates INV-018.

**Consequence 4:** A spec without implementation ordering guidance forces the LLM to choose an order that may violate dependency chains. This motivates INV-019 and §5.7.

### 0.2.3 The Causal Chain (Why DDIS Is Structured This Way)

DDIS prescribes a specific document structure because specifications fail in predictable ways, and each structural element prevents a named failure mode:

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
| LLM hallucinates unauthorized behavior | Implementation includes features not in spec | Negative specifications (§3.8, INV-017) |
| LLM forgets invariant from distant section | Subtle invariant violation | Structural redundancy at point of use (INV-018) |
| LLM implements in wrong order | Cascading rework | Implementation ordering directives (§5.7, INV-019) |

```
First Principles (formal model of the problem)
  | justifies
Non-Negotiables + Invariants (what must always be true)
  | constrained by
Architecture Decision Records (choices that could go either way)
  | implemented via
Algorithms + Data Structures + Protocols (pseudocode, state machines)
  | bounded by
Negative Specifications (what must NOT be done)
  | verified by
Test Strategies + Performance Budgets + Verification Prompts
  | shipped via
Quality Gates + Master TODO (stop-ship criteria, execution checklist)
```

Every element in DDIS exists because removing it causes a specific, named failure. There are no decorative sections.

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
| **Verify** | Define how to confirm correctness | Test strategies, quality gates, verification prompts (§5.6) |
| **Exclude** | State what the system is NOT and must NOT do | Non-goals, scope boundaries, negative specifications (§3.8) |
| **Sequence** | Order the work | Phased roadmap, meta-instructions (§5.7), decision spikes, first PRs |
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

PART I: FOUNDATIONS
  First-principles derivation (full formal model)
  State machines for all stateful components
  Complexity analysis for fundamental operations
  End-to-end trace (one authored element through the full DDIS process)
  "Why this architecture is inevitable" narrative

PART II: CORE IMPLEMENTATION (the heart of the spec)
  One chapter per major subsystem, each containing:
    - Formal types (data model)
    - Algorithm pseudocode
    - State machine (if stateful)
    - Invariants this subsystem must preserve (RESTATED, not just referenced -- INV-018)
    - Negative specifications (what this subsystem must NOT do -- §3.8, INV-017)
    - Worked example(s)
    - WHY NOT annotations on non-obvious choices
    - Test strategy
    - Performance budget for this subsystem
    - Verification prompt (LLM self-check -- §5.6)
    - Meta-instructions (implementation ordering -- §5.7)          [If applicable]
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
    - Minimal deliverables order (with dependency chain -- INV-019)
    - Immediate next steps (first PRs)
  Agent/tool compatibility notes                           [Optional]

APPENDICES
  A: Glossary (every domain term, cross-referenced)
  B: Risk Register (risks + mitigations)
  C: Specification Error Taxonomy
  D: Quick-Reference Card
  E: Storage / Wire Formats                                [Optional]
  F: Benchmark Scenarios                                   [Optional]
  G: Reference Implementations / Extracted Code            [Optional]

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

> **META-INSTRUCTION (for LLM implementers):** When implementing from a DDIS spec, read PART 0 in full before beginning any implementation chapter. Do not skip the invariants or ADRs — they constrain every decision you will make. When implementing a specific subsystem, re-read the invariants listed in that chapter's header before writing code.

## 0.4 This Standard's Architecture

DDIS has a simple ring architecture:

1. **Core Standard (sacred)**: The mandatory structural elements, their required contents, quality criteria, and relationships. (PART 0, PART I, PART II of this document.)

2. **Guidance (recommended)**: Voice, proportional weight, anti-patterns, worked examples. These improve spec quality but their absence does not make a spec non-conforming. (PART III of this document.)

3. **Tooling (optional)**: Checklists, templates, validation procedures. (PART IV, Appendices.)

---

## Invariant Declarations

All 20 invariants of the DDIS standard. Full definitions with formal expressions, violation scenarios, and validation methods are in the core-standard module.

| ID | Statement | Domain | Owner |
|---|---|---|---|
| INV-001 | Every implementation section traces to at least one ADR or invariant, which traces to the formal model | core | system |
| INV-002 | Every design choice where a reasonable alternative exists is captured in an ADR | core | system |
| INV-003 | Every invariant can be violated by a concrete scenario and detected by a named test | core | system |
| INV-004 | Every described algorithm includes pseudocode, complexity analysis, worked example, and edge cases | core | system |
| INV-005 | Every performance claim is tied to a specific benchmark, design point, and measurement methodology | core | system |
| INV-006 | The specification contains a cross-reference web where no section is an island | core | system |
| INV-007 | Every section earns its place by serving at least one other section or preventing a named failure mode | core | system |
| INV-008 | The specification is self-contained: sufficient for a correct v1 without external oral tradition | core | system |
| INV-009 | Every domain-specific term used in the specification is defined in the glossary | core | system |
| INV-010 | Every state machine defines all states, all transitions, all guards, and behavior for invalid transitions | core | system |
| INV-011 | [Conditional] An LLM receiving a properly assembled bundle can implement the module's subsystem without information from any other module | modularization | system |
| INV-012 | [Conditional] Modules reference each other only through constitutional elements, never direct internal references | modularization | system |
| INV-013 | [Conditional] Every application invariant is maintained by exactly one module | modularization | system |
| INV-014 | [Conditional] Every assembled bundle fits within the hard ceiling defined in the manifest's context budget | modularization | system |
| INV-015 | [Conditional] Every invariant declaration in the system constitution is a faithful summary of its full definition | modularization | system |
| INV-016 | [Conditional] The manifest accurately reflects the current state of all spec files | modularization | system |
| INV-017 | Every implementation chapter includes explicit "DO NOT" constraints preventing likely hallucination patterns | core | system |
| INV-018 | Every implementation chapter restates the invariants it must preserve, not merely referencing them by ID | core | system |
| INV-019 | The spec provides an explicit dependency chain for implementation ordering | core | system |
| INV-020 | Every element specification chapter includes a structured verification prompt block | core | system |

---

## ADR Declarations

All 11 ADRs of the DDIS standard. Full specifications with Problem, Options, Decision, WHY NOT, Consequences, and Tests are in the core-standard module.

| ID | Title | Decision (one sentence) |
|---|---|---|
| ADR-001 | Document Structure Is Fixed, Not Flexible | Fixed structure: a reader who has seen one DDIS spec can navigate any other. |
| ADR-002 | Invariants Must Be Falsifiable, Not Merely True | Falsifiable invariants with plain-language statement, semi-formal expression, violation scenario, and validation method. |
| ADR-003 | Cross-References Are Mandatory, Not Optional Polish | Required: every non-trivial section must have inbound and outbound references using explicit identifiers. |
| ADR-004 | Self-Bootstrapping as Validation Strategy | Self-bootstrapping: this document is both the standard and its first conforming instance. |
| ADR-005 | Voice Is Specified, Not Left to Author Preference | Voice guidance: technically precise but human, a senior engineer explaining to a peer they respect. |
| ADR-006 | Tiered Constitution over Flat Root | Three-tier as full protocol with two-tier as blessed simplification for small specs. |
| ADR-007 | Cross-Module References Through Constitution Only | Through constitution only: INV-012 enforces this mechanically. |
| ADR-008 | LLM Provisions Woven Throughout, Not Isolated | Woven throughout: LLM provisions integrated into each element specification. |
| ADR-009 | Negative Specifications as Formal Elements | Formal negative specification blocks required per implementation chapter. |
| ADR-010 | Verification Prompts per Implementation Chapter | Verification prompts per chapter: structured self-check at the end of each implementation chapter. |
| ADR-011 | ADR Supersession Protocol | Mark-and-supersede with cross-reference cascade preserving historical record. |

---

## Quality Gate Declarations

All 12 quality gates (7 standard + 5 modularization). Full descriptions are in the core-standard module; modularization gates M-1 through M-5 are detailed in the modularization module.

| Gate | What It Checks |
|---|---|
| Gate 1: Structural Conformance | All required elements from §0.3 present, including negative specs, verification prompts, meta-instructions |
| Gate 2: Causal Chain Integrity | Five random implementation sections trace backward to the formal model without breaks (INV-001) |
| Gate 3: Decision Coverage | Adversarial reviewer identifies zero "obvious alternatives" not covered by an ADR (INV-002) |
| Gate 4: Invariant Falsifiability | Every invariant has a constructible counterexample and a named test (INV-003) |
| Gate 5: Cross-Reference Web | The reference graph has no orphan sections and is connected (INV-006) |
| Gate 6: Implementation Readiness | A competent implementer can begin without clarifying questions about architecture |
| Gate 7: LLM Implementation Readiness | LLM given one chapter produces no hallucinated requirements and preserves all invariants (INV-017, INV-018, INV-019) |
| Gate M-1: Consistency Checks | All nine mechanical checks (CHECK-1 through CHECK-9) pass with zero errors (INV-012, INV-013, INV-014, INV-016) |
| Gate M-2: Bundle Budget Compliance | Every assembled bundle is under the hard ceiling; fewer than 20% exceed target (INV-014) |
| Gate M-3: LLM Bundle Sufficiency | LLM receiving one bundle produces zero questions requiring another module's content (INV-011) |
| Gate M-4: Declaration-Definition Faithfulness | Every Tier 1 declaration is a faithful summary of its full definition (INV-015) |
| Gate M-5: Cascade Simulation | A simulated change correctly identifies all affected modules via the cascade protocol (INV-016) |

---

## 0.8 Performance Budgets (Summary)

### 0.8.1 Specification Size Budgets

| System Complexity | Target Spec Length | Rationale |
|---|---|---|
| Small (single crate, < 5K LOC target) | 500-1,500 lines | Enough for formal model + invariants + key ADRs |
| Medium (multi-crate, 5K-50K LOC target) | 1,500-5,000 lines | Full DDIS treatment |
| Large (multi-service, > 50K LOC target) | 5,000-15,000 lines | May split into sub-specs linked by a master |

### 0.8.2 Proportional Weight Guide

| Section | % of Total | Why |
|---|---|---|
| Preamble + PART 0 | 15-20% | Dense: formal model, invariants, ADRs, quality gates |
| PART I: Foundations | 8-12% | First principles, state machines, complexity analysis |
| PART II: Core Implementation | 35-45% | THE HEART: algorithms, data structures, protocols, examples, negative specs, verification prompts |
| PART III: Interfaces | 8-12% | API schemas, adapters, external contracts |
| PART IV: Operations | 10-15% | Testing, operational playbook, roadmap |
| Appendices + Part X | 10-15% | Reference material, glossary, error taxonomy, master TODO |

---

## Essential Glossary

Core terms used across all modules. The full glossary (51 terms) is in the guidance-operations module (Appendix A).

| Term | Definition |
|---|---|
| **ADR** | Architecture Decision Record. A structured record of a design choice, including alternatives considered and rationale. (See §3.5) |
| **Bundle** | Assembled document for LLM implementation of one module: constitution + module. The unit of LLM consumption. (See §0.13.2) |
| **Causal chain** | The traceable path from a first principle through an invariant and/or ADR to an implementation detail. (See §0.2.3, INV-001) |
| **Constitution** | Cross-cutting material constraining all modules. In two-tier mode: system constitution only. (See §0.13.3) |
| **Cross-reference** | An explicit link between two sections using §X.Y, INV-NNN, or ADR-NNN identifiers. (See Chapter 10, INV-006) |
| **DDIS** | Decision-Driven Implementation Specification. This standard. |
| **Declaration** | A compact (1-line) summary of an invariant or ADR in the system constitution. (See §0.13.4) |
| **Design point** | The specific hardware, workload, and scale scenario against which performance budgets are validated. (See §3.7) |
| **Falsifiable** | A property of an invariant: it can be violated by a concrete scenario and detected by a concrete test. (See INV-003, ADR-002) |
| **First principles** | The formal model of the problem domain from which the architecture derives. (See §3.3) |
| **Gate** | A quality gate: a stop-ship predicate that must be true before the project can proceed. (See §3.6) |
| **Hallucination** | An LLM failure mode where the model generates plausible but unauthorized behaviors. Prevented by negative specifications (§3.8). (See §0.2.2) |
| **Invariant** | A numbered, falsifiable property that must hold at all times during system operation. (See §3.4) |
| **Manifest** | Machine-readable YAML declaring all modules, invariant ownership, and assembly rules. (See §0.13.9) |
| **Meta-instruction** | A directive to the LLM implementer providing ordering, sequencing, or process guidance. (See §5.7) |
| **Module** | Self-contained spec unit covering one major subsystem. Always assembled into a bundle. (See §0.13.2) |
| **Negative specification** | Explicit "DO NOT" constraint co-located with the implementation chapter. Primary defense against LLM hallucination. (See §3.8, INV-017) |
| **Self-bootstrapping** | A property of this standard: it is written in the format it defines. (See ADR-004) |
| **Structural redundancy** | Restating key invariants at their point of use to prevent context loss. Required by INV-018. (See §0.2.2) |
| **Verification prompt** | A structured self-check prompt at the end of an implementation chapter. (See §5.6, ADR-010) |
