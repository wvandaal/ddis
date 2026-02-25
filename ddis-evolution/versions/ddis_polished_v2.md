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
|---|---|
| Fixed context window | Spec competes with reasoning for token budget | Modularization (§0.13); proportional weight (§0.8.2) |
| No random access | Cannot "flip back" to check a definition | Structural redundancy: restate key invariants at point of use (INV-018) |
| Hallucination tendency | Fills gaps with plausible but incorrect details | Negative specifications: explicit "DO NOT" per subsystem (§3.8, INV-017) |
| Example over-indexing | Treats worked examples as authoritative templates | Quality bar for examples higher than human specs; anti-patterns mandatory |
| Implicit reference failure | "See above" resolves to wrong or lost context | All cross-refs use explicit §X.Y, INV-NNN, ADR-NNN identifiers (INV-006) |
| No clarification channel | Cannot ask the architect a question mid-implementation | Self-containment at chapter granularity (INV-008), not just document level |
| Instruction-following capability | Can execute embedded directives | Verification prompts (§5.6) and meta-instructions (§5.7) |

**Consequence 1:** A spec exceeding the context window is equivalent to an incomplete spec. This motivates the modularization protocol (§0.13).

**Consequence 2:** A spec lacking negative specifications will produce implementations with plausible but unauthorized behaviors. This motivates INV-017 and §3.8.

**Consequence 3:** A spec relying on cross-references without restating critical context at point of use will produce subtle inconsistencies. This motivates INV-018.

**Consequence 4:** A spec without implementation ordering guidance forces the LLM to choose an order that may violate dependency chains. This motivates INV-019 and §5.7.

### 0.2.3 The Causal Chain (Why DDIS Is Structured This Way)

DDIS prescribes a specific document structure because specifications fail in predictable ways, and each structural element prevents a named failure mode:

| Failure Mode | Symptom | DDIS Element That Prevents It |
|---|---|
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

Every element in DDIS exists because removing it causes a specific, named failure. There are no decorative sections.

### 0.2.4 Fundamental Operations of a Specification

Every specification, regardless of domain, performs these operations:

| Operation | What It Does | DDIS Element |
|---|---|
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

Meta-standard adaptation: When DDIS is applied to itself, §0.11 (Non-Negotiables)
and §0.12 (Non-Goals) are nested under §0.1 as §0.1.2 and §0.1.3 because they are
integral to the executive blueprint. This nesting is valid — the template prescribes
content obligations, not numbering rigidity. (Validates ADR-004.)

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

## §0.5 Invariant Registry (Declarations)

All 20 invariants of the DDIS standard. Full definitions with formal expressions, violation scenarios, and validation methods are in the owning module.

| ID | Statement | Conditional | Owner Module |
|---|---|---|
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
| INV-017 | Every implementation chapter includes explicit "DO NOT" constraints preventing likely hallucination patterns for that subsystem | No | core-standard |
| INV-018 | Every implementation chapter restates the invariants it must preserve, not merely referencing them by ID | No | core-standard |
| INV-019 | The spec provides an explicit dependency chain for implementation ordering | No | core-standard |
| INV-020 | Every element specification chapter includes a structured verification prompt block | No | core-standard |

---

## §0.6 ADR Registry (Declarations)

All 11 ADRs of the DDIS standard. Full specifications with Problem, Options, Decision, WHY NOT, Consequences, and Tests are in the implementing module.

| ID | Title | Decision (one-line) | Conditional | Implementing Module |
|---|---|---|---|
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

## §0.7 Quality Gates (Declarations)

A DDIS-conforming specification is "done" when all quality gates pass. Gates are ordered by priority; a failing Gate N makes Gates N+1 through 7 irrelevant. *(Full definitions: core-standard §0.7.)*

| Gate | Name | Validates | Check Type |
|------|------|-----------|------------|
| 1 | Structural Conformance | §0.3 completeness, INV-020 | Mechanical |
| 2 | Causal Chain Integrity | INV-001 | Sampling (5 sections) |
| 3 | Decision Coverage | INV-002 | Adversarial review |
| 4 | Invariant Falsifiability | INV-003 | Constructive |
| 5 | Cross-Reference Web | INV-006 | Graph analysis |
| 6 | Implementation Readiness | Spec sufficiency | Expert review |
| 7 | LLM Implementation Readiness | INV-017, INV-018, INV-019 | LLM test (≥ 2 chapters) |

### Modularization Quality Gates [Conditional — modular specs only]

In addition to Gates 1–7. A failing Gate M-1 makes Gates M-2 through M-5 irrelevant. *(Full definitions: modularization module §0.13.11–§0.13.13.)*

| Gate | Name | Validates | Check Type |
|------|------|-----------|------------|
| M-1 | Consistency Checks | INV-012, INV-013, INV-014, INV-016 | Mechanical (CHECK-1–CHECK-9) |
| M-2 | Bundle Budget Compliance | INV-014 | Mechanical |
| M-3 | LLM Bundle Sufficiency | INV-011 | LLM test (≥ 2 modules) |
| M-4 | Declaration-Definition Faithfulness | INV-015 | Comparison |
| M-5 | Cascade Simulation | INV-016 | Simulation |

*(Definition of Done: see core-standard module §0.7.)*

---

## 0.8 Performance Budgets (Declarations)

Specifications have performance characteristics. *(Full definitions with tables: core-standard §0.8.)*

**Key parameters:** Small spec 500–1,500 lines, Medium 1,500–5,000, Large 5,000–15,000. Domain-spec proportions: PART II 35–45% (the heart). Meta-standard proportions: PART 0 45–60%, PART II 20–30%. All proportions adjustable by ±20%.

DO NOT apply domain-spec proportions to a meta-standard. See Chapter 9 (guidance-operations module) for diagnostic signals.

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

1. ~~Machine-readable cross-references~~ **RESOLVED**: Three parseable token formats (`§X.Y`, `INV-NNN`, `ADR-NNN`); regex-extractable. See §10.1.

2. **Multi-document specs**: How should sub-specs reference each other and what invariants apply across spec boundaries? Partially addressed by §0.13.

3. ~~Spec evolution~~ **RESOLVED**: ADR-011 defines mark-and-supersede with cross-reference cascade (§13.3).

4. **Formal verification bridge**: Should DDIS define a pathway from falsifiable invariants to machine-checked properties for safety-critical systems?

5. ~~Confidence levels~~ **RESOLVED**: ADR format (§3.5) includes optional Confidence field (Committed / Provisional / Speculative).

6. **Composability across specs**: **DEFERRED** — requires >= 2 interdependent DDIS specs to validate. Candidate: `[SpecName]:INV-NNN` / `[SpecName]:§X.Y` with shared invariant registry.

---

## Glossary (Compact)

Core DDIS terms for quick orientation. *(Full glossary: Appendix A, guidance-operations module. If wording diverges, Appendix A is authoritative per INV-015.)*

| Term | Definition | See Module |
|---|---|
| **ADR** | Architecture Decision Record: structured choice with alternatives and rationale (§3.5) | core-standard |
| **Bundle** | Assembled document for LLM implementation: constitution + module (§0.13.10) | modularization |
| **Cascade protocol** | Procedure for re-validating modules affected by constitutional changes (§0.13.12) | modularization |
| **Causal chain** | Traceable path from first principle through invariant/ADR to implementation (§0.2.3, INV-001) | core-standard |
| **Constitution** | Cross-cutting material constraining all modules, organized in tiers (§0.13.3) | modularization |
| **Cross-reference** | Explicit link using §X.Y, INV-NNN, or ADR-NNN identifiers (INV-006) | guidance-operations |
| **DDIS** | Decision-Driven Implementation Specification — this standard | system-constitution |
| **Declaration** | Compact summary of invariant/ADR in system constitution (§0.13.4) | modularization |
| **Definition** | Full specification including formal expression, violation scenario, validation (§0.13.4) | modularization |
| **Design point** | Specific scale scenario anchoring performance budgets (§3.7) | element-specifications |
| **Falsifiable** | Can be violated by a concrete scenario and detected by a concrete test (INV-003) | core-standard |
| **Gate** | Quality gate: stop-ship predicate for project progression (§0.7) | core-standard |
| **Hallucination** | LLM failure: plausible but unauthorized behaviors; prevented by negative specs (§0.2.2) | core-standard |
| **Invariant** | Numbered, falsifiable property that must hold at all times (§3.4) | core-standard |
| **LLM** | Large Language Model: primary implementer under §0.2.2 constraints | core-standard |
| **Manifest** | Machine-readable YAML declaring modules, invariant ownership, assembly rules (§0.13.9) | modularization |
| **Meta-instruction** | Directive to LLM implementer: ordering, sequencing, process guidance (§5.7) | element-specifications |
| **Module** | Self-contained spec unit covering one major subsystem (§0.13.2, §0.13.5) | modularization |
| **Negative specification** | Explicit "DO NOT" constraint; primary defense against hallucination (§3.8, INV-017) | element-specifications |
| **Self-bootstrapping** | Property of this standard: written in the format it defines (ADR-004) | core-standard |
| **Verification prompt** | Structured self-check at end of implementation chapter (§5.6, INV-020) | element-specifications |

> **Verification**: Each definition here must faithfully summarize its Appendix A entry (INV-015); update both locations when adding or renaming terms.

---

## Section Map

Cross-reference lookup: which module file contains each section number. The Notes column indicates key cross-module references to help an LLM distinguish self-references from cross-module references.

| Section Range | Module File | Notes |
|---|---|
| §0.1–§0.10, Invariant/ADR/Gate declarations, Glossary | constitution/system.md | Cross-cutting: included in every bundle. §0.11/§0.12 content nested as §0.1.2/§0.1.3 |
| §0.5 (full defs), §0.6 (full ADRs), §0.7 (details), §0.8, §1.1–§1.4 | modules/core-standard.md | References: INV-011–016 from modularization |
| §0.13 (full protocol), INV-011–INV-016, ADR-006–ADR-007 | modules/modularization.md | References: §5.3, §5.5 from element-specifications |
| §2.1–§7.3 | modules/element-specifications.md | References: §1.4 from core-standard |
| §8.1–Part X | modules/guidance-operations.md | References: §3.8, §5.6, §5.7 from element-specifications, §1.1 from core-standard |

---

## Context Budget

> Replicated from `manifest.yaml` for orientation; the manifest is authoritative if values diverge (INV-016).

```
target_lines: 4000
hard_ceiling_lines: 5000
reasoning_reserve: 0.25
```

---

## Module Map

| Module | Domain | Contents |
|---|---|
| **core-standard** | core | INV-001--010, INV-017--020 (full defs); ADR-001--005, ADR-008--011 (full specs); PART I foundations |
| **element-specifications** | core | PART II element specs (Ch 2--5); negative specifications (§3.8), verification prompts (§5.6), meta-instructions (§5.7) |
| **modularization** | modularization | §0.13 protocol; INV-011--016; ADR-006--007; manifest schema, assembly, cascade, migration |
| **guidance-operations** | guidance | PART III guidance, PART IV operations, appendices (glossary, risk register, error taxonomy, quick-ref), PART X master TODO |


---
module: core-standard
domain: core
maintains: [INV-001, INV-002, INV-003, INV-004, INV-005, INV-006, INV-007, INV-008, INV-009, INV-010, INV-017, INV-018, INV-019, INV-020]
interfaces: [INV-011, INV-012, INV-013, INV-014, INV-015, INV-016]
implements: [ADR-001, ADR-002, ADR-003, ADR-004, ADR-005, ADR-008, ADR-009, ADR-010, ADR-011]
adjacent: [element-specifications, modularization, guidance-operations]
negative_specs:
  - "Must NOT define invariants without violation scenarios"
  - "Must NOT create strawman ADRs with obviously inferior options"
  - "Must NOT define quality gates without concrete measurement procedures"
---

# Core Standard Module

The heart of DDIS — full invariant definitions, full ADR specifications, quality gates detail, performance budgets, and PART I foundations.

**Invariants referenced from other modules (INV-018 compliance):**
- INV-011: An LLM receiving a properly assembled bundle can implement the module's subsystem without information from any other module (maintained by modularization module)
- INV-012: Modules reference each other only through constitutional elements (maintained by modularization module)
- INV-013: Every application invariant is maintained by exactly one module (maintained by modularization module)
- INV-014: Every assembled bundle fits within the hard ceiling (maintained by modularization module)
- INV-015: Every invariant declaration is a faithful summary of its full definition (maintained by modularization module)
- INV-016: The manifest accurately reflects the current state of all spec files (maintained by modularization module)

---

## 0.5 Invariants of the DDIS Standard

Every DDIS-conforming specification must satisfy these invariants. Each invariant has an identifier, a plain-language statement, a formal expression, a violation scenario, a validation method, and a WHY THIS MATTERS annotation.

---

**INV-001: Causal Traceability**

*Every implementation section traces to at least one ADR or invariant, which traces to the formal model.*

```
∀ section ∈ PART_II:
  ∃ adr ∈ ADRs ∪ inv ∈ Invariants:
    section.references(adr ∨ inv) ∧ (adr ∨ inv).derives_from(formal_model)
```

Violation scenario: An implementation chapter describes a caching layer with no ADR justifying its existence and no invariant it preserves. Six months later, nobody knows if it can be removed.

Validation: Manual audit. Pick 5 random implementation sections. For each, follow cross-references backward to an ADR or invariant, then to the formal model. If any chain breaks, INV-001 is violated.

// WHY THIS MATTERS: Without traceability, sections accumulate without justification and cannot be safely removed.

---

**INV-002: Decision Completeness**

*Every design choice where a reasonable alternative exists is captured in an ADR.*

```
∀ choice ∈ spec where ∃ alternative ∧ alternative.is_reasonable:
  ∃ adr ∈ ADRs: adr.covers(choice) ∧ adr.alternatives.contains(alternative)
```

Violation scenario: The spec prescribes advisory locking but never records why mandatory locking was rejected. A new team member re-implements with mandatory locks, causing deadlocks.

Validation: Adversarial review. For each implementation section, ask "could this reasonably be done differently?" If yes and no ADR exists, INV-002 is violated.

// WHY THIS MATTERS: Undocumented decisions get relitigated. Each relitigation costs the same as the original decision but adds no value.

---

**INV-003: Invariant Falsifiability**

*Every invariant can be violated by a concrete scenario and detected by a named test.*

```
∀ inv ∈ Invariants:
  ∃ scenario: scenario.violates(inv) ∧
  ∃ test ∈ TestStrategy: test.detects(scenario)
```

Violation scenario: An invariant states "the system shall be performant" — no concrete scenario can violate this because "performant" is undefined.

Validation: For each invariant, construct a counterexample (a state or sequence of events that would violate it). If no such counterexample can be constructed, the invariant is either trivially true (remove it) or too vague (sharpen it).

// WHY THIS MATTERS: Unfalsifiable invariants provide false confidence. They look like safety properties but prevent nothing.

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

Violation scenario: The spec describes a "conflict resolution algorithm" in prose without pseudocode. The LLM invents its own algorithm that handles the happy path but fails on concurrent modifications.

Validation: Mechanical check. Scan each algorithm section for the four required components.

// WHY THIS MATTERS: Prose descriptions of algorithms are ambiguous. LLMs fill ambiguity with plausible but incorrect logic.

---

**INV-005: Performance Verifiability**

*Every performance claim is tied to a specific benchmark scenario, a design point, and a measurement methodology.*

```
∀ perf_claim ∈ spec:
  ∃ benchmark: perf_claim.measured_by(benchmark) ∧
  ∃ design_point: perf_claim.valid_at(design_point) ∧
  benchmark.has(methodology)
```

Violation scenario: The spec claims "sub-millisecond dispatch" without a benchmark, design point, or measurement method. The implementer achieves 0.5ms in testing but 15ms in production on different hardware.

Validation: For each performance number, locate the benchmark that measures it. If the benchmark doesn't exist or doesn't describe how to run it, INV-005 is violated.

// WHY THIS MATTERS: Performance claims without measurement methodology are wishes, not contracts.

---

**INV-006: Cross-Reference Density**

*The specification contains a cross-reference web where no section is an island.*

```
∀ section ∈ spec (excluding Preamble, Glossary):
  section.outgoing_references.count ≥ 1 ∧
  section.incoming_references.count ≥ 1
```

Violation scenario: A "Security Considerations" section is added late. It references nothing and nothing references it. It contains good advice that no implementer reads because it's disconnected from the sections they work in.

Validation: Build a directed graph of cross-references. Every non-trivial section must have at least one inbound and one outbound edge. Orphan sections violate INV-006.

// WHY THIS MATTERS: Cross-references prevent a spec from devolving into independent essays. For LLMs, explicit identifiers (§X.Y, INV-NNN) are the ONLY navigation mechanism — they cannot "flip back" like a human.

---

**INV-007: Signal-to-Noise Ratio**

*Every section earns its place by serving at least one other section or preventing a named failure mode.*

```
∀ section ∈ spec:
  ∃ justification:
    (section.serves(other_section) ∨ section.prevents(named_failure_mode))
```

Violation scenario: The spec includes a 200-line "History of the Project" section that serves no other section and prevents no failure. It consumes context budget without contributing to implementation correctness.

Validation: For each section, state in one sentence why removing it would make the spec worse. If you cannot, remove the section.

// WHY THIS MATTERS: Every line in the spec competes for the reader's attention (human) or context window (LLM). Noise displaces signal.

---

**INV-008: Self-Containment**

*The specification, combined with the implementer's general programming competence and domain knowledge available in public references, is sufficient to build a correct v1.*

```
∀ implementation_question Q:
  spec.answers(Q) ∨
  Q.answerable_from(general_competence ∪ public_references)
```

Violation scenario: The spec references "the standard retry algorithm" without specifying which one. The LLM picks exponential backoff; the use case requires jittered retry.

Validation: Every question a fresh engineer asks that the spec should have answered indicates an INV-008 violation.

// WHY THIS MATTERS: An LLM cannot ask clarifying questions mid-implementation. Every gap becomes a hallucination site.

---

**INV-009: Glossary Coverage**

*Every domain-specific term used in the specification is defined in the glossary.*

