---
module: system-constitution
domain: system
tier: 1
description: >
  System Constitution (Tier 1) — included in EVERY bundle.
  Contains orientation, first principles summary, invariant/ADR/gate declarations,
  glossary, and cross-cutting concerns. Sufficient to understand WHAT exists;
  modules provide HOW.
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

Most specifications fail in one of two ways: too abstract (the implementer guesses at critical details) or too mechanical (they prescribe code without explaining why, making evolution impossible). DDIS avoids both by requiring a **causal chain** from first principles through decisions to implementation details, where every element justifies its existence by serving the elements around it.

When the implementer is an LLM, a third failure mode emerges: the LLM **hallucinates** plausible details not in the spec, or **forgets** invariants defined far from the implementation section. DDIS addresses this with structural provisions woven throughout: negative specifications (§3.8), structural redundancy at point of use (INV-018), verification prompts (§5.6), and meta-instructions (§5.7). These are integral to every element specification, not an add-on. (Locked by ADR-008.)

DDIS synthesizes Architecture Decision Records (Nygard), Design by Contract (Meyer), temporal invariants (Lamport), performance budgeting (game engine development), and test-driven specification into a unified document structure. These techniques are well-known individually but rarely composed into a single coherent standard.

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
  Every algorithm, data structure, state machine, and protocol has: pseudocode or formal description, complexity analysis, at least one worked example, and a test strategy. Prose descriptions of behavior without any of these are insufficient.

- **Cross-references form a web, not a list**
  ADRs reference invariants. Invariants reference tests. Tests reference performance budgets. Performance budgets reference the design point. The design point references first principles. A specification where sections exist in isolation is a collection of essays, not a DDIS spec.

- **The document is self-contained**
  A competent implementer with the spec and the spec alone — no oral tradition, no Slack threads, no "ask the architect" — can build a correct v1. If they cannot, the spec has failed.

- **Negative specifications prevent hallucination**
  Every implementation chapter states what the subsystem must NOT do, not merely what it must do. This is the primary defense against LLM hallucination and human assumption. (See §3.8, INV-017.)

### 0.1.3 Non-Goals (Explicit)

DDIS does not attempt:

- **To replace code.** A spec describes what to build, why, and how to verify it — not the literal source code. Design sketches illustrate intent; they are not copy-paste targets.
- **To eliminate judgment.** Implementers make thousands of micro-decisions. DDIS constrains macro-decisions (architecture, algorithms, invariants) so micro-decisions are locally safe.
- **To be a project management framework.** The Master TODO and phased roadmap are execution aids, not a substitute for sprint planning or issue tracking.
- **To prescribe notation.** DDIS requires formal models but does not mandate TLA+, Alloy, Z, or any specific formalism. Pseudocode, state machines, mathematical notation, or "close to [language]" sketches are all acceptable if precise.
- **To guarantee correctness.** A DDIS-conforming spec dramatically reduces the chance of building the wrong thing but cannot eliminate it. The spec is a contract for intent, not a machine-checked proof.
- **To optimize for a specific LLM.** DDIS provisions target structural properties benefiting all transformer-based models (context window management, explicit constraints, structural predictability), not prompt-engineering tricks for a particular model family.

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

1. **Completeness over elegance.** A verbose spec that leaves nothing ambiguous beats a terse spec that leaves critical details to inference. (But see INV-007: verbosity without structure is noise.)

2. **Decisions over descriptions.** The hardest part of building a system is making the hundreds of design decisions that determine whether the code is correct. A spec that describes without recording why is a snapshot, not a blueprint.

3. **Verifiability over trust.** Every claim must be testable. "The system is fast" is not verifiable. "Event ingestion completes in < 100µs p99 at the design point of 300 agents / 10K tasks, measured by Benchmark B-001" is verifiable.

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
LLM_Implement: (Spec_Fragment, Context_Budget) → Implementation