```
∀ term ∈ spec where term.is_domain_specific:
  ∃ entry ∈ Glossary: entry.defines(term)
```

Violation scenario: The spec uses "reservation" (meaning advisory file lock) without defining it. The LLM uses the common-English meaning and builds a booking system.

Validation: Extract all non-common-English terms. Check each against the glossary.

// WHY THIS MATTERS: LLMs default to the most common meaning of a word. Domain-specific overloads MUST be defined explicitly.

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

Violation scenario: A task state machine defines states {Pending, InProgress, Done} but omits what happens when "complete" arrives for an already-Done task. The LLM silently accepts the duplicate completion, corrupting downstream state.

Validation: For each state machine, enumerate the state × event cross-product. Every cell must name a transition or explicitly state "invalid — [policy]."

// WHY THIS MATTERS: Incomplete state machines are the most common source of bugs in event-driven systems. LLMs implement only the happy-path transitions unless told otherwise.

---

**INV-017: Negative Specification Coverage**

*Every implementation chapter includes explicit "DO NOT" constraints that prevent the most likely hallucination patterns for that subsystem.*

```
∀ chapter ∈ PART_II_chapters:
  chapter.has(negative_specifications) ∧
  chapter.negative_specifications.count ≥ 3
```

Violation scenario: The scheduler chapter describes how tasks are dispatched but never says "DO NOT use blocking locks." The LLM adds a mutex-based priority system that deadlocks under load.

Validation: For each implementation chapter, verify ≥ 3 negative specifications exist, each addressing a plausible LLM hallucination. Test: "Would an LLM, given only the positive spec, plausibly do this?" If yes and no negative spec prevents it, INV-017 is violated.

// WHY THIS MATTERS: LLMs fill specification gaps with plausible behavior. Negative specifications tell the LLM what NOT to do, preventing hallucination before it occurs. (Locked by ADR-009.)

---

**INV-018: Structural Redundancy at Point of Use**

*Every implementation chapter restates the invariants it must preserve, not merely referencing them by ID number alone.*

```
∀ chapter ∈ PART_II_chapters:
  ∀ inv ∈ chapter.preserved_invariants:
    chapter.contains(inv.id) ∧
    chapter.contains(inv.one_line_statement ∨ inv.full_statement)
```

Violation scenario: An implementation chapter says "Preserves: INV-003, INV-017, INV-018" but never restates what these require. The LLM, 2,000 lines past the definitions, violates INV-017 unknowingly.

Validation: For each implementation chapter, verify preserved invariants are restated (minimum: ID + one-line statement). Bare ID lists violate INV-018.

// WHY THIS MATTERS: An invariant reference 2,000 lines from its definition is functionally invisible to an LLM. Restating at point of use is the structural equivalent of "inline the header."

---

**INV-019: Implementation Ordering Explicitness**

*The spec provides an explicit dependency chain for implementation ordering: which subsystems must be built before which, and why.*

```
∃ ordering ∈ spec:
  ordering.is_dag ∧
  ∀ (a, b) ∈ ordering.edges:
    ∃ reason: a.must_precede(b).because(reason)
```

Violation scenario: The spec describes five subsystems with no ordering guidance. The LLM builds the UI layer first, then discovers it depends on a nonexistent data model. Cascading rework ensues.

Validation: Locate the implementation ordering (operational playbook or meta-instructions). Verify it is a DAG. For each dependency edge, verify the stated reason.

// WHY THIS MATTERS: LLMs implement in whatever order they encounter sections. Explicit ordering prevents cascading rework. (See §5.7 in element-specifications module for meta-instruction format.)

---

**INV-020: Verification Prompt Coverage**

*Every element specification chapter includes a structured verification prompt block that demonstrates §5.6 (element-specifications module) by self-application.*

```
∀ chapter ∈ element_specification_chapters:
  chapter.has(verification_prompt_block) ∧
  chapter.verification_prompt_block.has(positive_check) ∧
  chapter.verification_prompt_block.has(negative_check)
```

Violation scenario: The DDIS standard prescribes verification prompts (§5.6, element-specifications module) but its own element specification chapters lack them. An LLM author reading §5.6 sees the prescription but has no self-bootstrapping demonstration to copy.

Validation: For each element specification chapter (Chapters 2–7), verify a verification prompt block exists with at least one positive and one negative check referencing specific invariants. Bare quality criteria without the §5.6 (element-specifications module) format do not satisfy INV-020.

// WHY THIS MATTERS: Self-bootstrapping (ADR-004) requires the standard to demonstrate every element it prescribes. Without verification prompts in its own element specs, LLM authors lack a concrete model to follow. (Locked by ADR-010.)

---

## 0.6 Architecture Decision Records

### ADR-001: Document Structure Is Fixed, Not Flexible

#### Problem

Should DDIS prescribe a fixed document structure, or allow authors to organize freely as long as content requirements are met?

#### Options

A) **Fixed structure** (prescribed section ordering and hierarchy)
- Pros: Predictable for readers; mechanical completeness checking; LLMs benefit from structural predictability (§0.2.2).
- Cons: May feel rigid; some domains fit better than others.

B) **Content requirements only** (prescribe what, not where)
- Pros: Flexibility; authors organize by whatever axis makes sense.
- Cons: Every spec is unique; readers re-learn structure each time; harder to validate; LLMs perform worse with unpredictable structure.

C) **Fixed skeleton with flexible interior** (prescribed top-level parts, flexible chapters within)
- Pros: Balance of predictability and flexibility.
- Cons: "Flexible interior" often means "no structure at all."

#### Decision

**Option A: Fixed structure.** A reader who has seen one DDIS spec can navigate any other. Worth the cost of occasionally awkward placement. For LLMs, fixed structure reduces output variance (§0.2.2).

Sections may be renamed (e.g., "Kernel Invariants" instead of "Invariants") and domain-specific sections added within any PART, but required elements (§0.3) must appear and PART ordering preserved.

#### Consequences

- Authors must sometimes determine where a domain-specific concept fits; readers and tools gain predictability in return

#### Tests

- (Validated by INV-001, INV-006) If an author places content in an unexpected location, cross-references will either break or become strained, surfacing the misplacement.

---

### ADR-002: Invariants Must Be Falsifiable, Not Merely True

#### Problem

Should invariants be aspirational properties ("the system should be fast") or formal contracts with concrete violation scenarios?

#### Options

A) **Aspirational invariants** (natural language desired properties)
- Pros: Easy to write; captures intent.
- Cons: Cannot be tested, violated, or used for verification.

B) **Formal invariants with proof obligations** (TLA+-style temporal logic)
- Pros: Machine-checkable; mathematically rigorous.
- Cons: Requires formal methods expertise; high authoring cost; most implementers can't read them.

C) **Falsifiable invariants** (formal enough to test, informal enough to read)
- Pros: Each has a concrete counterexample and test; readable by engineers and LLMs.
- Cons: Not machine-checkable; relies on human judgment for completeness.

#### Decision

**Option C: Falsifiable invariants.** Every invariant must include: plain-language statement, semi-formal expression, violation scenario, and validation method.

// WHY NOT Option B? The goal is implementation correctness by humans and LLMs, not machine-checked proofs. If a domain requires machine-checked invariants, the DDIS spec can reference an external formal model.

#### Consequences

- Invariants become immediately actionable as test cases; some subtle properties may be hard to express in this format

#### Tests

- (Validated by INV-003) Every invariant in a DDIS spec must have a constructible counterexample.

---

### ADR-003: Cross-References Are Mandatory, Not Optional Polish

#### Problem

Should cross-references between sections be recommended or required?

#### Options

A) **Recommended** — encourage authors to add cross-references where helpful.
B) **Required** — every non-trivial section must have inbound and outbound references, using explicit identifiers (§X.Y, INV-NNN, ADR-NNN).

#### Decision

**Option B: Required.** Cross-references transform a collection of sections into a unified specification. Without them, the causal chain (INV-001) cannot be verified. For LLMs, explicit identifiers (§X.Y, INV-NNN, ADR-NNN) are the ONLY reliable navigation — implicit references like "see above" fail (§0.2.2).

#### Consequences

- Higher authoring cost per section, offset by graph-based completeness validation

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

**Option B: Self-bootstrapping.** This document is both the standard and its first conforming instance. If the standard is unclear or incomplete, the author discovers this while applying it to itself.

// WHY NOT Option A? A standard that cannot be applied to itself is suspect. Self-application is the ultimate dog-fooding.

#### Consequences

- More trustworthy (tested by self-application) but more complex (meta-level and object-level interleave); readers may initially find the self-referential nature disorienting

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

**Option B: Voice guidance.** Specifications fail when either too dry to read or too casual to trust. DDIS prescribes a specific voice: technically precise but human, a senior engineer explaining to a peer they respect. (See §8.1, guidance-operations module.) For LLMs, explicit voice guidance reduces generic boilerplate.

#### Consequences

- Specs feel more unified; authors must sometimes revise natural writing habits

#### Tests

- Qualitative review: sample 5 sections, assess whether each sounds like a senior engineer talking to a peer. If any sounds like a textbook, marketing copy, or bureaucratic report, the voice is wrong.

---

### ADR-008: LLM Provisions Woven Throughout, Not Isolated

#### Problem

DDIS introduces structural provisions for LLM consumption (negative specifications, verification prompts, meta-instructions, structural redundancy). How should these provisions be integrated into the standard?

#### Options

A) **Isolated chapter** — a "Chapter N: LLM Considerations" appendix.
- Pros: Easy to find; easy to skip for human implementers.
- Cons: Provisions distant from modified elements get forgotten (§0.2.2 implicit reference failure).

B) **Woven throughout** — integrate LLM provisions into each element specification.
- Pros: Guidance at point of use; follows INV-018 (structural redundancy). No separate chapter to forget.
- Cons: Increases element spec length by 10–15%.

C) **Dual: woven plus summary appendix.**
- Pros: Best of both worlds.
- Cons: Divergence risk (INV-015); marginal value since the Quick-Reference Card (Appendix D) already summarizes.

#### Decision

**Option B: Woven throughout.** LLM provisions are integrated into each element specification. The Quick-Reference Card provides the high-level summary. Context at point of use is worth the redundancy cost.

// WHY NOT Option A? It suffers from the exact failure mode DDIS prevents — information distant from point of use gets lost.

// WHY NOT Option C? Maintaining two copies creates divergence risk.

#### Consequences

- Every element specification includes LLM-specific notes and provisions
- Authors encounter LLM considerations naturally while writing each element
- ~10% longer, but every added line is at maximum impact

#### Tests

- (Validated by INV-017, INV-018) Conforming specs have negative specifications and structural redundancy in every implementation chapter.
- Qualitative: An author writing a DDIS spec encounters LLM guidance for every element without consulting a separate chapter.

---

### ADR-009: Negative Specifications as Formal Elements

#### Problem

How should "what the system must NOT do" be captured in a DDIS spec? Anti-patterns in §8.3 (guidance-operations module) partially serve this role, but they are guidance (PART III), not required structural elements.

#### Options

A) **Anti-patterns only** — rely on existing anti-pattern catalog (§8.3, guidance-operations module).
- Pros: No new element required; works well for human readers.
- Cons: Document-level guidance, not subsystem-level requirements. LLMs need co-located constraints — distant lists have minimal effect.

B) **Formal negative specification blocks** — required per implementation chapter with prescribed format.
- Pros: Co-located with the subsystem (maximum LLM impact per §0.2.2). Falsifiable. Machine-verifiable.
- Cons: Adds ~5–10 lines per chapter. Requires adversarial thinking.

C) **Separate negative specification chapter** — one chapter listing all constraints.
- Pros: Easy to audit for completeness.
- Cons: Same distance-from-use problem as Option A.

#### Decision

**Option B: Formal negative specification blocks in each implementation chapter.** Required structural elements (INV-017), specified in §3.8 (element-specifications module), demonstrated throughout this document.

// WHY NOT Option A? LLMs need imperative, co-located constraints — not illustrative examples in a distant section.

// WHY NOT Option C? Same distance-from-use problem. The LLM implementing the scheduler won't have the chapter in context.

#### Consequences

- Every implementation chapter gains 3–8 negative specifications
- Authors think adversarially: "What would an LLM plausibly do wrong here?"
- Anti-pattern catalog (§8.3, guidance-operations module) remains as document-level guidance; negative specs are subsystem-level requirements

#### Tests

- (Validated by INV-017) Every implementation chapter has ≥ 3 negative specifications.
- LLM test: Give an implementation chapter without negative specs to an LLM; note hallucinations. Add negative specs; re-test. Hallucination rate should decrease measurably.

---

### ADR-010: Verification Prompts per Implementation Chapter

#### Problem

How should implementers verify that their work conforms to the spec? Test strategies (§6.2, element-specifications module) define what to test, but they operate post-implementation. Is there value in pre-/mid-implementation self-checks?

#### Options

A) **Test strategies only** — catch conformance issues post-implementation.
- Pros: No new element required; well-established.
- Cons: Post-implementation only. For LLMs, a self-check during generation is cheaper than rewriting after.

B) **Verification prompts per chapter** — structured self-check at the end of each implementation chapter.
- Pros: Catches misunderstandings before code is written. LLMs execute as part of implementation flow. Humans use as review checklists.
- Cons: Adds ~5–8 lines per chapter.

C) **Single end-of-document verification checklist.**
- Pros: Easy to find; comprehensive.
- Cons: Too distant and generic for subsystem-specific issues.

#### Decision

**Option B: Verification prompts per chapter.** Each chapter ends with positive checks ("DOES...") and negative checks ("does NOT..."), referencing specific invariants.

// WHY NOT Option A? Test strategies catch implementation bugs; verification prompts catch specification misunderstandings. Different failure modes, different workflow points.

// WHY NOT Option C? Same distance-from-use problem. Generic checklists miss subsystem-specific concerns.

#### Consequences

- Each chapter gains ~5–8 lines of verification prompts
- LLMs use as structured self-checks; humans use as PR review checklists
- Self-bootstrapping: this document includes verification prompts for its own elements

#### Tests

- (Validated by Gate 7) LLM implementation test includes executing verification prompts and confirming they catch common errors.
- Qualitative: An implementer finds verification prompts useful as a "did I miss anything?" check.

---

### ADR-011: ADR Supersession Protocol

#### Problem

When a Living spec (§1.1, §13.1 in guidance-operations module) supersedes an ADR, sections referencing the old decision may prescribe behavior incompatible with the new one. Without a formal protocol, LLMs encounter conflicting guidance.

#### Options

A) **Delete-and-replace** — Remove old ADR, reuse the same identifier.
- Pros: Clean; implementers see only current decisions.
- Cons: Loses reasoning history; future maintainers cannot understand why the decision was reversed.

B) **Mark-and-supersede with cross-reference cascade** — Retain old ADR as superseded record, create new ADR, cascade-update references.
- Pros: Preserves history; prevents re-exploration of dead ends; cascade ensures consistency.
- Cons: Requires cascade procedure; slightly increases document length.

C) **Versioned ADRs** — Same identifier with version suffixes (ADR-003v1, ADR-003v2).
- Pros: Easy to track evolution.
- Cons: Breaks cross-reference stability — "ADR-003" becomes ambiguous. LLMs cannot resolve version suffixes reliably.

#### Decision

**Option B: Mark-and-supersede with cross-reference cascade.** When an ADR is superseded:

1. Mark the original ADR with: `**Status: SUPERSEDED by ADR-NNN** (date)`
2. Create the new ADR with a fresh identifier, referencing the old ADR: `Supersedes: ADR-NNN`
3. The new ADR's "Options" section MUST include the old decision as a rejected option with a WHY NOT annotation explaining what changed
4. Execute a cross-reference cascade: every section referencing the old ADR-NNN must be updated to reference the new ADR-NNN (see §13.3 in guidance-operations module for the cascade procedure)

// WHY NOT Option A? Deleting ADRs destroys institutional knowledge — the reasoning prevents re-exploring dead ends.

// WHY NOT Option C? Version suffixes break cross-reference stability (INV-006). "ADR-003" becomes ambiguous without additional context.

#### Consequences

- Every supersession triggers a cross-reference cascade (§13.3, guidance-operations module)
- Superseded ADRs remain as historical record
- Spec length grows slightly with each supersession

#### Tests

- (Validated by INV-001) After supersession, trace 3 sections that referenced the old ADR. All must now reference the new ADR with an intact causal chain.
- (Validated by INV-006) The old ADR has at least one inbound reference (the new ADR's "Supersedes" link) — it is not orphaned.

---

## 0.7 Quality Gates

A DDIS-conforming specification is "done" when all quality gates pass. Gates are ordered by priority; a failing Gate N makes Gates N+1 through 7 irrelevant.

**Gate 1: Structural Conformance**
All required elements from §0.3 present, including negative specifications (INV-017; format in §3.8, element-specifications module), verification prompts (INV-020; format in §5.6, element-specifications module), and meta-instructions (INV-019; format in §5.7, element-specifications module). Every element spec chapter includes a verification prompt block (INV-020). Mechanical check.

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
Give ONLY one implementation chapter (plus glossary and relevant invariants) to an LLM. Verify: (a) no hallucinated requirements, (b) no clarifying questions about architecture, (c) all chapter-header invariants preserved, (d) all negative specifications observed. Test on ≥ 2 representative chapters. (Validates INV-017, INV-018, INV-019.)

> **Gate 7 demonstrations (thought experiments):** *(All section references below are in element-specifications module.)*
> - **§3.4 (Invariants)**: Five-part format required; no aspirational invariants or missing violation scenarios.
> - **§4.2 (State Machines)**: Complete state x event table with guards and invalid transition policy; no happy-path-only transitions.
> - **§5.1 (Implementation Chapters)**: All 13 components present; invariants restated (INV-018), negative specs included (INV-017).
> - **§6.1 (Operational Playbook)**: Decision spikes with time budgets; deliverable order as DAG; no aspirational exit criteria.

**Gate 7 test protocol:**

1. **Select** 2 representative implementation chapters (one high-complexity, one moderate).
2. **Assemble** test input per chapter: chapter text + glossary + referenced invariants. No other sections.
3. **Prompt** the LLM: "Implement this subsystem based on the following specification. Do not add features not described in the spec."
4. **Score** the output on four axes: (a) hallucinated requirements, (b) negative spec violations, (c) missing invariant preservation, (d) architectural clarifying questions.
5. **Pass criteria**: (a) = 0, (b) = 0, (c) all chapter-header invariants addressed, (d) ≤ 1 per chapter. (See SPEC-BENCH-002 in §0.8.4.)
6. **Failure remediation**: missing negative spec → add per INV-017 (format in §3.8, element-specifications module); missing restatement → fix per INV-018; ambiguity → clarify in chapter.

### Definition of Done (for this standard)

- This document passes Gates 1–7 applied to itself
- At least one non-trivial spec has been written conforming to DDIS without structural workarounds
- The Glossary (Appendix A) covers all DDIS-specific terminology
- LLM provisions are demonstrated in this document's own element specifications (self-bootstrapping)

## 0.8 Performance Budgets (for Specifications, Not Software)

Specifications have performance characteristics too. A 40-hour spec is too long. A 2-hour spec probably omits critical details.

### 0.8.1 Specification Size Budgets

| System Complexity | Target Spec Length | Rationale |
|---|---|
| Small (single crate, < 5K LOC target) | 500–1,500 lines | Enough for formal model + invariants + key ADRs |
| Medium (multi-crate, 5K–50K LOC target) | 1,500–5,000 lines | Full DDIS treatment |
| Large (multi-service, > 50K LOC target) | 5,000–15,000 lines | May split into sub-specs linked by a master |

### 0.8.2 Proportional Weight Guide

Not all sections are equal. These proportions prevent bloat and starvation. Domain-specific specs may adjust by ±20%.

**For domain specifications:**

| Section | % of Total | Why |
|---|---|
| Preamble + PART 0 | 15–20% | Dense: formal model, invariants, ADRs, quality gates |
| PART I: Foundations | 8–12% | First principles, state machines, complexity analysis |
| PART II: Core Implementation | 35–45% | THE HEART: algorithms, data structures, protocols, examples, negative specs, verification prompts |
| PART III: Interfaces | 8–12% | API schemas, adapters, external contracts |
| PART IV: Operations | 10–15% | Testing, operational playbook, roadmap |
| Appendices + Part X | 10–15% | Reference material, glossary, error taxonomy, master TODO |

**For meta-standards (self-bootstrapping specs about specification authoring):**

A meta-standard's "implementation" IS its own definitions — invariants, ADRs, quality gates, and element specifications all reside in PART 0 and PART II. The domain-spec proportions do not apply directly because there are no external algorithms or protocols to describe. Meta-standards use these adjusted proportions:

| Section | % of Total | Why |
|---|---|
| Preamble + PART 0 | 45–60% | Contains the entire standard definition: invariants, ADRs, gates, modularization protocol |
| PART I: Foundations | 3–6% | Formal model of specifications as artifacts — concise by nature |
| PART II: Element Specifications | 20–30% | Templates and guidance for each structural element |
| PART III: Guidance | 4–8% | Voice, style, and cross-reference patterns |
| PART IV: Operations | 3–6% | Authoring sequence, validation, evolution |
| Appendices + Part X | 6–12% | Reference material, glossary, error taxonomy, master TODO |

DO NOT apply domain-spec proportions to a meta-standard and flag a violation. The causal chain is: meta-standards define the structure that domain specs follow, so their weight distribution reflects authoring (definitions, invariants, ADRs) rather than implementation (algorithms, protocols). See Chapter 9 (guidance-operations module) for diagnostic signals of imbalanced weight and guidance on identifying the spec's "heart."

**Self-application — Proportional weight verification** (ADR-004): The modular form (3,811 lines) distributes PART 0 across the constitution and two modules. Aggregate proportions: Preamble + PART 0 = 59.6% (target 45–60%), PART II = 24.1% (target 20–30%), PART III + IV + Appendices = 16.3% (target 13–26% combined) — all within the ±20% adjustment band. The "heart" of this meta-standard (invariant definitions, ADRs, element specification templates) correctly dominates at ~84%.

### 0.8.3 Authoring Time Budgets

| Element | Expected Authoring Time |
|---|---|
| First-principles model | 2–4 hours |
| One invariant (high quality) | 15–30 minutes |
| One ADR (high quality) | 30–60 minutes |
| One implementation chapter | 2–4 hours |
| Negative specs per chapter | 15–30 minutes |
| Verification prompt per chapter | 10–15 minutes |
| End-to-end trace | 1–2 hours |
| Glossary | 1–2 hours |

### 0.8.4 Specification Quality Measurement

**Design point**: A 1,500–5,000 line spec (per §0.8.1) consumed by an experienced engineer or current-generation LLM (≥ 100K context). Specs exceeding 5,000 lines should apply these per-module after modularization (§0.13).

Measure these metrics during implementation to validate the performance budgets above:

| ID | Metric | Measurement Method | Target |
|---|---|---|
| SPEC-BENCH-001 | Time to first implementer question | Start timer when implementer begins reading the spec; stop at the first question that the spec should have answered but didn't. Exclude questions about micro-decisions the spec intentionally defers. | > 2 hours |
| SPEC-BENCH-002 | LLM hallucination rate | Give 2 representative implementation chapters (plus glossary and relevant invariants) to an LLM. Count unauthorized behaviors (actions contradicting spec or not derivable from spec) ÷ total architectural decisions in the output. Repeat with and without negative specifications. | < 5% with negative specs; > 15% without (validates INV-017) |
| SPEC-BENCH-003 | Cross-reference resolution time | Pick 10 random cross-references (§X.Y, INV-NNN, ADR-NNN). Time how long it takes to locate each target. | < 30 seconds per reference |
| SPEC-BENCH-004 | Gate passage rate | Run all quality gates (§0.7) against the spec. Record pass/fail per gate on first attempt. | > 80% of gates pass on first attempt |

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
  Validated   — External implementer confirms readiness (Gate 6 + Gate 7)
  Living      — In use, being updated as implementation reveals gaps

Transitions:
  Skeleton  →[author fills sections]→     Drafted
    Guard: All required sections from §0.3 have content (not just placeholders).
    Entry action: Author has completed authoring sequence steps 1–6 (§11.1, guidance-operations module).

  Drafted   →[author adds cross-refs]→    Threaded
    Guard: Every section has at least one outbound reference candidate.
    Entry action: Reference graph constructed; orphan sections identified.

  Threaded  →[gates 1-5 pass]→            Gated
    Guard: All mechanical gates (1–5) pass. Gate failures documented.
    Entry action: Gate status recorded in Master TODO.

  Gated     →[gates 6-7 pass]→            Validated
    Guard: Human implementer AND LLM implementer confirm readiness.
    Entry action: Validation results recorded.

  Validated →[implementation begins]→     Living
    Guard: At least one implementation team has started work.
    Entry action: Spec marked as "living" with change tracking enabled.

  Living    →[gap discovered]→            Drafted (partial regression)
    Guard: Gap is architectural (not micro-level). Documented in spec.
    Entry action: Affected sections marked for re-validation.

Invalid transitions (policy for each):
  Skeleton → Gated          — REJECT: Cannot pass gates with empty sections.
  Skeleton → Threaded       — REJECT: Cannot add cross-references to empty sections.
  Drafted → Validated       — REJECT: Cannot validate without cross-references (Gate 5).
  Drafted → Gated           — REJECT: Must thread cross-references first.
  Threaded → Validated      — REJECT: Must pass mechanical gates first.
  Gated → Living            — REJECT: Must validate with external implementer first.
  Any → Skeleton            — REJECT: Cannot un-write sections (use version control).
```

**State × Event Table** (INV-010 compliance — no empty cells):

| State \ Event | Fill sections | Add cross-refs | Gates 1–5 pass | Gates 6–7 pass | Implementation begins | Gap discovered |
|---|---|---|---|---|---|
| **Skeleton** | → Drafted | REJECT: empty sections | REJECT: empty sections | REJECT: empty sections | REJECT: not validated | REJECT: nothing to gap |
| **Drafted** | No transition (already filled) | → Threaded | REJECT: not threaded | REJECT: not threaded | REJECT: not validated | → Drafted (re-draft) |
| **Threaded** | → Drafted (partial regression) | No transition (already threaded) | → Gated | REJECT: gates 1–5 first | REJECT: not validated | → Drafted (partial regression) |
| **Gated** | → Drafted (partial regression) | No transition | No transition (already gated) | → Validated | REJECT: not validated | → Drafted (partial regression) |
| **Validated** | → Drafted (partial regression) | No transition | No transition | No transition (already validated) | → Living | → Drafted (partial regression) |
| **Living** | No transition | No transition | No transition | No transition | No transition (already living) | → Drafted (partial regression) |

Every cell is explicit: "REJECT" names the rejection policy, "No transition" marks idempotent/inapplicable events, and "partial regression" returns to Drafted for re-validation.

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

**Self-application — Safety verification**: Test case: §3.8 prescribes "3–8 negative specs per subsystem" while §5.6 prescribes "at least one negative check per verification prompt." These are complementary (§3.8 defines constraints, §5.6 verifies them), not contradictory. A contradiction would exist only if §3.8 made DO NOT constraints optional while INV-017 required them.

**Self-application — Liveness verification**: Question: "How should I handle LLM hallucination of unauthorized features?" Answer chain: §0.2.2 identifies the problem, §3.8 provides the mechanism, INV-017 requires >=3 per chapter, §5.6 verifies compliance, Gate 7 tests output. Five cross-referencing sections reach a Validated answer.

### 1.3 Complexity of Specification Elements

| Element | Authoring Complexity | Reading Complexity | Verification Complexity |
|---|---|---|
| Invariant | O(domain_understanding) | O(1) per invariant | O(1) per invariant (construct counterexample) |
| ADR | O(alternatives × analysis_depth) | O(alternatives) per ADR | O(1) per ADR (check that alternatives are genuine) |
| Algorithm | O(algorithm_complexity × edge_cases) | O(pseudocode_length) | O(worked_examples) |
| Cross-reference | O(1) per reference | O(1) per reference | O(sections²) for full graph analysis |
| End-to-end trace | O(subsystems × interactions) | O(subsystems) | O(1) (follow the trace) |
| Negative specification | O(adversarial_thinking) | O(1) per constraint | O(1) per constraint (check implementation) |
| Verification prompt | O(invariants_per_chapter) | O(1) per prompt | O(1) (execute the prompt) |

**Meta-standard operation complexity** (design point: 3,000-line domain spec, 15 invariants, 8 ADRs, 6 implementation chapters):

| Operation | Complexity | Design Point Estimate |
|---|---|
| Authoring a complete spec | O(sections × cross_refs) | ~120 section × ~4 refs = ~480 threading decisions |
| Validating Gates 1–5 | O(sections²) for cross-ref graph + O(invariants) for falsifiability | ~120² / 2 = ~7,200 edge checks + 15 counterexamples |
| Running Gate 7 (LLM readiness) | O(chapters × LLM_calls) | 6 chapters × 1 LLM call each = 6 eval sessions |
| Cross-reference cascade (§0.13.12) | O(affected_invariants × dependent_modules) | ~3 invariants × ~2 modules = ~6 cascade steps |
| Modularization decision (§0.13.7) | O(1) — single flowchart evaluation | 1 check against 2,500-line threshold |

Gate 5's O(sections²) cost dominates; automated cross-reference tooling is essential for specs exceeding 5,000 lines.

### 1.4 End-to-End Trace: Authoring an ADR Through the DDIS Process

This trace follows ADR-002 (Invariants Must Be Falsifiable) from initial recognition through full DDIS authoring to validation. It exercises the formal model (§0.2), non-negotiables (§0.1.2), invariants (§0.5), ADRs (§0.6), element specs (§3.4, §3.5 in element-specifications module), quality gates (§0.7), validation (Chapter 12, guidance-operations module), and self-bootstrapping (ADR-004).

**Step 1: Recognition (§0.2.1)**
Defining what a specification IS, the author recognizes that "verifiability over trust" (consequence 3) requires every claim to be testable. This raises a decision: what level of formality should invariants have?

**Step 2: Non-Negotiable Check (§0.1.2)**
"Invariants are falsifiable" establishes the commitment. Three reasonable alternatives exist (aspirational, formal proof, falsifiable-but-readable), so this requires an ADR.

**Step 3: ADR Creation (§3.5, element-specifications module)**
The author writes ADR-002 per §3.5 format:
- **Problem**: Aspirational, formally proven, or falsifiable?
- **Options**: Three genuine alternatives with concrete pros/cons.
- **Decision**: Option C (falsifiable) with WHY NOT annotations.
- **Tests**: "Validated by INV-003" — forward reference to the enforcing invariant.

**Step 4: Invariant Derivation (§3.4, element-specifications module)**
ADR-002 motivates INV-003 (Invariant Falsifiability). The author writes per §3.4 format: statement, formal expression, violation scenario, validation, WHY THIS MATTERS. The violation scenario ("the system shall be performant") demonstrates concretely how INV-003 would be violated.

**Step 5: Cross-Reference Threading (Chapter 10, guidance-operations module)**
The author threads cross-references: ADR-002 references INV-003, INV-003 references Gate 4, Gate 4 references back to INV-003, the element spec §3.4 references both ADR-002 and INV-003, and the anti-pattern in §3.4 demonstrates what an INV-003 violation looks like.

**Step 6: Quality Gate Validation (§0.7)**
- **Gate 2 (Causal Chain)**: ADR-002 traces to §0.2.1 consequence 3, which traces to the formal model. Chain intact.
- **Gate 3 (Decision Coverage)**: ADR-002 covers the falsifiability choice with three genuine alternatives.
- **Gate 4 (Invariant Falsifiability)**: INV-003 has a constructible counterexample and a validation method.
- **Gate 5 (Cross-Reference Web)**: ADR-002, INV-003, Gate 4, and §3.4 form a connected subgraph.
- **Gate 7 (LLM Implementation Readiness)**: Give §3.4 to an LLM; it should produce invariants with violation scenarios (not aspirational statements), demonstrating that ADR-002's decision propagates to LLM output.

**Step 7: Self-Bootstrap Verification (ADR-004)**
ADR-002 is applied to DDIS's own invariants. For example, INV-001 (Causal Traceability): Can we construct a violation? Yes — a section that references no ADR or invariant. Can we test it? Yes — the manual audit procedure. INV-001 passes INV-003.

This trace demonstrates that a single ADR touches 7 structural elements across 5 chapters, validating both the causal chain (INV-001) and the cross-reference web (INV-006).

---
module: element-specifications
domain: core
maintains: [] # Invariant definitions owned by core-standard; this module defines the FORMATS by which INV-017, INV-018, INV-019, INV-020 are satisfied
interfaces: [INV-001, INV-002, INV-003, INV-004, INV-005, INV-006, INV-007, INV-008, INV-009, INV-010, INV-017, INV-018, INV-019, INV-020]
implements: [ADR-008, ADR-009, ADR-010]
adjacent: [core-standard, guidance-operations]
negative_specs:
  - "Must NOT state design goals in terms of implementation technology"
  - "Must NOT write generic negative specs that apply to all subsystems"
  - "Must NOT write generic verification prompts without subsystem-specific checks"
---

# PART II: CORE STANDARD — ELEMENT SPECIFICATIONS

**Key invariants governing element specifications (INV-018 compliance):**
- INV-017: Every implementation chapter includes ≥3 explicit DO NOT constraints preventing likely hallucination patterns (maintained by core-standard module)
- INV-018: Every implementation chapter restates the invariants it must preserve, not merely referencing them by ID (maintained by core-standard module)
- INV-020: Every element specification chapter includes a structured verification prompt block (maintained by core-standard module)
- INV-004: Every described algorithm includes pseudocode, complexity analysis, worked example, and edge cases (maintained by core-standard module)

(Full definitions for all 20 invariants: see core-standard module §0.5)

The heart of DDIS. Each section specifies one structural element: what it must contain, quality criteria, how it relates to other elements, and what good versus bad looks like. Each includes woven LLM-specific provisions (ADR-008).

## Chapter 2: Preamble Elements

### 2.1 Design Goal

**What it is**: A single sentence (≤ 30 words) that states the system's reason for existing.

**Required properties**:
- States the core value proposition, not the implementation
- Uses bold for emphasis on the 3–5 key properties
- Readable by a non-technical stakeholder

**Quality criteria**: A reader who sees only the design goal should be able to decide whether this system is relevant to them.

**DO NOT** state the design goal in terms of implementation technology ("Build a Rust-based event-sourced system") — state it in terms of value.

**DO NOT** exceed 30 words — LLMs treat longer design goals as implementation requirements. (Validates INV-007.)

**DO NOT** use unmeasurable qualities ("robust", "scalable", "enterprise-grade") — LLMs generate boilerplate from abstract adjectives.

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

**DO NOT** use abstract qualities without concrete meaning ("robust", "scalable", "enterprise-grade").

**DO NOT** promise implementation details ("uses React", "built on PostgreSQL") — technical choices belong in ADRs. (Validates INV-002.)

**DO NOT** omit "without" clauses — stating only what the system does leaves the most important constraints implicit. (Validates INV-017.)

**Anti-pattern**: "The system provides robust, scalable, enterprise-grade coordination." ← Meaningless buzzwords.

**Good example** (FrankenTUI): "ftui is designed so you can build a Claude Code / Codex-class agent harness UI without flicker, without cursor corruption, and without sacrificing native scrollback."

---

### 2.3 Document Note

**What it is**: A short disclaimer (2–4 sentences) about code blocks and where correctness lives.

**Why it exists**: Without this note, implementers treat code blocks as copy-paste targets. The document note redirects trust from code to invariants and tests. LLMs will reproduce code blocks verbatim unless explicitly told otherwise.

**DO NOT** omit this note — LLMs reproduce code blocks verbatim unless told otherwise. (Validates INV-017.)

**DO NOT** let the document note exceed 4 sentences — longer embeds guidance that belongs in §2.4. (Validates INV-007.)

**DO NOT** omit mention of invariants and tests as the source of truth. (Validates INV-008.)

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
- For LLM implementers: includes a step about reading negative specifications and verification prompts

**Quality criteria**: A new team member reading only this section knows exactly how to engage with the document. The Public API Surface (§0.9, system constitution) lists the complete interface DDIS provides to specification authors.

**DO NOT** list more than 8 steps — process details belong in PART IV. (Validates INV-007.)

**DO NOT** omit the LLM-specific reading step for negative specifications and verification prompts — LLMs skip constraints and implement from positive descriptions only. (Validates INV-020.)

**DO NOT** use vague steps like "Understand the architecture" — every step must name a specific section or action. (Validates INV-017.)

### Verification Prompt for Chapter 2 (Preamble Elements)

After writing your spec's preamble, verify:
1. [ ] Design goal is ≤ 30 words and states value, not technology (INV-017)
2. [ ] Core promise uses "without" clauses and contains no abstract buzzwords (INV-017)
3. [ ] Document note states code blocks are design sketches, not copy-paste targets (INV-008)
4. [ ] How-to-use list starts with "Read PART 0" and includes LLM-specific step for negative specs/verification prompts
5. [ ] Preamble does NOT use marketing language ("enterprise-grade", "cutting-edge")
6. [ ] *Integration*: Design goal properties each map to at least one invariant and one quality gate (INV-001)

---

## Chapter 3: PART 0 Elements

### 3.1 Non-Negotiables (Engineering Contract)

**What it is**: 5–10 properties defining what the system IS. Stronger than invariants (which are formal and testable) — these are philosophical commitments that must never be compromised, even under pressure.

**Required format**:
```
- **[Property name in bold]**
  [One sentence explaining what this means concretely]
```

**Quality criteria for each non-negotiable**:
- An implementer could imagine a situation where violating it would be tempting (e.g., "just skip replay validation in dev mode — it's slow")
- The non-negotiable clearly says: no, even then
- It is not a restatement of a technical invariant; it is a commitment

**DO NOT** restate invariants as non-negotiables — non-negotiables are philosophical commitments; invariants are testable properties. (Validates INV-017.)

**DO NOT** list more than 10 — more than 10 means some are actually preferences, diluting the ones that matter. (Validates INV-007.)

**DO NOT** write non-negotiables that no reasonable person would violate ("the system must not corrupt data") — constrain tempting shortcuts, not universal ethics. (Validates INV-017.)

**Relationship to invariants**: Non-negotiables are the "why" that justifies groups of invariants. "Deterministic replay is real" (non-negotiable) justifies INV-003: "Same event log → identical state" (invariant). The non-negotiable is the commitment; the invariant is the testable manifestation.

---

### 3.2 Non-Goals

**What it is**: A list of 5–10 things the system explicitly does NOT attempt.

**Why it exists**: Scope creep is the most common spec failure. Non-goals give implementers permission to say "out of scope." For LLMs, non-goals prevent adding "helpful" features not in the spec.

**Quality criteria for each non-goal**:
- Someone has actually asked for this (or will), making the exclusion non-obvious
- The non-goal explains briefly why it's excluded (not just "not in scope" but why not)

**DO NOT** list absurd non-goals — every non-goal should exclude something a reasonable stakeholder has or will request. (Validates INV-007.)

**DO NOT** frame non-goals as deferred features ("not yet," "in a future version") — LLMs scaffold placeholder infrastructure for deferred items. A non-goal is a permanent scope boundary. (Validates INV-017.)

**DO NOT** include non-goals that duplicate non-negotiables from §3.1 — non-goals exclude scope; non-negotiables define identity. (Validates INV-007.)

**Anti-pattern**: "Non-goal: Building a quantum computer." ← Nobody asked for this. Non-goals should exclude things that are tempting, not absurd.

---

### 3.3 First-Principles Derivation

**What it is**: The formal model from which the entire architecture derives. Makes every section feel *inevitable* rather than *asserted*.

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

**DO NOT** assert the architecture without deriving it from the formal model — LLMs given asserted architectures make downstream decisions that violate the model. (Validates INV-001.)

**DO NOT** present the formal model in a specific programming language — language-specific syntax constrains LLMs to language-shaped solutions. Use mathematical notation, pseudocode, or language-neutral set theory. (Validates INV-008.)

**DO NOT** skip the Consequences section — LLMs treat the formal model as decoration without it. Consequences bridge model to architecture. (Validates INV-001.)

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

**DO NOT** write invariants without violation scenarios (violates INV-003). **DO NOT** restate type system guarantees as invariants (e.g., "TaskId values are unique" when using newtype with auto-increment). **DO NOT** write aspirational invariants without measurable criteria. (Validates INV-003.)

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

#### Confidence [Optional]
**[Committed|Provisional|Speculative]**: [Brief justification or spike reference]
```

**Confidence levels** (optional, recommended for early-stage specs):
- **Committed**: High confidence, well-analyzed, unlikely to change. Default if omitted.
- **Provisional**: Medium confidence, will be revisited after a decision spike (§6.1.1). Implementation should minimize coupling to this decision.
- **Speculative**: Low confidence, research needed. Implementation should use an abstraction boundary so the decision can be swapped without cascading rework.

**DO NOT** mark all ADRs as Committed in an early-stage spec — if none are Provisional, you are likely over-committing. (Validates INV-002.)

**DO NOT** use Speculative confidence to defer decisions indefinitely — every Speculative ADR must reference a decision spike in §6.1.1 with a time budget and exit criterion. (Validates INV-019.)

**Quality criteria for each ADR**:
- **Genuine alternatives**: Each option must have a real advocate. If Option B is a strawman nobody would choose, it is not a genuine alternative. The test: would a competent engineer in a different context reasonably choose Option B?
- **Concrete tradeoffs**: Pros and cons cite specific, measurable properties — not vague qualities like "simpler" or "more robust."
- **Consequential decision**: The choice materially affects the system. If swapping Option A for Option B would require < 1 day of refactoring, it's not an ADR — it's a local implementation choice.

**DO NOT** include decisions that predate the spec's scope (e.g., language choice if already decided). **DO NOT** create strawman ADRs where one option is obviously superior. **DO NOT** omit WHY NOT annotations for rejected options — LLMs re-explore rejected paths without them. (Validates INV-002.)

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

**Churn-magnets**: After all ADRs are written, add a brief section identifying which decisions cause the most downstream rework if changed. These are the decisions to lock first and spike earliest (see §6.1.1, Phase -1).

---

### 3.6 Quality Gates

**What it is**: 4–8 stop-ship criteria, ordered by priority.

**Required properties per gate**:
- A gate is a **predicate**, not a task. It is either passing or failing at any point in time.
- Each gate references specific invariants or test suites.
- Gates are ordered such that a failing Gate N makes Gate N+1 irrelevant.
- At least one gate specifically validates LLM implementation readiness (see Gate 7 in §0.7).

**Quality criteria**: A project manager should be able to assess gate status in < 30 minutes using the referenced tests. Proportional weight violations signal spec imbalance that may cause gate failures (see §9.2, guidance-operations module).

**DO NOT** define gates without concrete measurement procedures — "Code quality is high" is not a gate; "All invariants have passing tests" is. (Validates INV-003.)

**DO NOT** define overlapping gates — if two gates fail for the same root cause, merge them. (Validates INV-007.)

**DO NOT** omit the gate ordering rationale — gates must be ordered so failing Gate N makes Gate N+1 irrelevant. (Validates INV-019.)

---

### 3.7 Performance Budgets and Design Point

**What it is**: A table of performance targets anchored to a specific design point (hardware, workload, scale).

**Required components**:

1. **Design point**: The specific scenario these budgets apply to. E.g., "M1 Max, 300 concurrent agents, 10K tasks, 60Hz TUI refresh."

2. **Budget table**: Operation → target → measurement method.

3. **Measurement harness description**: How to run the benchmarks (at minimum, benchmark names and what they simulate).

4. **Adjustment guidance**: "These are validated against the design point. If your design point differs, adjust with reasoning — but document the new targets and re-validate."

**Quality criteria**: An implementer can run the benchmarks and get a pass/fail signal without asking anyone.

**DO NOT** include performance claims without numbers, design points, or measurement methods. (Validates INV-005.)

**DO NOT** anchor budgets to "current hardware" — name the exact machine or class (e.g., "M1 Max, 32GB RAM") so budgets are reproducible. (Validates INV-005.)

**DO NOT** omit adjustment guidance — without it, implementers treat budgets as absolute on different hardware or ignore them entirely. (Validates INV-008.)

**Anti-pattern**: "The system should be fast enough for real-time use." ← No number, no design point, no measurement method.

---

### 3.8 Negative Specifications

**What it is**: Explicit constraints on what the system (or subsystem) must NOT do, organized per implementation chapter.

**Why it exists**: LLMs fill specification gaps with plausible but unauthorized behaviors (§0.2.2). Negative specifications are co-located with the subsystem they constrain and use imperative language LLMs follow — more effective than distant anti-patterns. (Locked by ADR-009, validates INV-017.)

**Required format per implementation chapter**:
```
### Negative Specifications for [Subsystem]

- **DO NOT** [specific prohibited behavior]. [Brief reason or invariant reference.]
- **DO NOT** [specific prohibited behavior]. [Brief reason or invariant reference.]
- **DO NOT** [specific prohibited behavior]. [Brief reason or invariant reference.]
```

**Quality criteria for each negative specification**:
- Addresses a plausible action an implementer (especially an LLM) might take
- Is falsifiable: you can mechanically check whether the implementation violates it
- References the invariant or ADR it protects
- Is specific to the subsystem, not a generic platitude ("DO NOT write bugs" is not useful)

**Quantity guidance**: 3–8 per subsystem. Fewer suggests under-specification; more suggests the positive spec is ambiguous.

**DO NOT** write generic negative specs ("DO NOT introduce security vulnerabilities") — write subsystem-specific constraints. (Validates INV-017.)

**DO NOT** let negative specs exceed 8 per subsystem — more than 8 suggests the positive spec is too ambiguous. (Validates INV-007.)

**DO NOT** write negative specs that simply negate a positive requirement ("DO NOT fail to implement X") — address plausible wrong behaviors, not restated requirements. (Validates INV-017.)

**Anti-patterns**:
```
❌ BAD: "DO NOT write bad code."
  ← Not specific, not falsifiable, not subsystem-specific.

❌ BAD: "DO NOT use global variables."
  ← Generic programming advice, not a spec-level constraint.

✅ GOOD: "DO NOT bypass the reservation system for file writes.
  All file mutations must go through the ReservationManager (INV-022).
  Direct filesystem writes will cause data races with concurrent agents."

✅ GOOD: "DO NOT assume event ordering beyond the guarantees in APP-INV-017.
  Events from different agents may arrive out of wall-clock order.
  The only ordering guarantee is per-agent causal ordering."
```

**Self-bootstrapping demonstration**: This document includes negative specifications throughout its own element specifications (the "DO NOT" paragraphs in §2.1, §2.2, §3.1, §3.2, etc.).

### Verification Prompt for Chapter 3 (PART 0 Elements)

After writing your spec's PART 0, verify:
1. [ ] Every non-negotiable could tempt an implementer to violate it under pressure — none are trivially obvious (§3.1)
2. [ ] Every non-goal is something someone would plausibly request (INV-017)
3. [ ] First-principles model is formal enough to derive the architecture independently (INV-001)
4. [ ] Every invariant has all five components: statement, formal expression, violation scenario, validation method, WHY THIS MATTERS (INV-003)
5. [ ] Every ADR has ≥ 2 genuine alternatives a competent engineer could choose between (INV-002)
6. [ ] Performance budgets have numbers, design points, and measurement methods (INV-005)
7. [ ] PART 0 does NOT contain restated-invariant non-negotiables (§3.1), strawman ADRs (§3.5), or unfalsifiable invariants (§3.4)
8. [ ] *Integration*: PART 0 invariants and ADRs are referenced by at least one PART II chapter (INV-006)

---

## Chapter 4: PART I Elements

### 4.1 Full Formal Model

**What it is**: The expanded version of the first-principles derivation from §0.2. While the executive summary gives the 1-page version, PART I gives the full treatment:

- Complete state definition (all fields, all types)
- Complete input/event taxonomy
- Complete output/effect taxonomy
- State transition semantics
- Composition rules (how subsystems interact)

**DO NOT** copy-paste the §0.2 executive summary as the full formal model — include complete state definitions, input/output taxonomies, transition semantics, and composition rules. (Validates INV-004.)

**DO NOT** define the full formal model as a single monolithic block — decompose into state definition, input taxonomy, output taxonomy, transition semantics, and composition rules as separate subsections. (Validates INV-004.)

### 4.2 State Machines

**What it is**: Every stateful component gets a formal state machine.

**Required per state machine**:
- State diagram (ASCII art or description)
- State × Event table (what happens for every combination — no empty cells)
- Guard conditions on transitions
- Invalid transition policy (ignore? error? log?)
- Entry/exit actions

**Quality criteria**: The state × event table has no empty cells. Every cell either names a transition or explicitly says "no transition" or "error."

**DO NOT** define state machines with only happy-path transitions — LLMs implement only the transitions shown. (Validates INV-010.)

**DO NOT** present state machines as only ASCII diagrams — the state x event table is the formal specification; diagrams are supplementary. (Validates INV-010.)

**DO NOT** define guard conditions as prose — express guards as boolean predicates evaluable by an implementer or test. (Validates INV-003, INV-010.)

### 4.3 Complexity Analysis

**What it is**: Complexity bounds for every fundamental operation defined in the first-principles model.

**Required**: Big-O bounds with constants where they matter for the design point. "O(n) where n = active_agents, expected ≤ 300" is more useful than "O(n)."

**DO NOT** provide complexity bounds without anchoring to the design point — "O(n²)" is meaningless without knowing n. (Validates INV-005.)

### Verification Prompt for Chapter 4 (PART I Elements)

After writing your spec's PART I (Foundations), verify:
1. [ ] Full formal model includes complete state, input, output, and transition definitions — not just §0.2 summary (§4.1)
2. [ ] Every state machine has a state x event table with NO empty cells (INV-010)
3. [ ] Invalid transition policies are explicit for every state machine (INV-010, INV-017)
4. [ ] Complexity analysis includes constants at the design point, not just asymptotic bounds (§4.3)
5. [ ] PART I does NOT have happy-path-only state machines (§4.2) or unanchored complexity bounds
6. [ ] *Integration*: PART I state machines cover all states referenced by PART II chapters (INV-010)

---

## Chapter 5: PART II Elements

### 5.1 Implementation Chapters

**What it is**: One chapter per major subsystem — where the spec earns its value.

**Required components per chapter**:

1. **Purpose statement** (2–3 sentences): What this subsystem does and why. References the formal model.
2. **Formal types**: Data structures with layout analysis. Include `// WHY NOT` annotations (§5.4) and comparison blocks (§5.5).
3. **Algorithm pseudocode**: Every non-trivial algorithm with inline complexity analysis.
4. **State machine** (if stateful): Full state machine per §4.2.
5. **Invariants preserved** (RESTATED): **Restate each invariant's one-line statement, not just the ID** (INV-018).
6. **Negative specifications**: 3–8 "DO NOT" constraints per §3.8 (INV-017).
7. **Worked example(s)**: Concrete scenario with specific values, not variables.
8. **Edge cases and error handling**: Malformed inputs, resource exhaustion, invariant threats.
9. **Test strategy**: Which test types (unit, property, integration, replay, stress) apply.
10. **Performance budget**: Subsystem's share of the overall budget.
11. **Verification prompt**: Structured self-check per §5.6.
12. **Meta-instructions** (if applicable): Ordering directives per §5.7.
13. **Cross-references**: To ADRs, invariants, other subsystems, formal model. Include proportional weight (§9.1).

**Quality criteria**: An implementer could build this subsystem from this chapter alone. (Understanding composition requires other chapters, but each chapter is self-contained for its subsystem.)

> **Self-bootstrapping note (ADR-004):** For meta-standards (standards about specification authoring), not all 13 components apply literally. Formal types, algorithm pseudocode, and state machines are N/A for element specification chapters (which define document formats, not system behavior). The applicable components are: purpose statement, invariants preserved (RESTATED), negative specifications, worked examples (as anti-patterns), quality criteria (serving as test strategy), verification prompt, meta-instructions, and cross-references. This document demonstrates this adaptation.

**DO NOT** write implementation chapters before locking their ADR dependencies. **DO NOT** reference invariants by ID alone — restate them per INV-018. (Validates INV-001, INV-018.)

---

### 5.2 Worked Examples

**What it is**: A concrete scenario with specific values (not variables) showing the subsystem processing a realistic input.

**Required properties**:
- Uses concrete values: `task_id = T-042`, not "some task"
- Shows state before, the operation, and state after
- Includes at least one non-trivial aspect (an edge case, a conflict, a boundary condition)

**DO NOT** use variables or placeholders — LLMs over-index on examples; "some task" produces vague implementations while `task_id = T-042` teaches precision. (Validates INV-017.)

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

**Why it exists**: Individual examples prove each piece works. The end-to-end trace proves the pieces fit together. Most bugs live at subsystem boundaries. (Validates INV-001: every section traces to the formal model.)

**DO NOT** trace only the happy path — include at least one cross-subsystem failure mode. LLMs implement only the cases shown. (Validates INV-003.)

**DO NOT** use abstract values at subsystem boundaries — "The scheduler sends a message" is abstract; "The scheduler emits `TaskAssigned { task_id: T-042, agent_id: A-007, deadline: 1500ms }`" is concrete. (Validates INV-004.)

**DO NOT** skip intermediary subsystems — show every A->B->C handoff explicitly. LLMs implement direct coupling when intermediaries are omitted. (Validates INV-006.)

**Anti-pattern**:
```
❌ BAD:
  "An event enters the system. It gets processed by the scheduler, then the
  event store, then the TUI. The output is correct."
  ← No concrete values. No data at boundaries. No invariant annotations.
    No failure mode. Happy-path-only.

✅ GOOD (excerpt from §1.4, core-standard module):
  "Step 3: ADR Creation (§3.5) — The author writes ADR-002 per §3.5 format:
   Problem: Aspirational, formally proven, or falsifiable?
   Options: Three genuine alternatives with concrete pros/cons.
   Decision: Option C (falsifiable) with WHY NOT annotations.
   Tests: 'Validated by INV-003' — forward reference to the enforcing invariant."
  ← Concrete values, specific section references, traceable cross-subsystem path.
```

**Self-bootstrapping demonstration**: This document includes an end-to-end trace in §1.4 (core-standard module) — tracing ADR-002 from recognition through the full DDIS authoring process.

---

### 5.4 WHY NOT Annotations

**What it is**: Inline comments next to design choices explaining the road not taken.

**When to use**: Whenever a design choice might look suboptimal to an implementer who doesn't have the full context. If an implementer might think "I can improve this by doing X instead," and X was considered and rejected, add a WHY NOT annotation.

**Format**:
```
// WHY NOT [alternative]? [Brief tradeoff explanation. Reference ADR-NNN if a full ADR exists.]
```

**Relationship to ADRs**: WHY NOT annotations are micro-justifications for local choices. ADRs are macro-justifications for architectural choices. If a WHY NOT annotation grows beyond 3 lines, it should become an ADR.

**DO NOT** use WHY NOT for micro-decisions (variable naming, formatting, import ordering) — reserve for design choices where a reasonable alternative was rejected. (Validates INV-007.)

**DO NOT** let WHY NOT annotations exceed 3 lines — promote longer explanations to an ADR (§3.5). (Validates INV-002.)

**Anti-pattern**:
```
❌ BAD:
  // WHY NOT use a different approach? Because this one is better.
  ← No tradeoff, no specificity, no ADR reference. "Better" is not a reason.

✅ GOOD:
  // WHY NOT mandatory locking? Causes priority inversion under high contention
  // (measured: 3× latency at 200 concurrent agents). Advisory locking avoids
  // this at cost of occasional stale reads. See ADR-003.
  ← Named alternative, quantified tradeoff, ADR reference for full analysis.
```

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

**DO NOT** use comparison blocks without quantified reasoning — numbers are required so LLMs can evaluate context-dependent tradeoffs. (Validates INV-005.)

**DO NOT** compare more than 2 approaches in one block — promote three-way comparisons to a full ADR (§3.5). (Validates INV-002.)

**Anti-pattern**:
```
❌ BAD:
  // ❌ SUBOPTIMAL: HashMap
  //   - Slower for small collections
  // ✅ CHOSEN: BTreeMap
  //   - Faster for small collections
  ← Qualitative only ("slower", "faster"). No numbers. No design point.
    LLM cannot evaluate whether this holds for n=10 vs n=10,000.

✅ GOOD:
  // ❌ SUBOPTIMAL: HashMap<TaskId, Task>
  //   - O(1) amortized lookup, but 24 bytes overhead per entry (hash + metadata)
  //   - At design point (n ≤ 100 tasks), cache misses dominate: ~85ns per lookup
  // ✅ CHOSEN: BTreeMap<TaskId, Task>
  //   - O(log n) lookup, 8 bytes overhead per entry
  //   - At design point (n ≤ 100): ~12ns per lookup (cache-line friendly)
  //   - See ADR-005 for full benchmark analysis
```

---

### 5.6 Verification Prompts

**What it is**: A structured self-check at the end of each implementation chapter for verifying output against the spec before moving on. (Locked by ADR-010.)

**Required format**:
```
### Verification Prompt for [Subsystem]

After implementing this subsystem, verify:
1. [ ] [Specific positive check referencing INV-NNN: "Your implementation preserves INV-NNN by..."]
2. [ ] [Specific positive check: "The [algorithm] handles [edge case] by..."]
3. [ ] [Specific negative check: "Your implementation does NOT [prohibited behavior from §3.8]"]
4. [ ] [Specific integration check: "Your implementation's output is compatible with [adjacent subsystem]"]
```

**Quality criteria**:
- Each check is executable by the implementer without additional information
- At least one check is a positive invariant verification
- At least one check is a negative verification (references a negative specification)
- At least one check verifies integration with an adjacent subsystem
- Checks reference specific invariants (INV-NNN) or negative specifications

**DO NOT** write generic verification prompts ("did you test your code?") — each check must reference concrete invariants or constraints specific to the subsystem. (Validates INV-017.)

**Self-bootstrapping demonstration**: This document's element specifications implicitly serve as verification prompts — each quality criteria section tells the author what to check. An explicit verification prompt for this meta-standard:

> **Verification Prompt for a DDIS-conforming spec:**
> After writing your spec, verify:
>
> *Positive checks:*
> 1. [ ] Five random sections trace to the formal model (INV-001, Gate 2)
> 2. [ ] Every design choice with a reasonable alternative has an ADR with ≥ 2 options (INV-002, Gate 3)
> 3. [ ] Every algorithm has pseudocode, complexity, ≥ 1 example, and test strategy (INV-004)
> 4. [ ] Every performance claim has benchmark, design point, and measurement method (INV-005)
> 5. [ ] Cross-reference graph has no orphan sections (INV-006, Gate 5)
> 6. [ ] Spec is self-contained: implementer needs no external information (INV-008)
> 7. [ ] Every domain-specific term defined in glossary (INV-009)
> 8. [ ] Every state machine has complete state x event table, no empty cells (INV-010)
> 9. [ ] Every implementation chapter has ≥ 3 negative specifications (INV-017)
> 10. [ ] Every implementation chapter restates preserved invariants with ID + statement (INV-018)
> 11. [ ] Implementation ordering is an explicit DAG with dependency reasons (INV-019)
> 12. [ ] Every element spec chapter has a verification prompt block (INV-020)
>
> *Negative checks:*
> 13. [ ] No aspirational invariants without violation scenarios (INV-003)
> 14. [ ] No strawman ADR alternatives where one option is obviously inferior (§3.5, Gate 3)
> 15. [ ] No "see above" references — all cross-refs use §X.Y, INV-NNN, or ADR-NNN (INV-006)
>
> *Integration check:*
> 16. [ ] An LLM given one chapter + glossary + invariants produces correct output without hallucination (Gate 7)

---

### 5.7 Meta-Instructions

**What it is**: Directives to the LLM implementer providing ordering, sequencing, and process guidance that human implementers infer from experience.

**Required format**:
```
> **META-INSTRUCTION**: [Directive to the implementer]
> Reason: [Why this ordering/process matters]
```

**When to use**:
- When implementation order matters and getting it wrong causes cascading rework (supports INV-019)
- When a common implementation shortcut would violate an invariant
- When the spec intentionally leaves a micro-decision to the implementer but wants to constrain the decision process

**DO NOT** use meta-instructions for specification content — meta-instructions are process guidance ("implement X before Y"), not property declarations ("X must have property P"). (Validates INV-017.)

**Example**:
```
> **META-INSTRUCTION**: Implement the event store before the scheduler.
> Reason: The scheduler depends on event store types (EventId, EventPayload)
> defined in the storage domain constitution. Implementing the scheduler first
> will require placeholder types that inevitably diverge from the real types.

> **META-INSTRUCTION**: Do not optimize the dispatch hot path until
> Benchmark B-003 confirms it is actually the bottleneck at the design point.
> Reason: Premature optimization of dispatch is the #1 cause of unnecessary
> complexity in coordination systems (see ADR-005, consequences).
```

**Self-bootstrapping demonstration**: This document includes meta-instructions in §0.3.1 (reading order for LLM implementers) and §11.1 (authoring sequence, guidance-operations module).

### Verification Prompt for Chapter 5 (PART II Elements)

After writing your spec's implementation chapters, verify:
1. [ ] Each chapter has all 13 required components from §5.1
2. [ ] Preserved invariants are RESTATED with ID + one-line statement, not bare IDs (INV-018)
3. [ ] Each chapter has ≥ 3 subsystem-specific negative specifications per §3.8 (INV-017)
4. [ ] Worked examples use concrete values (task_id = T-042), not variables (§5.2)
5. [ ] Verification prompts include positive, negative, AND integration checks with INV-NNN refs (§5.6)
6. [ ] Meta-instructions use `> **META-INSTRUCTION**:` format with dependency reasons (§5.7, INV-019)
7. [ ] Chapters do NOT reference invariants by ID alone (INV-018), use "see above" (INV-006), or have generic negative specs (INV-017)
8. [ ] *Integration*: PART II chapters cross-reference their ADR dependencies in PART 0 (INV-001, INV-006)

---

## Chapter 6: PART IV Elements

### 6.1 Operational Playbook

**What it is**: Prevents the most common failure mode of detailed specs: infinite refinement without shipping.

**Required sections**:

#### 6.1.1 Phase -1: Decision Spikes

Run tiny experiments to de-risk the hardest unknowns before building. Each spike produces an ADR.

**Required per spike**: What question it answers, maximum time budget (1–3 days), exit criterion (one ADR).

**DO NOT** define decision spikes without explicit time budgets — an open-ended spike is a project, not a spike. (Validates INV-019.)

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

Build order chosen to maximize the "working subset" at each stage. The first deliverable exercises the core loop, not a complete system missing its core. Must be an explicit DAG with dependency reasons (INV-019).

**DO NOT** present the deliverable order as a flat list — LLMs implement in list order, not dependency order. Make the DAG explicit. (Validates INV-019.)

#### 6.1.5 Immediate Next Steps (First PRs)

The literal first 5–6 things to implement, in dependency order. Not strategic — tactical. Converts the spec from "a plan to study" into "a plan to execute now."

---

### 6.2 Testing Strategy

**What it is**: A taxonomy of test types with examples.

**Required taxonomy** (adapt to domain):

| Test Type | What It Validates | Example |
|---|---|
| Unit | Function correctness | Conflict detection returns correct overlaps |
| Property | Invariant preservation under random inputs | replay(snapshot, events) = direct_state |
| Integration | Subsystem composition | Task completion triggers scheduling cascade |
| Stress | Design-point limits | 300 agents, 10K tasks, sustained 60s |
| Replay | Determinism | Process N events, snapshot, replay, byte-compare |
| Adversarial | Malicious/malformed input robustness | Event with forged task_id |
| LLM conformance | Spec faithfulness | Implementation matches negative specs (Gate 7) |

**DO NOT** write only unit tests — at minimum include unit, property, integration, and stress types. (Validates INV-003.)

**DO NOT** omit property tests for invariant-heavy systems. (Validates INV-003.)

**DO NOT** skip LLM conformance tests — Gate 7 requires a concrete test suite. (Validates INV-020.)

---

### 6.3 Error Taxonomy

**What it is**: Classification of errors with handling strategy per class.

**Required per error class**:
- Severity (fatal, degraded, recoverable, ignorable)
- Handling strategy (crash, retry, degrade, log-and-continue)
- Which invariants the error class threatens

For specification authoring errors, see Appendix C.

**DO NOT** conflate severity with handling strategy — "recoverable" + "crash" signals inconsistency. (Validates INV-017.)

### Verification Prompt for Chapter 6 (PART IV Elements)

After writing your spec's operational chapters, verify:
1. [ ] Phase -1 decision spikes have time budgets and ADR exit criteria (§6.1.1)
2. [ ] Every phase has a specific, testable exit criterion (§6.1.2, INV-003)
3. [ ] Minimal deliverables order is an explicit DAG with dependency reasons (INV-019)
4. [ ] Testing strategy includes at minimum unit, property, integration, and stress types with examples (§6.2)
5. [ ] Error taxonomy maps each class to severity, handling strategy, and threatened invariants (§6.3)
6. [ ] Operational chapters do NOT use aspirational exit criteria, generic test types, or incomplete error classes
7. [ ] *Integration*: Playbook phases reference quality gates from §0.7 for phase exit (INV-019)

---

## Chapter 7: Appendix Elements

### 7.1 Glossary

**What it is**: Every domain-specific term, defined in 1–3 sentences with a cross-reference to where it's formally specified.

**Required properties**:
- Alphabetized
- Each entry includes (see §X.Y) pointing to the formal definition
- Terms that have both a common meaning and a domain-specific meaning clearly distinguish the two

**DO NOT** define terms with circular references ("task: a unit of work in the task system"). **DO NOT** assume common-English meaning suffices for domain terms — LLMs default to the most common meaning. (Validates INV-009.)

**Anti-pattern**: Defining "task" as "a unit of work." Define it as "a node in the task DAG representing a discrete, assignable unit of implementation work with explicit dependencies, acceptance criteria, and at most one assigned agent at any time (see §7.2, INV-012)."

---

### 7.2 Risk Register

**What it is**: Top 5–10 risks to the project, each with a concrete mitigation.

**Required per risk**:
- Risk description (what could go wrong)
- Impact (what happens if it materializes)
- Mitigation (what we do about it)
- Detection (how we know it's happening)

**DO NOT** list risks without detection methods — undetectable risks cannot be mitigated in time. (Validates INV-003.)

**DO NOT** write aspirational mitigations ("we will monitor closely") — each must be a specific, actionable procedure. (Validates INV-017.)

**DO NOT** list more than 10 risks — focus on risks that threaten invariants or quality gates. (Validates INV-007.)

---

### 7.3 Master TODO Inventory

**What it is**: A comprehensive, checkboxable task list organized by subsystem, cross-referenced to phases and ADRs.

**Required properties**:
- Organized by subsystem (not by phase — phases cut across subsystems)
- Each item is small enough to be a single PR
- Cross-references to the ADR or invariant that justifies it
- Checkboxable format (`- [ ]`) so the document serves as a living tracker

**DO NOT** organize the Master TODO by phase alone — LLMs implementing one subsystem need all related tasks in one place. (Validates INV-017.)

### Verification Prompt for Chapter 7 (Appendix Elements)

After writing your spec's appendices, verify:
1. [ ] Glossary defines every domain-specific term with cross-reference to formal definition (INV-009)
2. [ ] Glossary distinguishes domain-specific from common-English meanings (INV-009)
3. [ ] Risk register includes detection methods, not just mitigations (§7.2)
4. [ ] Master TODO organized by subsystem, cross-referenced to ADRs and phases (§7.3)
5. [ ] No circular glossary definitions or risks without detection methods
6. [ ] *Integration*: Glossary covers every bold/code-formatted term across PART II chapters (INV-009)

---
module: guidance-operations
domain: guidance
maintains: []
interfaces: [INV-001, INV-003, INV-006, INV-009, INV-017, INV-018, INV-019, INV-020]
implements: [ADR-004, ADR-005, ADR-011]
adjacent: [core-standard, element-specifications]
negative_specs:
  - "Must NOT let voice shift between sections"
  - "Must NOT hedge in invariants, ADRs, or negative specifications"
  - "Must NOT use implicit references like 'see above'"
  - "Must NOT treat anti-patterns as substitute for negative specifications"
---

# Guidance, Operations & Reference Module

**Invariants referenced from other modules (INV-018 compliance):**
- INV-001: Every implementation section traces to at least one ADR or invariant (maintained by core-standard module)
- INV-003: Every invariant can be violated by a concrete scenario and detected by a named test (maintained by core-standard module)
- INV-006: The specification contains a cross-reference web where no section is an island (maintained by core-standard module)
- INV-009: Every domain-specific term used in the specification is defined in the glossary (maintained by core-standard module)
- INV-017: Every implementation chapter includes explicit "DO NOT" constraints preventing likely hallucination patterns (maintained by core-standard module)
- INV-018: Every implementation chapter restates the invariants it must preserve, not merely referencing them by ID (maintained by core-standard module)
- INV-019: The spec provides an explicit dependency chain for implementation ordering (maintained by core-standard module)
- INV-020: Every element specification chapter includes a structured verification prompt block (maintained by core-standard module)

---

# PART III: GUIDANCE (RECOMMENDED)

> Note: For the DDIS meta-standard, "PART III: Interfaces" from the prescribed structure (§0.3) maps to "Guidance" — the voice, proportional weight, and cross-reference patterns ARE the interfaces through which authors interact with DDIS. The structural elements (§0.3) may be renamed to fit the domain.

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

**DO NOT** let the voice shift between sections — LLMs mirror inconsistency. **DO NOT** hedge in invariants, ADRs, or negative specs — LLMs treat hedged requirements as optional. (Validates INV-017.)

**LLM-specific voice guidance**: LLMs generate in the voice they're trained on. Specifically:
- **Active voice** — LLMs default to passive ("it is recommended that..."). Active ("the system retries three times, then fails") produces clearer implementation.
- **Concrete numbers** — Vague qualifiers ("quickly") produce untestable code. Use "< 1ms", "at most 3 retries."
- **Explicit names** — Without domain-specific names, LLMs generate "data", "handler", "process."

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
- Blockquotes for the preamble elements and meta-instructions (§5.7, element-specifications module)
- ASCII diagrams preferred over external image references (the spec should be readable in any text editor)

### 8.3 Anti-Pattern Catalog

Every DDIS element has bad and good examples defined in its specification (PART II). This section collects cross-cutting anti-patterns that affect multiple elements:

**Anti-pattern: The Hedge Cascade**
```
❌ "It might be worth considering potentially using a single-threaded loop,
which could arguably provide some benefits in terms of determinism."
✅ "The kernel loop is single-threaded — deterministic replay. See ADR-003."
```

**Anti-pattern: The Orphan Section**
A section that references nothing and is referenced by nothing. Connect it to the web or remove it.

**Anti-pattern: The Trivial Invariant**
"INV-042: The system uses UTF-8 encoding." Either enforced by the platform (not worth an invariant) or so fundamental it belongs in Non-Negotiables.

**Anti-pattern: The Strawman ADR**
```
❌ Options:
  A) Our chosen approach (clearly the best)
  B) A terrible approach nobody would choose
  Decision: A, obviously.
```
Every option in an ADR must have a genuine advocate — a competent engineer who, in a different context, would choose it.

**Anti-pattern: The Missing Verification Prompt**
An implementation chapter with negative specs and invariant references but no verification prompt block — the LLM has no structured self-check. (§5.6, INV-020.)

**Anti-pattern: The Percentage-Free Performance Budget**
"The system should respond quickly." Without a number, a design point, and a measurement method, this is a wish, not a budget.

**Anti-pattern: The Spec That Requires Oral Tradition**
If an implementer must ask the architect a question the spec should have answered, the spec has a gap. Patch gaps in; track questions during implementation (§1.1).

**Anti-pattern: The Afterthought LLM Section**
A "Chapter N: LLM Considerations" appendix bolted onto an LLM-unaware spec. Provisions must be woven throughout. (ADR-008.)

**DO NOT** treat anti-patterns as a substitute for subsystem-specific negative specifications (§3.8, element-specifications module) — anti-patterns are document-level; negative specs are subsystem-level. Both required. (Validates ADR-009.)

### Verification Prompt for Chapter 8 (Voice and Style)

After writing your spec's voice and style guidance, verify:
1. [ ] Voice is consistent across all sections — no shifts between casual, formal, or bureaucratic registers (§8.1, INV-017)
2. [ ] Anti-pattern catalog includes subsystem-specific examples, not just generic advice (§8.3, INV-017)
3. [ ] Your voice guidance does NOT hedge in requirements or constraints — hedging causes LLMs to treat requirements as optional (§8.1)
4. [ ] Your anti-patterns do NOT substitute for subsystem-level negative specifications (§8.3, INV-017)
5. [ ] *Integration*: Your anti-patterns reference specific invariants and ADRs from PART 0, not just abstract principles (INV-006)

---

## Chapter 9: Proportional Weight Deep Dive

### 9.1 Identifying the Heart

Every system has a "heart" — the 2–3 subsystems where most complexity and bugs live. These should receive 40–50% of the PART II line budget. The ring architecture (§0.4, system constitution) determines which sections are sacred (must-follow) versus recommended, which in turn constrains proportional weight allocation.

**How to identify the heart**:
- Which subsystems have the most invariants?
- Which subsystems have the most ADRs?
- Which subsystems appear in the most cross-references?
- If you had to cut the spec in half, which subsystems would you keep?

**DO NOT** apply domain-spec proportions (§0.8.2) to a meta-standard — meta-standards have fundamentally different weight distributions because PART 0 IS the substance. (Validates INV-007.)

**DO NOT** treat proportional weight as absolute — ±20% adjustment with documented reasoning is valid per §0.8.2. (Validates INV-007.)

**DO NOT** diagnose weight imbalance without checking the spec type first — §9.2 signals apply to domain specs; a meta-standard with 50% PART 0 is healthy. (Validates INV-017.)

### 9.2 Signals of Imbalanced Weight

- A subsystem with 5 invariants and 50 lines of spec is **starved**
- A subsystem with 1 invariant and 500 lines of spec is **bloated**
- PART 0 longer than PART II means the spec is top-heavy (more framing than substance)
- Appendices longer than PART II means reference material is displacing implementation spec

### Verification Prompt for Chapter 9 (Proportional Weight)

After writing your spec's proportional weight guidance, verify:
1. [ ] You have identified the 2–3 "heart" subsystems and they receive 40–50% of the PART II line budget (§9.1)
2. [ ] You have checked for the four imbalance signals: starved subsystems, bloated subsystems, top-heavy PART 0, oversized appendices (§9.2)
3. [ ] Your weight analysis does NOT apply domain-spec proportions to a meta-standard without adjustment (§9.1, INV-007)
4. [ ] Your proportional weight does NOT treat the §0.8.2 percentages as absolute — ±20% adjustment with documented reasoning is valid (INV-007)
5. [ ] *Integration*: Your weight analysis cross-references the invariant count per subsystem from §0.5 (constitution) to validate that heavy subsystems are heavy for good reason (INV-006)

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

**DO NOT** use implicit references ("see above", "as mentioned earlier") — LLMs cannot resolve positional context. Always use explicit §X.Y, INV-NNN, or ADR-NNN identifiers. (Validates INV-006.)

**DO NOT** create bidirectional references for trivially related sections — §A referencing §B does not require §B to reference §A unless §B genuinely depends on §A. (Validates INV-007.)

**DO NOT** rely on section ordering as a substitute for explicit cross-references — modular bundles may not preserve document order. (Validates INV-006.)

### 10.2 Reference Density Targets

| Section Type | Minimum Outbound References |
|---|---|
| Implementation chapter | 3 (at least: one ADR, one invariant, one other chapter) |
| ADR | 2 (at least: one invariant, one implementation chapter) |
| Invariant | 1 (at least: one test or validation method) |
| Performance budget | 2 (at least: one benchmark, one design point) |
| Test strategy | 2 (at least: one invariant, one implementation chapter) |
| Negative specification | 1 (at least: one invariant or ADR it protects) |
| Verification prompt | 2 (at least: one invariant, one negative specification) |

### Verification Prompt for Chapter 10 (Cross-Reference Patterns)

After writing your spec's cross-reference patterns, verify:
1. [ ] All references use explicit §X.Y, INV-NNN, or ADR-NNN identifiers — zero implicit "see above" references (§10.1, INV-006)
2. [ ] Each section type meets or exceeds the minimum outbound reference count from the §10.2 density table
3. [ ] Your cross-references do NOT use implicit positional language ("see above", "as mentioned earlier") (§10.1, INV-006)
4. [ ] Your references do NOT create gratuitous bidirectional links for trivially related sections (§10.1, INV-007)
5. [ ] *Integration*: Your cross-reference web connects to invariants in §0.5 (constitution) and ADRs in §0.6 (constitution), forming a connected graph with no orphan sections (INV-006, Gate 5)

---

# PART IV: OPERATIONS

## Chapter 11: Applying DDIS to a New Project

### 11.1 The Authoring Sequence

> **META-INSTRUCTION (for spec authors):** Write sections in this order (not document order) to minimize rework. Do not skip steps or reorder — the dependency chain between steps is real. See §0.9 (system constitution) for the complete interface DDIS provides.

**DO NOT** write in document order — use authoring order below. **DO NOT** skip negative specs (step 7) or verification prompts (step 11) — these cannot be retrofitted without re-reading each chapter. (Validates INV-019.)

**DO NOT** treat the authoring sequence as inflexible — experienced authors may batch steps within the same dependency tier, but reordering across tiers causes cascading rework regardless of experience. (Validates INV-019.)

1. **GOAL: Design goal + Core promise** — no dependencies; start here
2. **FORMAL: First-principles formal model** — depends on GOAL
3. **NON-NEG: Non-negotiables** — depends on GOAL, FORMAL
4. **INV: Invariants** — depends on FORMAL, NON-NEG: formalizes model properties and commitments
5. **ADR: ADRs** — depends on INV: writing invariants first reveals which decisions matter
6. **IMPL: Implementation chapters** — depends on INV, ADR: heaviest subsystems first
7. **NEG-SPEC: Negative specifications per chapter** — depends on IMPL: identify what an LLM might get wrong
8. **TRACE: End-to-end trace** — depends on IMPL: requires all subsystems drafted
9. **PERF: Performance budgets** — depends on IMPL: anchor budgets to specified operations
10. **TEST: Test strategies** — depends on INV, IMPL, NEG-SPEC
11. **VERIFY: Verification prompts per chapter** — depends on INV, NEG-SPEC, TEST
12. **XREF: Cross-references** — depends on steps 1-11: weaves the web
13. **GLOSS: Glossary** — depends on steps 1-12: writing earlier means missing terms
14. **TODO: Master TODO** — depends on IMPL, PERF, PLAYBOOK
15. **PLAYBOOK: Operational playbook** — depends on ADR, IMPL
16. **META-INST: Meta-instructions** — depends on IMPL, TODO, PLAYBOOK

### 11.2 Common Mistakes in First DDIS Specs

1. **Writing implementation chapters before ADRs.** You'll rewrite them when ADRs imply different choices.
2. **Writing the glossary first.** You don't know your terminology until you've written the spec.
3. **Treating the end-to-end trace as optional.** It's the single most effective quality check.
4. **Under-investing in WHY NOT annotations.** Every non-obvious choice needs one.
5. **Skipping the anti-patterns.** LLMs especially benefit from negative examples.
6. **Omitting negative specifications.** Without explicit "DO NOT" constraints, LLMs invent plausible but unauthorized behavior (§3.8, INV-017).
7. **Referencing invariants by ID only.** INV-017 means nothing 2,000 lines from its definition — restate it (INV-018).

### Verification Prompt for Chapter 11 (Applying DDIS)

After writing your spec's application guidance, verify:
1. [ ] The authoring sequence explicitly states dependency reasons between steps — not just ordering, but WHY each step depends on its prerequisites (§11.1, INV-019)
2. [ ] Common mistakes section includes at least the 7 listed pitfalls, each with a concrete consequence (§11.2)
3. [ ] Your authoring guidance does NOT prescribe writing in document order — the authoring sequence follows dependency order, not section numbering (§11.1, INV-019)
4. [ ] Your authoring guidance does NOT skip negative specs (step 7) or verification prompts (step 11) (§11.1, INV-017)
5. [ ] *Integration*: Your authoring sequence references the element specifications in PART II (element-specifications module) that define the format for each step's output (INV-006)

---

## Chapter 12: Validating a DDIS Specification

### 12.1 Self-Validation Checklist

**DO NOT** skip self-validation or treat it as polish. **DO NOT** validate gates out of order — a failing Gate 1 makes later gates irrelevant. (Validates INV-003, INV-020.)

**DO NOT** declare validation complete after only mechanical checks — Gates 6 and 7 require human or LLM judgment. Structurally conforming specs can still be practically unusable. (Validates INV-003.)

Before declaring a spec complete, the author should:

1. Pick 5 random implementation sections. Trace each to the formal model — did any chain break? (Gate 2)
2. Read each ADR's alternatives. Would a competent engineer choose any rejected option? (Gate 3)
3. For each invariant, try to construct a violation scenario — if you can't, it's trivially true or too vague. (Gate 4)
4. Build the cross-reference graph. Are there orphan sections? (Gate 5)
5. Read the spec as a first-time implementer. Where did you guess? (Gate 6)
6. Give 2+ implementation chapters (plus glossary and invariants) to an LLM. Would it hallucinate? Know what NOT to do? (Gate 7)
7. Check proportional weight against §9.2 signals. Any section starved or bloated? PART 0 heavier than PART II?

### 12.2 External Validation

Give the spec to an implementer (or LLM) and track: questions it should have answered (gaps), incorrect implementations not prevented (ambiguities), skipped sections (voice/clarity), added behaviors not in spec (missing negative specs).

### Verification Prompt for Chapter 12 (Validating a DDIS Specification)

After writing your spec's validation guidance, verify:
1. [ ] The self-validation checklist covers all 7 quality gates in order, with each gate referencing specific invariants (§12.1, INV-003)
2. [ ] External validation tracks four categories: gaps, ambiguities, clarity issues, and missing negative specs (§12.2)
3. [ ] Your validation guidance does NOT skip judgment-based gates (Gate 6, Gate 7) after mechanical checks pass (§12.1, INV-003)
4. [ ] Your validation guidance does NOT validate gates out of order — failing Gate N makes later gates irrelevant (§12.1, INV-020)
5. [ ] *Integration*: Your validation checklist references the quality gates defined in §0.7 (constitution) and the invariant definitions in the core-standard module (INV-003, INV-006)

---

## Chapter 13: Evolving a DDIS Specification

### 13.1 The Living Spec

**DO NOT** treat the Living state as permission for informal changes — every modification must maintain INV-001, INV-006, and quality gates. **DO NOT** delete superseded ADRs — follow the supersession protocol (ADR-011, §13.3).

Once implementation begins, the spec enters the Living state (§1.1, core-standard module). In this state:

- **Gaps** are patched into the spec, not oral tradition.
- **ADRs may be superseded.** Mark as "Superseded by ADR-NNN," update cross-references. Do not delete.
- **New invariants may be added.** Use full INV-NNN format.
- **Performance budgets may be revised.** Document whether the budget or design changed, and why.
- **Negative specifications may be added** as LLM implementation reveals hallucination patterns.

### 13.2 Spec Versioning

DDIS recommends a simple versioning scheme: `Major.Minor` where:
- **Major** increments when the formal model or a non-negotiable changes
- **Minor** increments when ADRs, invariants, or implementation chapters are added or revised

### 13.3 ADR Supersession Procedure

When an ADR is superseded (locked by ADR-011), follow this procedure:

**Step 1: Mark the original ADR.**
Add `**Status: SUPERSEDED by ADR-NNN** ([date])` to the original ADR's header. Do NOT delete the original ADR — it is historical record that prevents future teams from re-exploring rejected paths.

**Step 2: Create the new ADR.**
Write the replacement ADR with a fresh identifier (the next sequential ADR-NNN). The new ADR MUST:
- Reference the superseded ADR: `Supersedes: ADR-NNN`
- Include the original decision as a rejected option in the "Options" section, with a WHY NOT annotation explaining what changed since the original decision
- State what new information or implementation experience motivated the supersession

**Step 3: Execute the cross-reference cascade.**
Identify all sections that reference the superseded ADR-NNN:
1. Search the spec for all occurrences of the old ADR identifier
2. For each reference: update to the new ADR identifier, verify the surrounding text is still accurate under the new decision
3. If the new decision changes the behavior prescribed in a section, update the section's content (not just the cross-reference)
4. For modular specs: run `ddis_validate.sh --check-cascade ADR-NNN` (§0.13.12, modularization module) to identify affected modules

**Step 4: Re-validate affected gates.**
After the cascade:
- Gate 2 (Causal Chain): Verify that sections updated in Step 3 still trace to the formal model
- Gate 5 (Cross-Reference Web): Verify the superseded ADR still has at least one inbound reference (the new ADR's "Supersedes" link)

**DO NOT** supersede an ADR without executing the cross-reference cascade — conflicting guidance produces inconsistent implementations. (Validates INV-001, INV-006.)

### Verification Prompt for Chapter 13 (Evolving a DDIS Specification)

After writing your spec's evolution guidance, verify:
1. [ ] The Living state description maintains all invariants and quality gates — evolution does not degrade spec quality (§13.1, INV-001, INV-006)
2. [ ] The ADR supersession procedure includes all four steps: mark, create, cascade, re-validate (§13.3, ADR-011)
3. [ ] Your evolution guidance does NOT treat the Living state as permission for informal changes (§13.1, INV-001)
4. [ ] Your supersession procedure does NOT allow deleting superseded ADRs — they are historical record (§13.3, ADR-011)
5. [ ] *Integration*: Your cascade protocol references the modularization module's §0.13.12 for cross-module impact analysis and the quality gates in §0.7 (constitution) for re-validation (INV-006)

---

# APPENDICES

## Appendix A: Glossary

> **Relationship to constitution glossary**: The constitution's compact glossary provides 1-line declarations for orientation. This appendix provides the full definitions with cross-references. Both must stay synchronized per INV-015 (Declaration-Definition Consistency).

| Term | Definition |
|---|---|
| **ADR** | Architecture Decision Record: structured choice with alternatives and rationale (§3.5) |
| **ADR supersession** | Replacing an ADR while preserving the original as historical record; requires cross-reference cascade (ADR-011, §13.3) |
| **Assembly script** | Tool that reads the manifest and produces bundles by concatenating tiers with module content (§0.13.10) |
| **Blast radius** | Set of modules/invariants affected by a constitutional change; determines cascade scope (§0.13.12) |
| **Bundle** | Assembled document for LLM implementation: Tier 1 + Tier 2 + Tier 3 + Module. The unit of LLM consumption (§0.13.10) |
| **Cascade protocol** | Procedure for re-validating modules affected by constitutional changes (§0.13.12) |
| **Causal chain** | Traceable path from first principle through invariant/ADR to implementation detail (§0.2.3, INV-001) |
| **Churn-magnet** | A decision that causes the most downstream rework if left open; lock via ADRs first (§3.5) |
| **Comparison block** | Side-by-side ❌/✅ comparison with quantified reasoning (§5.5) |
| **Confidence level** | Optional ADR field: Committed (default), Provisional (revisit after spike), or Speculative (needs abstraction boundary) (§3.5) |
| **Constitution** | Cross-cutting material constraining all modules, organized in tiers (§0.13.3) |
| **Context budget** | LLM context window available for spec content after reserving reasoning space (§0.13.9, §0.2.2) |
| **Cross-cutting module** | Module spanning multiple domains (e.g., end-to-end trace) with domain set to "cross-cutting" (§0.13.6) |
| **Cross-reference** | Explicit link using §X.Y, INV-NNN, or ADR-NNN identifiers (Chapter 10, INV-006) |
| **DDIS** | Decision-Driven Implementation Specification — this standard |
| **Declaration** | Compact (1-line) summary of invariant/ADR in system constitution (Tier 1); contrasts with full definition (§0.13.4) |
| **Decision spike** | Time-boxed experiment (1–3 days) that de-risks an unknown and produces an ADR (§6.1.1) |
| **Deep context** | Tier 3: cross-domain invariant definitions and interface contracts needed by a specific module; zero overlap with Tier 2 (§0.13.3) |
| **Definition** | Full specification of invariant/ADR including formal expression, violation scenario, and validation method (§0.13.4) |
| **Design point** | Specific hardware, workload, and scale scenario anchoring performance budgets (§3.7) |
| **Design sketch** | Code block illustrating intent, not compilable. Correctness lives in invariants and tests, not sketch syntax (§2.3) |
| **Domain** | Architectural grouping of modules with tighter internal than external coupling (§0.13.2) |
| **Domain constitution** | Tier 2: full invariant definitions and ADR analysis for one architectural domain (§0.13.3) |
| **End-to-end trace** | Worked scenario traversing all major subsystems with data at each boundary (§5.3, §0.13.6) |
| **Exit criterion** | Specific, testable condition for phase completion (§6.1.2) |
| **Falsifiable** | Can be violated by a concrete scenario and detected by a concrete test (INV-003, ADR-002) |
| **First principles** | Formal model of the problem domain from which architecture derives (§3.3) |
| **Formal model** | Mathematical or pseudo-mathematical definition of the system as a state machine or function (§0.2.1) |
| **Gate** | Quality gate: stop-ship predicate for project progression (§3.6) |
| **Hallucination** | LLM failure: generates plausible but unauthorized behaviors; prevented by negative specifications (§0.2.2, §3.8) |
| **Invariant** | Numbered, falsifiable property that must hold at all times (§3.4) |
| **Invariant registry** | Manifest section listing every invariant with its owning module, enforcing INV-013 (§0.13.9) |
| **Living spec** | Specification in active use, updated as implementation reveals gaps (§13.1) |
| **LLM** | Large Language Model: primary implementer under §0.2.2 constraints (fixed context, no random access, hallucination tendency) |
| **LLM consumption model** | Formal model of how an LLM consumes a DDIS spec, including failure modes and mitigations (§0.2.2) |
| **Manifest** | Machine-readable YAML declaring modules, domains, invariant ownership, and assembly rules (§0.13.9) |
| **Master TODO** | Checkboxable task inventory cross-referenced to subsystems, phases, and ADRs (§7.3) |
| **Meta-instruction** | Directive to LLM implementer: ordering, sequencing, or process guidance (§5.7) |
| **Module** | Self-contained spec unit covering one major subsystem; always assembled into a bundle (§0.13.2, §0.13.5) |
| **Module header** | Structured YAML block declaring domain, maintained invariants, interfaces, and negative specs (§0.13.5) |
| **Monolith** | A DDIS spec as a single document; all specs start as monoliths (§0.13.2) |
| **Negative specification** | Explicit "DO NOT" constraint co-located with the subsystem; primary defense against hallucination (§3.8, INV-017) |
| **Non-goal** | Something the system explicitly does not attempt (§3.2) |
| **Non-negotiable** | Philosophical commitment stronger than an invariant — defines what the system IS (§3.1) |
| **Operational playbook** | How the spec gets converted into shipped software (§6.1) |
| **Partial regression** | Lifecycle transition returning from a later state to Drafted for re-validation of affected sections (§1.1) |
| **Proportional weight** | Line budget guidance preventing section bloat and starvation (§0.8.2) |
| **Quality criteria** | Testable properties an element must exhibit to be well-formed within DDIS (Chapters 2–7) |
| **Reasoning reserve** | Fraction of LLM context window reserved for reasoning (default 0.25). Declared in manifest (§0.13.9) |
| **Ring architecture** | DDIS's structural organization: Core Standard (sacred), Guidance (recommended), Tooling (optional) (§0.4) |
| **Self-bootstrapping** | Property of this standard: written in the format it defines (ADR-004) |
| **Signal-to-noise ratio** | Proportion of content contributing to understanding vs. overhead (INV-007, §0.8.2) |
| **Structural redundancy** | Restating key invariants at point of use to prevent context loss. Required by INV-018 (§0.2.2) |
| **System constitution** | Tier 1: compact declarations of all invariants/ADRs plus system-wide orientation. Always in every bundle (§0.13.3) |
| **Three-tier mode** | Standard modularization: Tier 1 + Tier 2 + Tier 3 + module (§0.13.7, ADR-006) |
| **Two-tier mode** | Simplified modularization for small specs (< 20 invariants): Tier 1 (full definitions) + module (§0.13.7.1) |
| **Verification prompt** | Structured self-check at end of implementation chapter for spec conformance (§5.6, ADR-010) |
| **Verification prompt coverage** | INV-020: every element spec chapter includes a verification prompt block (§5.6) |
| **Voice** | DDIS writing style: technically precise but human (§8.1) |
| **WHY NOT annotation** | Inline comment explaining why a non-obvious alternative was rejected (§5.4) |
| **Worked example** | Concrete scenario with specific values showing a subsystem in action (§5.2) |

---

## Appendix B: Risk Register

| # | Risk | Impact | Mitigation | Detection |
|---|---|---|---|
| 1 | Too prescriptive, authors feel constrained | Low adoption | Non-goals + [Optional] elements provide flexibility | Author feedback; time-to-first-spec comparison |
| 2 | Too verbose, specs become shelfware | Implementers skip the spec | Proportional weight guide limits bloat; voice guide keeps prose readable | Track questions spec should have answered |
| 3 | Cross-reference requirement is burdensome | Authors skip references (INV-006) | Authoring sequence (§11.1) defers cross-refs to step 12 | Reference graph analysis during validation |
| 4 | Self-bootstrapping creates confusion | Meta/object-level ambiguity | Consistent "this standard" vs "a conforming spec" language | Reader feedback on first encounter |
| 5 | No automated validation tooling | Quality gates require manual effort | Completeness checklist (Part X) systematizes manual checks | Track time-to-validate; prioritize if > 2 hours |
| 6 | Negative specs become boilerplate | Generic "DO NOT" with no value | §3.8 (element-specifications module) requires subsystem-specific, falsifiable constraints | LLM hallucination rate with/without (§0.8.4) |
| 7 | LLM provisions add bulk without value | Length exceeds growth budget | INV-007 governs all additions; proportional weight applies | Measure LLM quality with vs without |
| 8 | Declaration-definition drift in modular specs | LLMs code against wrong invariant contracts | Semi-automated comparison check (Gate M-4); review as part of cascade protocol (§0.13.12, modularization module) | INV-015 validation fails; diff between Tier 1 declaration and module definition |
| 9 | Assembly tooling unavailable for validation | Consistency checks (§0.13.11) cannot be automated | Manual validation using CHECK-1 through CHECK-9 as checklists; prioritize tooling | Validation takes > 2 hours manually |

---

## Appendix C: Specification Error Taxonomy

Classification of errors in specification authoring, analogous to §6.3 (element-specifications module) error taxonomy for domain specs. Every DDIS spec should avoid these errors; validation (Chapter 12) should detect them.

| Error Class | Severity | Symptom | Detection | Handling |
|---|---|---|---|
| **Ambiguity** | High | A statement admits multiple valid interpretations | Adversarial review: restate in your own words | Rewrite with concrete values or formal expression |
| **Contradiction** | Critical | Two sections prescribe incompatible behaviors | Cross-reference graph shows conflicting edges | Resolve via ADR; supersede one prescription |
| **Orphan section** | Medium | Section has no inbound or outbound references | Graph analysis (INV-006 check) | Connect to reference web or remove |
| **Unfalsifiable invariant** | High | Invariant has no constructible counterexample | INV-003 check: attempt to construct violation | Sharpen with concrete violation scenario |
| **Strawman ADR** | High | ADR option is not a genuine alternative | Review: "would a competent engineer choose this?" | Replace with genuine alternative or remove option |
| **Missing negative spec** | High | Implementation chapter lacks "DO NOT" constraints | INV-017 check: count negative specs per chapter | Add subsystem-specific negative specifications |
| **Implicit reference** | Medium | Cross-reference uses "see above" instead of §X.Y | Text search for positional references | Replace with explicit identifiers |
| **Aspirational budget** | Medium | Performance claim has no number or measurement | INV-005 check: locate benchmark for each claim | Add number, design point, and measurement method |
| **Context loss** | High | Invariant referenced by ID only, far from definition | INV-018 check: verify restatement at point of use | Restate invariant at point of use |
| **Missing ordering** | Medium | No implementation dependency chain | INV-019 check: locate ordering DAG | Add meta-instructions with dependency reasons |
| **Missing verification prompt** | Medium | Element spec or implementation chapter lacks structured self-check block | INV-020 check: verify prompt block per chapter | Add verification prompt with positive, negative, and integration checks |
| **Superseded ADR without cascade** | High | ADR marked superseded but referencing sections still prescribe old behavior | Audit: search for old ADR-NNN references in non-superseded sections | Execute cross-reference cascade per §13.3 (ADR-011) |

---

## Appendix D: Quick-Reference Card

For experienced DDIS authors who need a reminder, not the full standard:

```
PREAMBLE: Design goal → Core promise → Document note → How to use
PART 0:   Summary (+ Non-negotiables + Non-goals) → First principles (+ LLM consumption model) →
          Document structure → Architecture → Invariants → ADRs → Gates (1-7) →
          Budgets → API surface
PART I:   Formal model → State machines → Complexity → End-to-end trace
PART II:  [Per subsystem: purpose → types → algorithm → state machine →
          invariants (RESTATED) → negative specs (DO NOT) → example →
          edge cases → tests → budget → verification prompt →
          meta-instructions → cross-refs]  (13 components per §5.1, element-specifications module)
          End-to-end trace (crosses all subsystems)
PART III: Protocol schemas → Adapters → UI contracts
PART IV:  Test taxonomy → Error taxonomy → Operational playbook
          (spikes → exit criteria → merge discipline → deliverable order → first PRs)
APPENDICES: Glossary → Risks → Error taxonomy → Quick-reference → Formats → Benchmarks
PART X:   Master TODO (checkboxable, by subsystem)

Every invariant: ID + statement + formal + violation + test + why
Every ADR: problem + options (genuine) + decision + WHY NOT + consequences + tests
Every algorithm: pseudocode + complexity + example + edge cases
Every impl chapter: negative specs (≥3) + verification prompt + invariants RESTATED
Every element spec chapter: verification prompt block (INV-020)
ADR supersession: mark old + create new + cascade cross-refs (ADR-011, §13.3)
Cross-refs: web, not list. No orphan sections. Explicit §X.Y, never "see above."
Voice: senior engineer to respected peer. No hedging. No marketing. No bureaucracy.
LLM provisions: woven throughout, not isolated. Negative specs co-located.
DO NOT constraints: in EVERY element spec, PART III guidance, AND PART IV operations.
```

---

# PART X: MASTER TODO INVENTORY

## A) Meta-Standard Validation
- [x] Self-bootstrapping: this document uses the format it defines
- [x] Preamble elements: design goal, core promise, document note, how to use (with LLM step)
- [x] Non-negotiables defined (§0.1.2) — includes "Negative specifications prevent hallucination"
- [x] Non-goals defined (§0.1.3) — includes LLM model-agnosticism non-goal
- [x] First-principles derivation (§0.2) — includes LLM consumption model (§0.2.2)
- [x] Document structure prescribed (§0.3) — includes negative specs, verification prompts, meta-instructions
- [x] Invariants numbered and falsifiable (§0.5, INV-001 through INV-020)
- [x] ADRs with genuine alternatives (§0.6, ADR-001 through ADR-011)
- [x] Quality gates defined (§0.7) — Gates 1–7 including LLM Implementation Readiness (Gate 7)
- [x] Performance budgets (§0.8 — for spec authoring, not software)
- [x] Proportional weight guide (§0.8.2)
- [x] Specification quality measurement methodology (§0.8.4)

## B) Element Specifications
- [x] Preamble elements specified (Chapter 2) — with LLM-specific "DO NOT" constraints
- [x] PART 0 elements specified (Chapter 3) — including §3.8 Negative Specifications
- [x] PART I elements specified (Chapter 4) — with LLM-specific "DO NOT" for state machines
- [x] PART II elements specified (Chapter 5) — including §5.6 Verification Prompts, §5.7 Meta-Instructions
- [x] PART IV elements specified (Chapter 6) — including LLM conformance test type
- [x] Appendix elements specified (Chapter 7)
- [x] Anti-pattern catalog (§8.3) — including "Afterthought LLM Section" anti-pattern
- [x] Cross-reference patterns (Chapter 10) — with "DO NOT use implicit references"

## C) LLM Provisions
- [x] LLM Consumption Model (§0.2.2) with formal model and failure modes
- [x] INV-017 (Negative Specification Coverage) with violation scenario and validation
- [x] INV-018 (Structural Redundancy at Point of Use) with violation scenario and validation
- [x] INV-019 (Implementation Ordering Explicitness) with violation scenario and validation
- [x] INV-020 (Verification Prompt Coverage) — NEW in 3.0: requires verification prompt blocks in element spec chapters
- [x] ADR-008 (LLM Provisions Woven Throughout) with genuine alternatives
- [x] ADR-009 (Negative Specifications as Formal Elements) with genuine alternatives
- [x] ADR-010 (Verification Prompts per Chapter) with genuine alternatives
- [x] ADR-011 (ADR Supersession Protocol) — NEW in 3.0: formal mark-and-supersede with cross-reference cascade
- [x] Gate 7 (LLM Implementation Readiness) with thought experiment demonstration
- [x] Negative specifications woven throughout element specs (§2.1–§3.7, §4.2, §5.1–§5.7, §7.1, §8.1, §8.3, §10.1, §11.1, §12.1, §13.1)
- [x] §3.8 Negative Specifications element spec with format, quality criteria, and anti-patterns
- [x] §5.6 Verification Prompts element spec with format and self-bootstrapping demo
- [x] §5.7 Meta-Instructions element spec with format, examples, and self-bootstrapping demo

## D) Self-Conformance Fixes
- [x] End-to-end trace for DDIS itself (§1.4)
- [x] State machine (§1.1) with guards, entry actions, complete invalid transition list
- [x] Error taxonomy for specification authoring (Appendix C)
- [x] Specification quality measurement methodology (§0.8.4)
- [x] Verification prompt blocks in all element spec chapters (Chapters 2–7, INV-020)
- [x] INV-018 restatements at point of use within element specs
- [x] ADR supersession protocol formalized (ADR-011, §13.3)

## E) Guidance
- [x] Voice and style guide (Chapter 8) with LLM-specific guidance
- [x] Proportional weight deep dive (Chapter 9)
- [x] Authoring sequence (§11.1) with negative specs, verification prompts, meta-instructions
- [x] Common mistakes (§11.2)
- [x] Validation procedure (Chapter 12) including Gate 7
- [x] Evolution guidance (Chapter 13) including §13.3 ADR Supersession

## F) Reference Material
- [x] Glossary (Appendix A) — all DDIS-specific terms defined
- [x] Risk register (Appendix B) including LLM-specific risks
- [x] Specification error taxonomy (Appendix C)
- [x] Quick-reference card (Appendix D)

## G) Modularization Protocol
- [x] Modularization protocol integrated (§0.13) with 14 subsections
- [x] INV-011 through INV-016 present with violation scenarios and validation methods (INV-020 extended to cover modular element specs)
- [x] ADR-006 (Tiered Constitution) and ADR-007 (Cross-Module References) with genuine alternatives
- [x] Quality gates M-1 through M-5 defined (§0.7)
- [x] Tiered constitution model specified: Tier 1 (declarations), Tier 2 (domain definitions), Tier 3 (cross-domain deep)
- [x] Manifest schema documented with full YAML example (§0.13.9)
- [x] Assembly rules specified for both two-tier and three-tier modes (§0.13.10)
- [x] All 9 consistency checks defined with formal expressions (§0.13.11, CHECK-1 through CHECK-9)
- [x] Cascade protocol documented with and without beads fallback (§0.13.12)
- [x] Migration procedure: monolith to modular, 9 steps (§0.13.13)
- [x] Module header format specified with namespace distinction (§0.13.5)
- [x] Cross-module reference rules formalized (§0.13.6)
- [x] Modularization decision flowchart with two-tier simplification (§0.13.7)
- [ ] Gate M-3 (LLM Bundle Sufficiency): Requires external validation — give 2+ bundles to an LLM
- [ ] Tooling: ddis_assemble.sh implementing §0.13.10
- [ ] Tooling: ddis_validate.sh implementing §0.13.11

## H) External Validation (not yet completed)
- [ ] INV-008 (Self-Containment): Requires external validation — give this standard to a first-time author and track their questions
- [ ] Gate 6 (Implementation Readiness): Requires a non-trivial spec to be written conforming to DDIS
- [ ] Gate 7 (LLM Implementation Readiness): Requires LLM to implement from DDIS-conforming spec chapters

## I) Modular Self-Conformance
- [x] Constitution declarations are genuine summaries, not near-duplicates of module definitions (INV-015)
- [x] Compact glossary trimmed to navigational aid; full glossary in Appendix A is authoritative (INV-015)
- [x] Section Map accuracy validated against current module structure after edits (INV-016) — verified RALPH iter-2: range corrected to §0.1–§0.10, §6.1.1 misattribution fixed
- [x] Cross-module references audited: all go through constitutional elements or Section Map (INV-012) — verified RALPH iter-2: 19 bare refs fixed across 3 modules
- [x] Declaration-definition faithfulness verified: each Tier 1 declaration matches its Tier 2 definition (Gate M-4) — verified RALPH iter-2: 30/31 faithful, INV-017 qualifier added
- [x] Context budget values in constitution match manifest.yaml (INV-016) — verified RALPH iter-2: target=4000, ceiling=5000, reserve=0.25 match exactly

---

## Conclusion

DDIS synthesizes well-established traditions: Architecture Decision Records (Nygard), Design by Contract (Meyer), formal specification (Lamport), game-engine performance budgeting, test-driven development, LLM-era specification practice (negative specs, verification prompts, meta-instructions, structural redundancy — §0.2.2, INV-017 through INV-020, ADR-008 through ADR-011), and living-document evolution (ADR supersession, ADR-011, §13.3).

The result is a specification standard that is:

- **Decision-driven**: Architecture emerges from locked decisions, not assertions
- **Invariant-anchored**: Correctness defined before implementation
- **Falsifiable throughout**: Every claim can be tested
- **LLM-optimized**: Structural provisions prevent hallucination and context loss; verification prompts self-demonstrated in every element spec chapter (Gate 7, INV-020)
- **Self-validating**: Quality gates provide mechanical conformance checking
- **Self-bootstrapping**: This document is both the standard and its first conforming instance

*DDIS: Where rigor meets readability — and specifications become implementations.*

---
module: modularization
domain: modularization
maintains: [INV-011, INV-012, INV-013, INV-014, INV-015, INV-016]
interfaces: [INV-001, INV-006, INV-008, INV-017, INV-018]
implements: [ADR-006, ADR-007]
adjacent: [core-standard]
negative_specs:
  - "Must NOT allow direct cross-module references bypassing the constitution"
  - "Must NOT allow bundles exceeding the hard ceiling"
  - "Must NOT allow invariants to be unowned or multiply-owned"
---

# Modularization Protocol Module

This module contains the DDIS modularization protocol (§0.13) for specs exceeding 2,500 lines or context window limits, its associated invariants, ADRs, and quality gates.

---

## Modularization Invariants

**INV-011: Module Completeness** [Conditional — modular specs only]

*An LLM receiving a properly assembled bundle can implement the module's subsystem without information from any other module's implementation content.*

```
∀ module ∈ modules:
  let bundle = ASSEMBLE(module)
  ∀ implementation_question Q about module's subsystem:
    bundle.answers(Q) ∨ Q.answerable_from(general_competence)
```

Violation scenario: The Scheduler module references EventStore's internal ring buffer layout, but ring buffer details live only in the EventStore module — not in the constitution.

Validation: Give a bundle (not the full spec) to an LLM. Track questions requiring information from another module's implementation. Any such question violates INV-011.

// WHY THIS MATTERS: If module completeness fails, modularization provides no benefit. The value proposition is that bundles are sufficient.

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

// WHY THIS MATTERS: If modules reference each other's internals, bundles need other modules' implementation — defeating modularization. The constitution is the "header file"; modules are "implementation files" never directly included. (Locked by ADR-007.)

// META-STANDARD CARVE-OUT: For meta-standards using two-tier mode with a declaration-only constitution, the Section Map (§0.3) serves as the constitutional routing table for cross-module section references. The pattern "§X.Y, [module-name] module" is permissible because (a) the Section Map is constitutional content mediating the reference, (b) the module name suffix makes the cross-module nature explicit, and (c) the referenced sections are the meta-standard's public API surface. Application specs should still prefer invariant/ADR references over direct section references.

---

**INV-013: Invariant Ownership Uniqueness** [Conditional — modular specs only]

*Every application invariant is maintained by exactly one module (or explicitly by the system constitution). No invariant is unowned or multiply-owned.*

```
∀ inv ∈ invariant_registry:
  (inv.owner = "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 0)
  ∨ (inv.owner ≠ "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 1)
```

Violation scenario: Both EventStore and SnapshotManager list APP-INV-017 in their maintains declarations. Which module's tests are authoritative?

Validation: Mechanical (CHECK-1 in §0.13.11).

// WHY THIS MATTERS: If two modules both claim to maintain an invariant, neither takes full responsibility for its test coverage.

---

**INV-014: Bundle Budget Compliance** [Conditional — modular specs only]

*Every assembled bundle fits within the hard ceiling defined in the manifest's context budget.*

```
∀ module ∈ modules:
  line_count(ASSEMBLE(module)) ≤ context_budget.hard_ceiling_lines
```

Violation scenario: Scheduler module grows to 3,500 lines. With 1,200-line constitutional context, the bundle is 4,700 lines — under the 5,000 hard ceiling but over the 4,000 target (WARN). If the bundle reaches 5,100 lines, INV-014 is violated (ERROR, assembly fails).

Validation: Mechanical (CHECK-5 in §0.13.11). Run the assembly script; it validates budget compliance automatically.

// WHY THIS MATTERS: Budget violations mean modularization added complexity without delivering its benefit.

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

Violation scenario: System constitution declares "APP-INV-017: Event log is append-only" but the Storage domain definition now says "append-only except during compaction." An LLM implementing a different domain codes against the wrong contract.

Validation: Semi-mechanical. Extract declaration/definition pairs; present to reviewer for semantic consistency.

// WHY THIS MATTERS: Divergence between tiers means different modules implement against different understandings of the same invariant. The declaration is the API; the definition is the implementation — they must agree.

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

// WHY THIS MATTERS: A file not in the manifest is invisible to all tooling — assembly, validation, improvement loops, cascade analysis.

---

## Modularization ADRs

### ADR-006: Tiered Constitution over Flat Root [Conditional — modular specs only]

#### Problem

When a DDIS spec is modular for context-window compliance (§0.13), constitutional context must accompany every module bundle. How should this constitutional context be structured?

#### Options

A) **Flat root** — one file containing everything.
- Pros: Simple; one file to maintain; no tier logic.
- Cons: Doesn't scale past ~20 invariants / ~10 ADRs. At scale (25 invariants, 15 ADRs, 4,800 lines), the root alone is ~1,500 lines, leaving only 2,500 for the module.

B) **Two-tier** — system constitution (full definitions) + modules.
- Pros: Simple; works for small modular specs (< 20 invariants, constitution ≤ 400 lines).
- Cons: Constitution grows linearly with invariant count; exceeds budget at medium scale.

C) **Three-tier** — system constitution (declarations only) + domain constitution (full definitions) + cross-domain deep context + module.
- Pros: Scales to large specs; domain grouping already present in well-architected systems (double duty); no duplication between tiers.
- Cons: One additional indirection level; requires domain identification.