where:
  Correctness = f(
    completeness(Spec_Fragment),
    absence_of_hallucination_triggers,
    explicit_negative_constraints,
    structural_redundancy_at_point_of_use
  )

  hallucination_triggers = {
    gap: ∃ question Q: ¬Spec_Fragment.answers(Q) ∧ Q.is_architectural,
    ambiguity: ∃ statement S: |interpretations(S)| > 1,
    implicit_reference: ∃ ref R: R.target ∉ Spec_Fragment
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
  ↓ justifies
Non-Negotiables + Invariants (what must always be true)
  ↓ constrained by
Architecture Decision Records (choices that could go either way)
  ↓ implemented via
Algorithms + Data Structures + Protocols (pseudocode, state machines)
  ↓ bounded by
Negative Specifications (what must NOT be done)
  ↓ verified by
Test Strategies + Performance Budgets + Verification Prompts
  ↓ shipped via
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
    - Invariants this subsystem must preserve (RESTATED, not just referenced — INV-018)
    - Negative specifications (what this subsystem must NOT do — §3.8, INV-017)
    - Worked example(s)
    - WHY NOT annotations on non-obvious choices
    - Test strategy
    - Performance budget for this subsystem
    - Verification prompt (LLM self-check — §5.6)
    - Meta-instructions (implementation ordering — §5.7)          [If applicable]
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
    - Minimal deliverables order (with dependency chain — INV-019)
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

**Meta-standard PART mapping**: When DDIS is applied to a meta-standard (a standard about standards), PARTs are renamed: PART II → Element Specifications, PART III → Guidance, PART IV → Authoring & Validation. This document demonstrates this mapping.

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

## Invariant Registry (Declarations)

All 20 invariants of the DDIS standard. Full definitions with formal expressions, violation scenarios, and validation methods are in the owning module.

| ID | Statement | Conditional | Owner Module |
|---|---|---|---|
| INV-001 | Every implementation section traces to at least one ADR or invariant, which traces to the formal model | No | core-standard |
| INV-002 | Every design choice where a reasonable alternative exists is captured in an ADR | No | core-standard |
| INV-003 | Every invariant can be violated by a concrete scenario and detected by a named test | No | core-standard |
| INV-004 | Every described algorithm includes pseudocode, complexity analysis, worked example, and edge cases | No | core-standard |
| INV-005 | Every performance claim is tied to a specific benchmark, design point, and measurement methodology | No | core-standard |
| INV-006 | The specification contains a cross-reference web where no section is an island | No | core-standard |
| INV-007 | Every section earns its place by serving at least one other section or preventing a named failure mode | No | core-standard |
| INV-008 | The specification is self-contained: sufficient for a correct v1 without external oral tradition | No | core-standard |
| INV-009 | Every domain-specific term used in the specification is defined in the glossary | No | core-standard |
| INV-010 | Every state machine defines all states, all transitions, all guards, and behavior for invalid transitions | No | core-standard |
| INV-011 | An LLM receiving a properly assembled bundle can implement the module's subsystem without information from any other module's implementation content | Conditional — modular specs only | modularization |
| INV-012 | Modules reference each other only through constitutional elements, never direct internal references | Conditional — modular specs only | modularization |
| INV-013 | Every application invariant is maintained by exactly one module | Conditional — modular specs only | modularization |
| INV-014 | Every assembled bundle fits within the hard ceiling defined in the manifest's context budget | Conditional — modular specs only | modularization |
| INV-015 | Every invariant declaration in the system constitution is a faithful summary of its full definition | Conditional — modular specs only | modularization |
| INV-016 | The manifest accurately reflects the current state of all spec files | Conditional — modular specs only | modularization |
| INV-017 | Every implementation chapter includes explicit "DO NOT" constraints preventing likely hallucination patterns | No | core-standard |
| INV-018 | Every implementation chapter restates the invariants it must preserve, not merely referencing them by ID | No | core-standard |
| INV-019 | The spec provides an explicit dependency chain for implementation ordering | No | core-standard |
| INV-020 | Every element specification chapter includes a structured verification prompt block | No | core-standard |

---

## ADR Registry (Declarations)

All 11 ADRs of the DDIS standard. Full specifications with Problem, Options, Decision, WHY NOT, Consequences, and Tests are in the implementing module.

| ID | Title | Decision (one-line) | Conditional | Implementing Module |
|---|---|---|---|---|
| ADR-001 | Document Structure Is Fixed, Not Flexible | Fixed structure: a reader who has seen one DDIS spec can navigate any other | No | core-standard |
| ADR-002 | Invariants Must Be Falsifiable, Not Merely True | Falsifiable invariants with plain-language statement, semi-formal expression, violation scenario, and validation method | No | core-standard |
| ADR-003 | Cross-References Are Mandatory, Not Optional Polish | Required: every non-trivial section must have inbound and outbound references using explicit identifiers | No | core-standard |
| ADR-004 | Self-Bootstrapping as Validation Strategy | Self-bootstrapping: this document is both the standard and its first conforming instance | No | core-standard |
| ADR-005 | Voice Is Specified, Not Left to Author Preference | Voice guidance: technically precise but human, a senior engineer explaining to a peer they respect | No | core-standard |
| ADR-006 | Tiered Constitution over Flat Root | Three-tier as full protocol with two-tier as blessed simplification for small specs | Conditional — modular specs only | modularization |
| ADR-007 | Cross-Module References Through Constitution Only | Through constitution only: INV-012 enforces this mechanically | Conditional — modular specs only | modularization |
| ADR-008 | LLM Provisions Woven Throughout, Not Isolated | Woven throughout: LLM provisions integrated into each element specification | No | core-standard |
| ADR-009 | Negative Specifications as Formal Elements | Formal negative specification blocks required per implementation chapter | No | core-standard |
| ADR-010 | Verification Prompts per Implementation Chapter | Verification prompts per chapter: structured self-check at end of each implementation chapter | No | core-standard |
| ADR-011 | ADR Supersession Protocol | Mark-and-supersede with cross-reference cascade preserving historical record | No | core-standard |

---

## Quality Gates (Summaries)

A DDIS-conforming specification is "done" when all quality gates pass. Gates are ordered by priority; a failing Gate N makes Gates N+1 through 7 irrelevant.

**Gate 1: Structural Conformance**
All required elements from §0.3 present, including negative specifications (§3.8), verification prompts (§5.6), and meta-instructions (§5.7). Every element spec chapter includes a verification prompt block (INV-020). Mechanical check.

**Gate 2: Causal Chain Integrity**
Five random implementation sections trace backward to the formal model without breaks. (Validates INV-001.)

**Gate 3: Decision Coverage**
Adversarial reviewer identifies zero "obvious alternatives" not covered by an ADR. (Validates INV-002.)

**Gate 4: Invariant Falsifiability**
Every invariant has a constructible counterexample and a named test. (Validates INV-003.)

**Gate 5: Cross-Reference Web**
The reference graph has no orphan sections and is connected. (Validates INV-006.)

**Gate 6: Implementation Readiness**
A competent implementer (or LLM), given only the spec and public references, can begin implementing without clarifying questions about architecture, algorithms, data models, or invariants. Micro-level questions (variable names, error message wording) are acceptable.

**Gate 7: LLM Implementation Readiness**
Give ONLY one implementation chapter (plus glossary and relevant invariants) to an LLM. Verify: (a) no hallucinated requirements, (b) no clarifying questions about architecture, (c) all chapter-header invariants preserved, (d) all negative specifications observed. Test on >= 2 representative chapters. (Validates INV-017, INV-018, INV-019.)

> **Gate 7 demonstration (thought experiment):** Give §3.4 plus the glossary to an LLM and ask it to write invariants for a hypothetical system. It should produce the prescribed format (statement, formal expression, violation scenario, validation, WHY THIS MATTERS); NOT produce aspirational invariants (prevented by anti-pattern in §3.4); NOT omit violation scenarios (prevented by negative spec). If correct without hallucinating format elements, Gate 7 passes for §3.4.

### Modularization Quality Gates [Conditional — modular specs only]

In addition to Gates 1-7, modular specs must pass these gates. A failing Gate M-1 makes Gates M-2 through M-5 irrelevant.

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

DDIS 3.0 is "done" when:
- This document passes Gates 1-7 applied to itself
- At least one non-trivial spec has been written conforming to DDIS without structural workarounds
- The Glossary (Appendix A) covers all DDIS-specific terminology
- LLM provisions are demonstrated in this document's own element specifications (self-bootstrapping)

---

## 0.8 Performance Budgets (for Specifications, Not Software)

### 0.8.1 Specification Size Budgets

| System Complexity | Target Spec Length | Rationale |
|---|---|---|
| Small (single crate, < 5K LOC target) | 500-1,500 lines | Enough for formal model + invariants + key ADRs |
| Medium (multi-crate, 5K-50K LOC target) | 1,500-5,000 lines | Full DDIS treatment |
| Large (multi-service, > 50K LOC target) | 5,000-15,000 lines | May split into sub-specs linked by a master |

### 0.8.2 Proportional Weight Guide

Not all sections are equal. These proportions prevent bloat and starvation. Domain-specific specs may adjust by +/-20%.

| Section | % of Total | Why |
|---|---|---|
| Preamble + PART 0 | 15-20% | Dense: formal model, invariants, ADRs, quality gates |
| PART I: Foundations | 8-12% | First principles, state machines, complexity analysis |
| PART II: Core Implementation | 35-45% | THE HEART: algorithms, data structures, protocols, examples, negative specs, verification prompts |
| PART III: Interfaces | 8-12% | API schemas, adapters, external contracts |
| PART IV: Operations | 10-15% | Testing, operational playbook, roadmap |
| Appendices + Part X | 10-15% | Reference material, glossary, error taxonomy, master TODO |

**For meta-standards** (self-bootstrapping specs about specification authoring):

| Section | % of Total | Why |
|---|---|---|
| Preamble + PART 0 | 45-60% | Contains the entire standard definition: invariants, ADRs, gates, modularization |
| PART I: Foundations | 3-6% | Formal model of specifications — concise by nature |
| PART II: Element Specifications | 20-30% | Templates and guidance for each structural element |
| PART III: Guidance | 4-8% | Voice, style, and cross-reference patterns |
| PART IV: Operations | 3-6% | Authoring sequence, validation, evolution |
| Appendices + Part X | 6-12% | Reference material, glossary, error taxonomy, master TODO |

DO NOT apply domain-spec proportions to a meta-standard and flag a violation. See Chapter 9 for diagnostic signals.

---

## 0.9 Public API Surface (of DDIS Itself)

DDIS exposes the following "API" to specification authors:

1. **Document Structure Template** (§0.3) — the skeleton to fill in.
2. **Element Specifications** (PART II) — what each structural element must contain, including LLM-specific provisions.
3. **Quality Criteria** (§0.5 invariants, §0.7 gates) — how to validate conformance.
4. **Voice and Style Guide** (PART III, §8.1) — how to write well within the structure.
5. **Anti-Pattern Catalog** (PART III, §8.3) — what bad specs look like.
6. **Error Taxonomy** (Appendix C) — classification of specification authoring errors.
7. **Completeness Checklist** (Part X) — mechanical conformance validation.

---

## 0.10 Open Questions (for DDIS 3.0)

1. ~~**Machine-readable cross-references**~~ **RESOLVED**: DDIS cross-references use three parseable token formats: `§X.Y`, `INV-NNN`, `ADR-NNN`. Machine-parseable via regex. See §10.1.

2. **Multi-document specs**: For very large systems, how should sub-specs reference each other? What invariants apply across spec boundaries? (Partially addressed by §0.13.)

3. ~~**Spec evolution**~~ **RESOLVED**: ADR-011 defines mark-and-supersede with cross-reference cascade (§13.3).

4. **Formal verification bridge**: Should DDIS define a pathway from falsifiable invariants to machine-checked properties for safety-critical systems?

5. ~~**Confidence levels**~~ **RESOLVED**: ADR format (§3.5) now includes optional Confidence field with three levels: Committed (default), Provisional (revisit after spike), Speculative (needs abstraction boundary).

6. **Composability across specs**: **DEFERRED** — requires ≥ 2 interdependent DDIS-conforming specs to validate. Candidate approach: `[SpecName]:INV-NNN` and `[SpecName]:§X.Y` syntax with shared invariant registry.

---

## Glossary (Compact)

All DDIS-specific terms. Short definitions for orientation; full definitions are in the referenced modules.

> **Authoritative glossary**: Appendix A (guidance-operations module) contains full definitions with cross-references. This compact glossary is a navigational aid — entries here are declarations that summarize Appendix A definitions. If wording diverges, Appendix A is authoritative. (See INV-015.)

| Term | Definition | See Module |
|---|---|---|
| **ADR** | Architecture Decision Record: structured record of a design choice with alternatives and rationale (§3.5) | core-standard |
| **ADR supersession** | Replacing an ADR while preserving the original as historical record; requires cross-reference cascade (ADR-011, §13.3) | core-standard |
| **Assembly script** | Tool that reads the manifest and produces bundles by concatenating tiers with module content (§0.13.10) | modularization |
| **Blast radius** | Set of modules and invariants affected by a change to constitutional content; determines cascade scope (§0.13.12) | modularization |
| **Bundle** | Assembled document for LLM implementation: constitution + module. The unit of LLM consumption (§0.13.2, §0.13.10) | modularization |
| **Cascade protocol** | Procedure for identifying and re-validating modules affected by a change to constitutional content (§0.13.12) | modularization |
| **Causal chain** | Traceable path from a first principle through an invariant and/or ADR to an implementation detail (§0.2.3, INV-001) | core-standard |
| **Churn-magnet** | A decision that, if left open, causes the most downstream rework; ADRs should prioritize locking these (§3.5) | core-standard |
| **Comparison block** | Side-by-side rejected/chosen comparison with quantified reasoning (§5.5) | element-specifications |
| **Confidence level** | Optional ADR field: Committed (default), Provisional (revisit after spike), Speculative (needs abstraction boundary) (§3.5) | core-standard |
| **Constitution** | Cross-cutting material constraining all modules, organized in tiers (§0.13.3) | modularization |
| **Context budget** | Portion of LLM context window available for spec content after reserving space for reasoning (§0.13.9, §0.2.2) | modularization |
| **Cross-cutting module** | Module with domain "cross-cutting" spanning multiple architectural domains, e.g., end-to-end trace (§0.13.6) | modularization |
| **Cross-reference** | Explicit link between sections using §X.Y, INV-NNN, or ADR-NNN identifiers (Chapter 10, INV-006) | guidance-operations |
| **DDIS** | Decision-Driven Implementation Specification. This standard | system-constitution |
| **Decision spike** | Time-boxed experiment that de-risks an unknown and produces an ADR (§6.1.1) | guidance-operations |
| **Declaration** | Compact (1-line) summary of an invariant or ADR in the system constitution (§0.13.4) | modularization |
| **Deep context** | Tier 3: cross-domain invariant definitions and interface contracts for a specific module (§0.13.3) | modularization |
| **Definition** | Full specification of an invariant or ADR including formal expression, violation scenario, validation (§0.13.4) | modularization |
| **Design point** | Specific hardware, workload, and scale scenario against which performance budgets are validated (§3.7) | element-specifications |
| **Design sketch** | Code block illustrating intent and API shape, not copy-paste-ready; correctness lives in invariants (§2.3) | element-specifications |
| **Domain** | Architectural grouping of related modules sharing tighter coupling (§0.13.2) | modularization |
| **Domain constitution** | Tier 2: full invariant definitions and ADR analysis for one architectural domain (§0.13.3) | modularization |
| **End-to-end trace** | Worked scenario traversing all major subsystems, showing data at each boundary (§5.3) | element-specifications |
| **Exit criterion** | Specific, testable condition for phase completion (§6.1.2) | guidance-operations |
| **Falsifiable** | Property of an invariant: can be violated by a concrete scenario and detected by a concrete test (INV-003, ADR-002) | core-standard |
| **First principles** | Formal model of the problem domain from which the architecture derives (§3.3) | core-standard |
| **Formal model** | Mathematical or pseudo-mathematical definition of the system as a state machine or function (§0.2.1) | core-standard |
| **Gate** | Quality gate: stop-ship predicate that must be true before the project can proceed (§3.6) | core-standard |
| **Hallucination** | LLM failure mode: generates plausible but unauthorized behaviors; prevented by negative specifications (§3.8, §0.2.2) | core-standard |
| **Invariant** | Numbered, falsifiable property that must hold at all times during system operation (§3.4) | core-standard |
| **Invariant registry** | Manifest section listing every invariant with owning module, ensuring INV-013 uniqueness (§0.13.9) | modularization |
| **Living spec** | Specification in active use, updated as implementation reveals gaps (§13.1) | guidance-operations |
| **LLM consumption model** | Formal model of how an LLM consumes a DDIS spec, including failure modes and mitigations (§0.2.2) | core-standard |
| **Manifest** | Machine-readable YAML declaring all modules, invariant ownership, and assembly rules (§0.13.9) | modularization |
| **Master TODO** | Checkboxable task inventory cross-referenced to subsystems, phases, and ADRs (§7.3) | guidance-operations |
| **Meta-instruction** | Directive to the LLM implementer providing ordering, sequencing, or process guidance (§5.7) | element-specifications |
| **Module** | Self-contained spec unit covering one major subsystem; always assembled into a bundle (§0.13.2, §0.13.5) | modularization |
| **Module header** | Structured YAML block at module start declaring domain, maintained invariants, interfaces, negative specs (§0.13.5) | modularization |
| **Monolith** | A DDIS spec that exists as a single document, as opposed to a modular spec (§0.13.2) | modularization |
| **Negative specification** | Explicit "DO NOT" constraint co-located with implementation chapter; primary defense against LLM hallucination (§3.8, INV-017) | element-specifications |
| **Non-goal** | Something the system explicitly does not attempt (§3.2) | core-standard |
| **Non-negotiable** | Philosophical commitment stronger than an invariant; defines what the system IS (§3.1) | core-standard |
| **Operational playbook** | Chapter covering how the spec gets converted into shipped software (§6.1) | guidance-operations |
| **Partial regression** | Spec lifecycle transition returning from a later state to Drafted for re-validation of affected sections only (§1.1) | core-standard |
| **Proportional weight** | Line budget guidance preventing bloat in some sections and starvation in others (§0.8.2) | system-constitution |
| **Quality criteria** | Specific, testable properties an element must exhibit to be well-formed; defined per element type (Chapters 2–7) | element-specifications |
| **Reasoning reserve** | Fraction of LLM context window reserved for reasoning, not spec content; default 0.25 (§0.13.9) | modularization |
| **Ring architecture** | DDIS structural organization: Core Standard (sacred), Guidance (recommended), Tooling (optional) (§0.4) | system-constitution |
| **Self-bootstrapping** | Property of this standard: it is written in the format it defines (ADR-004) | core-standard |
| **Signal-to-noise ratio** | Proportion of section content contributing to understanding vs overhead; governed by INV-007 (§0.8.2) | core-standard |
| **Structural redundancy** | Restating key invariants at point of use to prevent context loss; required by INV-018 (§0.2.2) | core-standard |
| **System constitution** | Tier 1: compact declarations of all invariants and ADRs, plus system-wide orientation (§0.13.3) | modularization |
| **Three-tier mode** | Standard modularization: system constitution + domain constitution + cross-domain deep + module (§0.13.7, ADR-006) | modularization |
| **Two-tier mode** | Simplified modularization: system constitution + module, with two variants: full-definition (< 20 items) or declaration-only (bundles within budget) (§0.13.7.1) | modularization |
| **Verification prompt** | Structured self-check prompt at end of implementation chapter (§5.6, ADR-010) | element-specifications |
| **Verification prompt coverage** | Property (INV-020) that every element spec chapter includes a verification prompt block (INV-020) | core-standard |
| **Voice** | Writing style prescribed by DDIS: technically precise but human (§8.1) | guidance-operations |
| **WHY NOT annotation** | Inline comment explaining why a non-obvious alternative was rejected (§5.4) | element-specifications |
| **Worked example** | Concrete scenario with specific values showing a subsystem in action (§5.2) | element-specifications |

---

## Section Map

Cross-reference lookup: which module file contains each section number. The Notes column indicates key cross-module references to help an LLM distinguish self-references from cross-module references.

| Section Range | Module File | Notes |
|---|---|---|
| §0.1–§0.12, Invariant/ADR/Gate declarations, Glossary | constitution/system.md | Cross-cutting: included in every bundle |
| §0.5 (full defs), §0.6 (full ADRs), §0.7 (details), §0.8, §1.1–§1.4 | modules/core-standard.md | References: INV-011–016 from modularization |
| §0.13 (full protocol), INV-011–INV-016, ADR-006–ADR-007 | modules/modularization.md | References: §5.3, §5.5 from element-specifications |
| §2.1–§7.3 | modules/element-specifications.md | References: §6.1.1 from guidance-operations, §1.4 from core-standard |
| §8.1–Part X | modules/guidance-operations.md | References: §3.8, §5.6, §5.7 from element-specifications, §1.1 from core-standard |

---

## Context Budget

```
target_lines: 4000
hard_ceiling_lines: 5000
reasoning_reserve: 0.25
```

---

## Module Map

| Module | Domain | Contents |
|---|---|---|
| **core-standard** | core | Full invariant definitions (INV-001 through INV-010, INV-017 through INV-020), full ADR specifications (ADR-001 through ADR-005, ADR-008 through ADR-011), PART I foundations (formal model, state machines, complexity, end-to-end trace) |
| **element-specifications** | core | PART II element specs: preamble elements (Ch 2), PART 0 elements (Ch 3), PART I elements (Ch 4), PART II elements (Ch 5), negative specifications (§3.8), verification prompts (§5.6), meta-instructions (§5.7) |
| **modularization** | modularization | Full modularization protocol (§0.13): scaling problem, core concepts, tiered constitution, manifest schema, assembly rules, consistency checks, cascade protocol, migration procedure; INV-011 through INV-016; ADR-006, ADR-007 |
| **guidance-operations** | guidance | PART III guidance (voice, style, anti-patterns, proportional weight, cross-reference patterns), PART IV operations (authoring sequence, validation, evolution, testing, error taxonomy), appendices (glossary, risk register, error taxonomy, quick-reference), PART X master TODO |