#### Decision

**Option C as the full protocol, with Option B as a blessed simplification** for small specs (< 20 invariants, constitution ≤ 400 lines). The `tier_mode` manifest field selects between them — no forced complexity for specs that don't need it, with a clear upgrade path.

// WHY NOT Option A? At scale, the flat root consumes 30–37% of the context budget before the module starts. That's context waste, not management.

#### Consequences

- Authors must identify 2–5 architectural domains when modularizing (usually obvious from architecture overview)
- Two-tier specs migrate to three-tier without restructuring modules (§0.13.13)
- Domain boundaries serve double duty: architectural isolation and context management

#### Tests

- (Validated by INV-014) Bundle budget compliance confirms that the chosen tier mode keeps bundles within ceiling.
- (Validated by INV-011) Module completeness confirms that the constitutional context in each bundle is sufficient.

---

### ADR-007: Cross-Module References Through Constitution Only [Conditional — modular specs only]

#### Problem

When a DDIS spec is modular, how should modules reference content in other modules?

#### Options

A) **Direct references** — "see section 7.3 in the Scheduler module."
- Pros: Natural; mirrors monolithic cross-references.
- Cons: Creates invisible dependencies. Module A's bundle needs Module B — defeating modularization. Violates INV-011.

B) **Through constitution only** — Module A references APP-INV-032 in the constitution, never Module B's internals.
- Pros: Enforces isolation mechanically; bundles are self-contained.
- Cons: Authors must extract all cross-module contracts into the constitution; feels indirect for tightly coupled subsystems.

#### Decision

**Option B: Through constitution only.** INV-012 enforces this mechanically. Cross-module contracts are expressed as invariants or shared types in the constitution, never as references to another module's internals.

// WHY NOT Option A? It breaks INV-011. Module A's bundle would need Module B's implementation — the very thing modularization avoids.

#### Consequences

- All cross-module contracts must be elevated to the constitution
- Modules become truly self-contained; tight coupling becomes visible in the constitution's interface surface

#### Tests

- (Validated by INV-012) Mechanical check (CHECK-7 in §0.13.11) scans modules for direct cross-module references.
- (Validated by INV-011) LLM bundle sufficiency test confirms modules don't need each other's content.

---

## Modularization Quality Gates [Conditional — modular specs only]

In addition to Gates 1–7, modular specs must pass these gates. A failing Gate M-1 makes Gates M-2 through M-5 irrelevant.

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

---

## 0.13 Modularization Protocol [Conditional]

REQUIRED when the monolithic spec exceeds 4,000 lines or when the target context window cannot hold the full spec plus reasoning budget. OPTIONAL but recommended for 2,500–4,000 line specs.

> Namespace note: INV-001 through INV-020 and ADR-001 through ADR-011 are DDIS meta-standard invariants/ADRs (defined in this standard). Application specs using DDIS define their OWN invariant namespace (e.g., APP-INV-001) — never reuse the meta-standard's INV-NNN space. Examples in this section use APP-INV-NNN to demonstrate this convention.

### 0.13.1 The Scaling Problem

When the spec exceeds the LLM's context window, two failure modes emerge:

1. **Truncation**: The LLM silently drops content from the beginning, losing invariants and the formal model.

2. **Naive splitting**: Arbitrary splits break cross-references, orphan invariants, and force guessing at contracts in unseen sections.

The modularization protocol prevents both with principled decomposition and formal completeness guarantees. (Motivated by INV-008, INV-014.)

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

// WHY THREE TIERS? Two tiers work for < 20 invariants / < 10 ADRs. Beyond that, the root exceeds budget. Three tiers add domain grouping — already present in well-architected systems. The domain boundary serves double duty: architectural isolation and context management. See ADR-006.

### 0.13.4 Invariant Declarations vs. Definitions

An invariant has two representations:

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

Key insight: a module's maintained invariants are ALWAYS in its own domain (enforced by CHECK-4). Therefore Tier 2 always covers them; Tier 3 ONLY adds cross-domain content, eliminating duplication. The same pattern applies to ADRs.

### 0.13.5 Module Header (Required per Module)

Every module begins with a structured header that makes the module self-describing. The header uses application-level invariant identifiers (APP-INV-NNN), not the DDIS meta-standard's INV-NNN identifiers.

Two formats are valid:

**YAML frontmatter (preferred)** — machine-parseable, used when tooling consumes the headers. This standard uses this format.

```yaml
---
module: [module-name]
domain: [domain-name]
maintains: [APP-INV-017, APP-INV-018, APP-INV-019]
interfaces: [APP-INV-003, APP-INV-032]
implements: [APP-ADR-003, APP-ADR-011]
adjacent: [EventStore, Scheduler]
negative_specs:
  - "Must NOT directly access TUI rendering state (use event bus)"
  - "Must NOT bypass the reservation system for file writes"
  - "Must NOT assume event ordering beyond the guarantees in APP-INV-017"
---
```

**Comment-style** — for specs without assembly tooling, embedded directly in the document:

```yaml
# Module Header: [Module Name]
# Domain: [Domain Name]
# Maintains: APP-INV-017, APP-INV-018, APP-INV-019
# Interfaces: APP-INV-003 (via EventStore), APP-INV-032 (via Scheduler)
# Negative specs: Must NOT directly access TUI rendering state (use event bus)
```

**DO NOT** omit the `negative_specs` field in the module header — the assembly script needs it to populate subsystem-specific constraints. (Validates INV-011.)

The module header is consumed by:
1. **The assembly script** — to determine what context to include in the bundle (YAML frontmatter preferred)
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

> **Meta-standard note:** When the constitution includes a Section Map (§0.3), the pattern "§X.Y, [module-name] module" is a Section Map-mediated reference, not a direct cross-module reference. The Section Map is constitutional content that routes navigation. See the carve-out note on INV-012 above.

**Rule 2: Shared types are defined in the constitution, not in any module.**

If two modules both use `TaskId` or `EventPayload`, the type definition lives in the domain constitution (Tier 2) or the system constitution (Tier 1), not in either module. Modules reference the type; they don't define it.

**Rule 3: The end-to-end trace is a special module.**

The end-to-end trace (§5.3, element-specifications module) is the one element that legitimately crosses all module boundaries. It is stored as its own module file with a special header:

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
             Does the system constitution (declarations only) + largest module
             fit within the target_lines budget?
             |-- Yes -> TWO-TIER (see §0.13.7.1)
             +-- No  -> Does system have natural domain boundaries?
                        |-- Yes -> THREE-TIER (standard protocol)
                        +-- No  -> Refactor architecture to create domain
                                   boundaries, then THREE-TIER
```

**Self-bootstrapping demonstration** (comparison block per §5.5 format, element-specifications module):
```
// SUBOPTIMAL: Two-tier for large specs — constitution ~1,500 lines, module budget ~3,500 lines
// CHOSEN: Three-tier with domain grouping (ADR-006) — Tier 1 ~350 lines, Tier 2 ~400 lines,
//   module budget ~4,250 lines (21% more capacity). See ADR-006 for full analysis.
```

#### 0.13.7.1 Two-Tier Simplification

For modular specs where bundles fit within the target budget, the domain tier can be skipped. Two-tier mode has two variants:

**Variant A: Full-definition constitution.** For specs with few invariants/ADRs (< 20 total):
- **Tier 1 (System Constitution)**: Contains BOTH declarations AND full definitions for all invariants and ADRs (since there are few enough to fit in ≤ 400 lines).

**Variant B: Declaration-only constitution.** For specs with more invariants/ADRs where full definitions live in the owning modules:
- **Tier 1 (System Constitution)**: Contains declarations only (ID + 1-line statement + owner). Full definitions are in the modules that maintain each invariant/ADR. This works when the constitution + largest module fits within `target_lines`.

In both variants:
- **Tier 2 (Domain Constitution)**: SKIPPED. Does not exist in the file layout.
- **Tier 3 (Cross-Domain Deep)**: SKIPPED. Not needed because either Tier 1 has full definitions (Variant A) or modules are self-contained with their own full definitions (Variant B).
- **Module**: Unchanged.

Assembly in two-tier mode: `system_constitution + module → bundle`.

**Self-bootstrapping note**: This standard uses Variant B (31 items exceed Variant A threshold). All bundles are 1,144-1,442 lines with the 524-line declaration-only constitution -- well within the 4,000-line target. (Validates ADR-006, ADR-004.)

The manifest uses `tier_mode: two-tier` to signal this to the assembly script. If the spec grows beyond the two-tier threshold (constitution + largest module > `target_lines`), migrate to three-tier by extracting domain constitutions (see Migration Procedure §0.13.13).

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
ddis_version: "3.0"
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

**DO NOT** define modules without the `maintains` field — unowned invariants leave nobody responsible for testing. (Validates INV-013.)

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

**Complexity**: Assembly is O(T) file reads per module (T = number of tiers, at most 3). Full rebuild is O(M × T) where M = number of modules. Space is O(max_bundle_size) per assembly. For a typical spec with 8 modules and 3 tiers, full assembly reads ≤ 32 files.

**Worked example**: Assembling the `scheduler` module in three-tier mode:
- Tier 1: `constitution/system.md` — 350 lines
- Tier 2: `constitution/domains/coordination.md` — 400 lines
- Tier 3: `deep/scheduler.md` — 200 lines
- Module: `modules/scheduler.md` — 2,500 lines
- **Total**: 3,450 lines (under 4,000 target, under 5,000 ceiling)

**Edge cases**:
- **Missing file**: If any referenced path (constitution, domain, deep context, or module) does not exist, ABORT with an error naming the missing file and the manifest entry that references it. **DO NOT** silently skip missing files — a missing file is a manifest-spec synchronization failure (INV-016). (Detected by CHECK-9.)
- **Empty module**: If a module file exists but contains 0 lines, WARN and produce a minimal bundle (constitution only). The author likely forgot to populate the module.
- **Missing deep context**: If `deep_context` is non-null but the file does not exist, ABORT. This is a manifest-spec synchronization failure. (Detected by CHECK-9.)
- **Manifest schema violation**: If a module entry lacks required fields (`file`, `domain`, `maintains`), ABORT before assembly with a validation error listing the missing fields.

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

#### Worked Examples for CHECK-1 through CHECK-9

Minimal manifest: two modules (event_store, scheduler), three invariants (APP-INV-001 through APP-INV-003).

```
CHECK-1/6 (Ownership):
  ✓ APP-INV-001 owner=event_store, event_store.maintains=[APP-INV-001] → one owner, referenced
  ✗ Both event_store and scheduler maintain APP-INV-001 → duplicate owner. Fix: remove from one.
CHECK-2 (Interface consistency):
  ✓ scheduler.interfaces=[APP-INV-001], event_store.maintains=[APP-INV-001]
  ✗ scheduler.interfaces=[APP-INV-099], no module maintains it. Fix: assign owner.
CHECK-3 (Adjacency symmetry):
  ✓ event_store.adjacent=[scheduler], scheduler.adjacent=[event_store] → bidirectional
  ✗ event_store.adjacent=[scheduler], scheduler.adjacent=[] → asymmetric. Fix: add reciprocal.
CHECK-4 (Domain membership):
  ✓ scheduler.domain=orchestration, APP-INV-002.domain=orchestration → match
  ✗ scheduler.domain=orchestration maintains APP-INV-001.domain=storage → mismatch. Fix: reassign.
CHECK-5 (Budget compliance):
  ✓ ASSEMBLE(scheduler) = 3,800 lines, ceiling = 5,000
  ✗ ASSEMBLE(event_store) = 5,200 lines > ceiling. Fix: split module.
CHECK-7 (Cross-module isolation):
  ✓ scheduler.md references "APP-INV-001" (constitutional)
  ✗ scheduler.md references "event_store §3.2" directly. Fix: extract to constitution.
CHECK-8 (Deep context):
  ✓ scheduler has cross-domain interface and deep_context file
  ✗ Same scenario but deep_context=null. Fix: create deep context file.
CHECK-9 (File existence):
  ✓ manifest references "modules/scheduler.md", file exists
  ✗ manifest references "modules/cache.md", file missing. Fix: create or remove from manifest.
```

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

**Complexity**: Cascade identification is O(I x M) where I = changed invariants/ADRs and M = modules. Re-validation is O(A x V) where A = affected modules and V = per-module validation cost. Both workflows query the same manifest; beads adds persistence and ordering. If a cascaded change triggers further constitutional changes, repeat the cascade (should not recurse more than once since modules cannot modify the constitution).

### 0.13.13 Monolith-to-Module Migration Procedure

**Step 1: Identify domains.**
Group PART II chapters into 2–5 domains based on architectural boundaries.

**Step 2: Extract system constitution.**
From monolith to `constitution/system.md`: preamble, PART 0 sections, all invariant DECLARATIONS, all ADR DECLARATIONS, glossary (1-line definitions), quality gates, non-negotiables, non-goals.

**Step 3: Extract domain constitutions.**
For each domain to `constitution/domains/{domain}.md`: domain formal model, full invariant definitions owned by domain, full ADR analysis decided in domain, cross-domain interface contracts, domain performance budgets.

**Step 4: Extract modules.**
For each PART II chapter to `modules/{subsystem}.md`: add module header (§0.13.5), include implementation content, convert cross-module direct references to constitutional references (hardest step — see INV-012).

**DO NOT** defer cross-module reference conversion — convert during extraction while the monolith is still available for context. Deferring produces bundles that violate INV-012.

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

### Verification Prompt for Modularization Protocol (§0.13)

After modularizing your spec, verify:

1. [ ] Every module header declares maintained invariants, interfaces, and negative specs (§0.13.5, INV-011)
2. [ ] Cross-module references go through constitutional elements, never direct module internals (INV-012, ADR-007)
3. [ ] Every invariant is maintained by exactly one module (INV-013, CHECK-1)
4. [ ] Every assembled bundle fits within the hard ceiling (INV-014, CHECK-5)
5. [ ] Constitution declarations faithfully summarize module definitions (INV-015, Gate M-4)
6. [ ] Your modularization does NOT have modules referencing each other's internal sections (INV-012)
7. [ ] Your manifest does NOT have orphan invariants or missing file paths (CHECK-6, CHECK-9)
8. [ ] *Integration*: Your cascade protocol correctly identifies affected modules for a simulated invariant change (Gate M-5)
