# DDIS: Decision-Driven Implementation Specification Standard

## Version 3.1 — A Self-Bootstrapping Meta-Specification

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
> 5) Treat the **cross-reference web** as a product requirement, not polish.
> 6) If your spec exceeds **2,500 lines** or your target LLM's context window, decompose per **§0.13 (Modularization Protocol)**.
> 7) If the primary implementer is an **LLM**, ensure every implementation chapter includes negative specifications (§3.8), verification prompts (§5.6, INV-020), and meta-instructions (§5.7).

---

# PART 0: EXECUTIVE BLUEPRINT

## 0.1 Executive Summary

DDIS (Decision-Driven Implementation Specification) is a standard for writing technical specifications that bridge architectural vision and correct implementation. The primary optimization target is **LLM consumption**: the primary implementer will be a large language model.

Most specifications fail in two ways: too abstract (the implementer guesses at critical details) or too mechanical (they prescribe code without explaining why, making evolution impossible). DDIS avoids both by requiring a **causal chain** from first principles through decisions to implementation details, where every element justifies its existence by serving the elements around it.

When the implementer is an LLM, a third failure mode emerges: the LLM **hallucinates** plausible details not in the spec, or **forgets** invariants defined far from the implementation section. DDIS addresses this with structural provisions woven throughout: negative specifications (§3.8), structural redundancy at point of use (INV-018), verification prompts (§5.6), and meta-instructions (§5.7). These are integral to every element specification, not an add-on. (Locked by ADR-008.)

DDIS synthesizes Architecture Decision Records (Nygard), Design by Contract (Meyer), temporal invariants (Lamport), performance budgeting (game engine development), and test-driven specification into a unified document structure.

### 0.1.1 What DDIS Is

DDIS is a domain-agnostic document standard specifying: what structural elements a specification must contain, how those elements relate (the cross-reference web), what quality criteria each must meet, how to validate completeness, and how to structure elements for optimal LLM consumption (§0.2.2).

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
  ADRs reference invariants. Invariants reference tests. Tests reference performance budgets. A specification where sections exist in isolation is a collection of essays, not a DDIS spec.

- **The document is self-contained**
  A competent implementer with the spec and the spec alone can build a correct v1. If they cannot, the spec has failed.

- **Negative specifications prevent hallucination**
  Every implementation chapter states what the subsystem must NOT do, not merely what it must do. This is the primary defense against LLM hallucination and human assumption. (See §3.8, INV-017.)

### 0.1.3 Non-Goals (Explicit)

DDIS does not attempt:

- **To replace code.** A spec describes what to build, why, and how to verify it — not the literal source code.
- **To eliminate judgment.** Implementers make thousands of micro-decisions. DDIS constrains macro-decisions so micro-decisions are locally safe.
- **To be a project management framework.** The Master TODO and phased roadmap are execution aids, not a substitute for sprint planning.
- **To prescribe notation.** DDIS requires formal models but does not mandate TLA+, Alloy, Z, or any specific formalism.
- **To guarantee correctness.** A DDIS-conforming spec dramatically reduces the chance of building the wrong thing but cannot eliminate it.
- **To optimize for a specific LLM.** DDIS provisions target structural properties benefiting all transformer-based models, not prompt-engineering tricks for a particular model family.

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

2. **Decisions over descriptions.** The hardest part of building a system is the hundreds of design decisions that determine correctness. A spec that describes without recording why is a snapshot, not a blueprint.

3. **Verifiability over trust.** Every claim must be testable. "The system is fast" is not verifiable. "Event ingestion completes in < 100us p99 at the design point of 300 agents / 10K tasks, measured by Benchmark B-001" is verifiable.

### 0.2.2 LLM Consumption Model

An LLM consuming a DDIS spec operates under constraints fundamentally different from a human reader. This model justifies INV-017–020, ADR-008–011, and Gate 7.

| Constraint | Failure Mode | DDIS Mitigation |
|---|---|---|
| Fixed context window | Spec competes with reasoning for token budget | Modularization (§0.13); proportional weight (§0.8.2) |
| No random access | Cannot "flip back" to check a definition | Structural redundancy at point of use (INV-018) |
| Hallucination tendency | Fills gaps with plausible but incorrect details | Negative specifications per subsystem (§3.8, INV-017) |
| Example over-indexing | Treats worked examples as authoritative templates | Quality bar for examples higher than human specs; anti-patterns mandatory |
| Implicit reference failure | "See above" resolves to wrong or lost context | All cross-refs use explicit §X.Y, INV-NNN, ADR-NNN identifiers (INV-006) |
| No clarification channel | Cannot ask the architect mid-implementation | Self-containment at chapter granularity (INV-008) |
| Instruction-following | Can execute embedded directives | Verification prompts (§5.6) and meta-instructions (§5.7) |

**Formal model:**
```
LLM_Implement: (Spec_Fragment, Context_Budget) → Implementation
where Correctness = f(completeness, absence_of_hallucination_triggers,
                       negative_constraints, structural_redundancy)
  hallucination_triggers = {
    gap: ∃ Q: ¬Spec_Fragment.answers(Q) ∧ Q.is_architectural,
    ambiguity: ∃ S: |interpretations(S)| > 1,
    implicit_ref: ∃ R: R.target ∉ Spec_Fragment }
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

Every specification performs these operations:

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
| **Sequence** | Order the work | Phased roadmap, meta-instructions (§5.7), decision spikes |
| **Lexicon** | Define terminology | Glossary |

## 0.3 Document Structure (Required)

A DDIS-conforming specification must contain the following structure. Sections may be renamed to fit the domain but the structural elements are mandatory unless explicitly marked [Optional].

> Note: Non-negotiables and non-goals may be placed within the Executive Summary (as this meta-standard does) or as separate §0.11/§0.12 sections.

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
  Specification composition properties (how specs compose)
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
  E: Modularization Details (file layout, manifest, assembly)  [Conditional]
  F: Storage / Wire Formats                                [Optional]
  G: Benchmark Scenarios                                   [Optional]
  H: Reference Implementations / Extracted Code            [Optional]

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

> **META-INSTRUCTION (for LLM implementers):** When implementing from a DDIS spec, read PART 0 in full before beginning any implementation chapter. Do not skip the invariants or ADRs — they constrain every decision you will make. When implementing a specific subsystem, re-read the invariants listed in that chapter's header before writing code.

## 0.4 This Standard's Architecture

DDIS has a simple ring architecture:

1. **Core Standard (sacred)**: The mandatory structural elements, their required contents, quality criteria, and relationships. (PART 0, PART I, PART II of this document.)

2. **Guidance (recommended)**: Voice, proportional weight, anti-patterns, worked examples. These improve spec quality but their absence does not make a spec non-conforming. (PART III of this document.)

3. **Tooling (optional)**: Checklists, templates, validation procedures. (PART IV, Appendices.)

## 0.5 Invariants of the DDIS Standard

Every DDIS-conforming specification must satisfy these invariants. Each has an identifier, plain-language statement, formal expression, violation scenario, validation method, and WHY THIS MATTERS annotation.

**INV-001: Causal Traceability** — *Every implementation section traces to at least one ADR or invariant, which traces to the formal model.*
```
∀ section ∈ PART_II:
  ∃ adr ∈ ADRs ∪ inv ∈ Invariants:
    section.references(adr ∨ inv) ∧ (adr ∨ inv).derives_from(formal_model)
```
Violation: An implementation chapter describes a caching layer with no ADR and no invariant it preserves.
Validation: Pick 5 random implementation sections; follow cross-references to the formal model. Any broken chain violates.
// WHY THIS MATTERS: Unjustified sections accumulate and cannot be safely removed.

---

**INV-002: Decision Completeness** — *Every design choice where a reasonable alternative exists is captured in an ADR.*
```
∀ choice ∈ spec where ∃ alternative ∧ alternative.is_reasonable:
  ∃ adr ∈ ADRs: adr.covers(choice) ∧ adr.alternatives.contains(alternative)
```
Violation: Spec prescribes advisory locking but never records why mandatory locking was rejected. A team member re-implements with mandatory locks, causing deadlocks.
Validation: For each implementation section, ask "could this reasonably be done differently?" If yes and no ADR exists, violated.
// WHY THIS MATTERS: Undocumented decisions get relitigated at the same cost with no added value.

---

**INV-003: Invariant Falsifiability** — *Every invariant can be violated by a concrete scenario and detected by a named test.*
```
∀ inv ∈ Invariants:
  ∃ scenario: scenario.violates(inv) ∧ ∃ test ∈ TestStrategy: test.detects(scenario)
```
Violation: "The system shall be performant" — no concrete scenario can violate this because "performant" is undefined.
Validation: Construct a counterexample for each invariant. If impossible, the invariant is trivially true or too vague.
// WHY THIS MATTERS: Unfalsifiable invariants look like safety properties but prevent nothing.

---

**INV-004: Algorithm Completeness** — *Every algorithm includes pseudocode, complexity analysis, at least one worked example, and error/edge case handling.*
```
∀ algorithm ∈ spec:
  algorithm.has(pseudocode) ∧ algorithm.has(complexity_analysis) ∧
  algorithm.has(worked_example) ∧ algorithm.has(edge_cases)
```
Violation: "Conflict resolution algorithm" described in prose only. The LLM invents its own, handling the happy path but failing on concurrent modifications.
Validation: Scan each algorithm section for the four required components.
// WHY THIS MATTERS: Prose algorithm descriptions are ambiguous; LLMs fill ambiguity with plausible but incorrect logic.

---

**INV-005: Performance Verifiability** — *Every performance claim is tied to a benchmark, a design point, and a measurement methodology.*
```
∀ perf_claim ∈ spec:
  ∃ benchmark: perf_claim.measured_by(benchmark) ∧
  ∃ design_point: perf_claim.valid_at(design_point) ∧ benchmark.has(methodology)
```
Violation: "Sub-millisecond dispatch" claimed without benchmark, design point, or measurement method.
Validation: For each performance number, locate the benchmark. If absent, violated.
// WHY THIS MATTERS: Performance claims without measurement methodology are wishes, not contracts.

---

**INV-006: Cross-Reference Density** — *Every non-trivial section has inbound and outbound references.*
```
∀ section ∈ spec (excluding Preamble, Glossary):
  section.outgoing_references.count ≥ 1 ∧ section.incoming_references.count ≥ 1
```
Violation: A "Security Considerations" section references nothing and nothing references it.
Validation: Build a cross-reference graph. Every non-trivial section needs >= 1 inbound and >= 1 outbound edge.
// WHY THIS MATTERS: For LLMs, explicit identifiers (§X.Y, INV-NNN) are the ONLY navigation mechanism.

---

**INV-007: Signal-to-Noise Ratio** — *Every section earns its place by serving at least one other section or preventing a named failure mode.*
```
∀ section ∈ spec:
  ∃ justification: (section.serves(other_section) ∨ section.prevents(named_failure_mode))
```
Violation: A 200-line "History of the Project" section that serves no other section and prevents no failure.
Validation: For each section, state why removing it would make the spec worse. If you cannot, remove the section.
// WHY THIS MATTERS: Every line competes for the reader's attention (human) or context window (LLM). Noise displaces signal.

---

**INV-008: Self-Containment** — *The spec plus the implementer's general competence is sufficient to build a correct v1.*
```
∀ implementation_question Q:
  spec.answers(Q) ∨ Q.answerable_from(general_competence ∪ public_references)
```
Violation: Spec references "the standard retry algorithm" without specifying which. LLM picks exponential backoff; the use case requires jittered retry.
Validation: Give the spec to an unfamiliar competent engineer. Track every question revealing missing info.
// WHY THIS MATTERS: An LLM cannot ask clarifying questions. Every gap becomes a hallucination site.

---

**INV-009: Glossary Coverage** — *Every domain-specific term is defined in the glossary.*
```
∀ term ∈ spec where term.is_domain_specific:
  ∃ entry ∈ Glossary: entry.defines(term)
```
Violation: Spec uses "reservation" (meaning advisory file lock) without defining it. LLM builds a booking system.
Validation: Extract all non-common-English terms; check each against the glossary.
// WHY THIS MATTERS: LLMs default to the most common meaning. Domain-specific overloads MUST be defined explicitly.

---

**INV-010: State Machine Completeness** — *Every state machine defines all states, all transitions, all guards, and behavior for invalid transitions.*
```
∀ sm ∈ StateMachines:
  sm.has(all_states) ∧ sm.has(all_transitions) ∧
  sm.has(guards_per_transition) ∧ sm.has(invalid_transition_policy)
```
Violation: A task state machine defines {Pending, InProgress, Done} but omits what happens when "complete" arrives for an already-Done task.
Validation: Enumerate the state x event cross-product. Every cell must name a transition or explicitly state "invalid — [policy]."
// WHY THIS MATTERS: Incomplete state machines are the most common source of bugs in event-driven systems. LLMs implement only happy-path transitions unless told otherwise.

---

**INV-011: Module Completeness** [Conditional] — *An LLM receiving a properly assembled bundle can implement the module's subsystem without information from any other module.*
```
∀ module ∈ modules:
  let bundle = ASSEMBLE(module)
  ∀ question Q about module's subsystem:
    bundle.answers(Q) ∨ Q.answerable_from(general_competence)
```
Violation: Scheduler module references EventStore's ring buffer layout, but those details live only in EventStore's module.
Validation: Give a bundle (not the full spec) to an LLM. Any question requiring another module's content violates INV-011.
// WHY THIS MATTERS: If module completeness fails, modularization provides no benefit.

---

**INV-012: Cross-Module Isolation** [Conditional] — *Modules reference each other only through constitutional elements, never another module's internals.*
```
∀ module_a, module_b ∈ modules where module_a ≠ module_b:
  ∀ ref ∈ module_a.outbound_references:
    ref.target ∉ module_b.internal_sections ∧
    ref.target ∈ {constitution, shared_types, invariants, ADRs}
```
Violation: TUI Renderer says "use the same batching strategy as EventStore's flush_batch()."
Validation: Mechanical scan for direct cross-module references.
// WHY THIS MATTERS: Cross-module internal references force bundles to include other modules' implementation. (Locked by ADR-007.)

---

**INV-013: Invariant Ownership Uniqueness** [Conditional] — *Every application invariant is maintained by exactly one module (or the system constitution).*
```
∀ inv ∈ invariant_registry:
  (inv.owner = "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 0)
  ∨ (inv.owner ≠ "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 1)
```
Violation: Both EventStore and SnapshotManager list APP-INV-017. Which module's tests are authoritative?
Validation: Mechanical check of manifest declarations.
// WHY THIS MATTERS: Dual ownership means neither module takes full responsibility.

---

**INV-014: Bundle Budget Compliance** [Conditional] — *Every assembled bundle fits within the manifest's hard ceiling.*
```
∀ module ∈ modules:
  line_count(ASSEMBLE(module)) ≤ context_budget.hard_ceiling_lines
```
Violation: Scheduler + 1,200-line constitution = 4,700 lines, over target but under 5,000 ceiling. Growth to 5,100 violates.
Validation: Mechanical — assembly validates budget compliance automatically.
// WHY THIS MATTERS: Budget violations mean modularization added complexity without delivering benefit.

---

**INV-015: Declaration-Definition Consistency** [Conditional] — *Every system constitution declaration faithfully summarizes its domain constitution definition.*
```
∀ inv ∈ invariant_registry:
  let decl = system_constitution.declaration(inv)
  let defn = full_definition(inv)
  decl.id = defn.id ∧ decl.one_line is_faithful_summary_of defn.statement
```
Violation: System declares "APP-INV-017: Event log is append-only" but domain definition says "append-only except during compaction."
Validation: Semi-mechanical — extract declaration/definition pairs; review for semantic consistency.
// WHY THIS MATTERS: Tier divergence means modules implement against different understandings of the same invariant.

---

**INV-016: Manifest-Spec Synchronization** [Conditional] — *The manifest accurately reflects all spec files.*
```
∀ path ∈ manifest.all_referenced_paths: file_exists(path)
∀ inv ∈ manifest.all_referenced_invariants: inv ∈ system_constitution
∀ module_file ∈ filesystem("modules/"): module_file ∈ manifest
```
Violation: Author adds `modules/new_feature.md` but not the manifest. Assembly never bundles it.
Validation: Mechanical — manifest paths vs. filesystem.
// WHY THIS MATTERS: A file not in the manifest is invisible to all tooling.

---

**INV-017: Negative Specification Coverage** — *Every implementation chapter includes >= 3 "DO NOT" constraints targeting likely hallucination patterns.*
```
∀ chapter ∈ PART_II_chapters:
  chapter.has(negative_specifications) ∧ chapter.negative_specifications.count ≥ 3
```
Violation: Scheduler chapter omits "DO NOT use blocking locks." LLM adds mutex-based priority, deadlocking under load.
Validation: Verify >= 3 negative specs per chapter, each addressing a plausible LLM hallucination.
// WHY THIS MATTERS: LLMs fill gaps with plausible behavior. Negative specs prevent hallucination before it occurs. (Locked by ADR-009.)

---

**INV-018: Structural Redundancy at Point of Use** — *Every implementation chapter restates the invariants it preserves, not merely listing IDs.*
```
∀ chapter ∈ PART_II_chapters:
  ∀ inv ∈ chapter.preserved_invariants:
    chapter.contains(inv.id) ∧ chapter.contains(inv.one_line_statement ∨ inv.full_statement)
```
Violation: Chapter says "Preserves: INV-003, INV-017, INV-018" without restating them. LLM, 2,000 lines past the definitions, violates INV-017 unknowingly.
Validation: Verify preserved invariants are restated (minimum: ID + one-line statement). Bare ID lists violate.
// WHY THIS MATTERS: An invariant reference 2,000 lines from its definition is functionally invisible to an LLM.

---

**INV-019: Implementation Ordering Explicitness** — *The spec provides an explicit dependency DAG for implementation ordering.*
```
∃ ordering ∈ spec:
  ordering.is_dag ∧
  ∀ (a, b) ∈ ordering.edges: ∃ reason: a.must_precede(b).because(reason)
```
Violation: Five subsystems with no ordering. LLM builds UI first, then discovers it depends on a nonexistent data model.
Validation: Verify the ordering is a DAG with stated reasons for each dependency edge.
// WHY THIS MATTERS: LLMs implement in encounter order. Explicit ordering prevents cascading rework. (See §5.7.)

---

**INV-020: Verification Prompt Coverage** — *Every element specification chapter includes a verification prompt block with positive and negative checks.*
```
∀ chapter ∈ element_specification_chapters:
  chapter.has(verification_prompt_block) ∧
  chapter.verification_prompt_block.has(positive_check) ∧
  chapter.verification_prompt_block.has(negative_check)
```
Violation: DDIS prescribes verification prompts (§5.6) but its own chapters lack them — no self-bootstrapping demonstration.
Validation: For each element spec chapter (Chapters 2–7), verify a prompt block with positive and negative checks.
// WHY THIS MATTERS: Self-bootstrapping (ADR-004) requires demonstrating every prescribed element. (Locked by ADR-010.)

---

## 0.6 Architecture Decision Records

### ADR-001: Document Structure Is Fixed, Not Flexible

**Problem**: Should DDIS prescribe a fixed document structure, or allow free organization?

**Options**: A) **Fixed structure** — predictable, mechanically checkable, LLMs benefit (§0.2.2); may feel rigid. B) **Content requirements only** — flexible but unpredictable, harder to validate. C) **Fixed skeleton, flexible interior** — balance, but often degenerates.

**Decision**: **Option A.** A reader who has seen one DDIS spec can navigate any other. Sections may be renamed; required elements (§0.3) must appear and PART ordering preserved.
// WHY NOT B, C? Unpredictable structure forces re-learning and prevents mechanical validation.

**Consequences**: Authors determine where domain concepts fit; readers gain predictability; validation mechanical.
**Tests**: (Validated by INV-001, INV-006.)

---

### ADR-002: Invariants Must Be Falsifiable, Not Merely True

**Problem**: Should invariants be aspirational properties or formal contracts with violation scenarios?

**Options**: A) **Aspirational** — easy to write; cannot be tested. B) **Formal with proof obligations** (TLA+-style) — machine-checkable but requires formal methods expertise. C) **Falsifiable** — each has a concrete counterexample and test; readable by engineers and LLMs.

**Decision**: **Option C.** Every invariant must include: plain-language statement, semi-formal expression, violation scenario, and validation method.
// WHY NOT B? The goal is implementation correctness, not machine-checked proofs.

**Consequences**: Invariants immediately actionable as test cases; violation scenarios force adversarial thinking.
**Tests**: (Validated by INV-003.)

---

### ADR-003: Cross-References Are Mandatory, Not Optional Polish

**Problem**: Should cross-references be recommended or required?

**Options**: A) **Recommended**. B) **Required** — every non-trivial section must have inbound and outbound references using explicit identifiers.

**Decision**: **Option B.** Cross-references transform sections into a unified spec. For LLMs, explicit identifiers (§X.Y, INV-NNN, ADR-NNN) are the ONLY reliable navigation (§0.2.2).
// WHY NOT A? Recommended means optional. Optional means absent.

**Consequences**: Higher authoring cost; much higher reader value; enables graph-based validation.
**Tests**: (Validated by INV-006.)

---

### ADR-004: Self-Bootstrapping as Validation Strategy

**Problem**: How do we validate that the DDIS standard itself is coherent?

**Options**: A) **External validation** — write in prose, validate by review. B) **Self-bootstrapping** — write in its own format, validate by self-conformance.

**Decision**: **Option B.** This document is both the standard and its first conforming instance.
// WHY NOT A? A standard that cannot be applied to itself is suspect.

**Consequences**: More trustworthy but more complex; document serves as both reference and example.
**Tests**: This document passes its own Quality Gates (§0.7) and Completeness Checklist (Part X).

---

### ADR-005: Voice Is Specified, Not Left to Author Preference

**Problem**: Should DDIS prescribe the writing voice?

**Options**: A) **No voice guidance**. B) **Voice guidance** — specify tone, provide examples, define anti-patterns.

**Decision**: **Option B.** Specs fail when too dry or too casual. DDIS prescribes: technically precise but human — the voice of a senior engineer talking to a peer. For LLMs, explicit voice guidance reduces generic boilerplate.

**Consequences**: Unified, readable specs; authors may revise natural habits.
**Tests**: Sample 5 sections; each sounds like a senior engineer talking to a peer.

---

### ADR-006: Tiered Constitution over Flat Root [Conditional — modular specs only]

**Problem**: How should constitutional context for modular bundles be structured?

**Options**: A) **Flat root** — simple but doesn't scale past ~20 invariants (root alone ~1,500 lines at scale). B) **Two-tier** — system constitution + modules; works for small specs. C) **Three-tier** — system (declarations) + domain (definitions) + cross-domain deep + module; scales with domain grouping.

**Decision**: **Option C as full protocol, Option B as blessed simplification** for small specs. Manifest `tier_mode` selects.
// WHY NOT A? At scale, flat root consumes 30-37% of context budget before the module starts.

**Consequences**: Authors identify 2-5 domains; two-tier migrates to three-tier without restructuring modules.
**Tests**: (Validated by INV-014, INV-011.)

---

### ADR-007: Cross-Module References Through Constitution Only [Conditional — modular specs only]

**Problem**: How should modules reference content in other modules?

**Options**: A) **Direct references** — natural but defeats modularization (violates INV-011). B) **Through constitution only** — enforces isolation mechanically.

**Decision**: **Option B.** INV-012 enforces mechanically. Cross-module contracts expressed as invariants or shared types in the constitution.
// WHY NOT A? Breaks INV-011. Module A's bundle would need Module B's implementation.

**Consequences**: All cross-module contracts elevated to constitution; tight coupling becomes visible.
**Tests**: (Validated by INV-012, INV-011.)

---

### ADR-008: LLM Provisions Woven Throughout, Not Isolated

**Problem**: How should LLM consumption provisions be integrated?

**Options**: A) **Isolated chapter** — easy to find but distant from point of use (exactly §0.2.2 failure). B) **Woven throughout** — in each element specification; guidance at point of use per INV-018. C) **Dual** — woven plus summary; divergence risk.

**Decision**: **Option B.** Every element specification includes LLM-specific notes. Quick-Reference Card provides summary.
// WHY NOT A? Suffers the exact failure mode DDIS prevents.

**Consequences**: Authors encounter LLM considerations naturally; ~10% longer but at maximum impact.
**Tests**: (Validated by INV-017, INV-018.)

---

### ADR-009: Negative Specifications as Formal Elements

**Problem**: How should "what the system must NOT do" be captured?

**Options**: A) **Anti-patterns only** — works for humans but LLMs need co-located constraints. B) **Formal negative specification blocks** — required per chapter, co-located with subsystem. C) **Separate chapter** — easy to audit but same distance-from-use problem.

**Decision**: **Option B.** Required structural elements (INV-017), specified in §3.8, demonstrated throughout this document.
// WHY NOT A, C? LLMs need imperative, co-located constraints — not examples 500 lines away.

**Consequences**: Every chapter gains 3-8 negative specs; anti-pattern catalogs remain as document-level guidance.
**Tests**: (Validated by INV-017.)

---

### ADR-010: Verification Prompts per Implementation Chapter

**Problem**: Is there value in self-checks beyond post-implementation test strategies?

**Options**: A) **Test strategies only**. B) **Verification prompts per chapter** — catches misunderstandings before code is written; LLMs execute as part of flow. C) **Single end-of-document checklist** — too distant and generic.

**Decision**: **Option B.** Each chapter ends with positive and negative checks referencing specific invariants.
// WHY NOT A? Tests catch bugs; prompts catch misunderstandings. Different failure modes.

**Consequences**: Each chapter gains ~5-8 lines; LLMs use as self-checks; humans as review checklists.
**Tests**: (Validated by Gate 7, INV-020.)

---

### ADR-011: ADR Supersession Protocol

**Problem**: When a Living spec supersedes an ADR, referencing sections may prescribe incompatible behavior.

**Options**: A) **Delete-and-replace** — clean but loses reasoning history. B) **Mark-and-supersede with cascade** — preserve old, create new, cascade-update all references. C) **Versioned ADRs** (ADR-003v1, v2) — breaks cross-reference stability.

**Decision**: **Option B.** When superseded: (1) Mark original with `Status: SUPERSEDED by ADR-NNN`, (2) Create new ADR referencing old, (3) Include old decision as rejected option with WHY NOT, (4) Execute cross-reference cascade: update all sections referencing the superseded ADR.
// WHY NOT A? Destroys institutional knowledge. // WHY NOT C? Breaks INV-006.

**Consequences**: Every supersession triggers a cascade update; superseded ADRs remain as record.
**Tests**: (Validated by INV-001, INV-006.)

---

## 0.7 Quality Gates

A DDIS-conforming specification is "done" when all quality gates pass. Gates are ordered; a failing Gate N makes Gates N+1 through 7 irrelevant.

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
A competent implementer (or LLM), given only the spec and public references, can begin implementing without clarifying questions about architecture, algorithms, data models, or invariants.

**Gate 7: LLM Implementation Readiness**
Give ONLY one implementation chapter (plus glossary and relevant invariants) to an LLM. Verify: (a) no hallucinated requirements, (b) no clarifying questions about architecture, (c) all chapter-header invariants preserved, (d) all negative specifications observed. Test on >= 2 chapters. (Validates INV-017, INV-018, INV-019.)

### Modularization Quality Gates [Conditional]

**Gate M-1**: All nine consistency checks pass. **Gate M-2**: Every bundle under hard ceiling; < 20% exceed target. **Gate M-3**: LLM receiving one bundle produces zero cross-module questions (>= 2 modules tested). **Gate M-4**: Every Tier 1 declaration faithfully summarizes its Tier 2 definition. **Gate M-5**: Simulated invariant change correctly identifies all affected modules.

### Definition of Done (for this standard)

DDIS 3.1 is "done" when:
- This document passes Gates 1–7 applied to itself
- At least one non-trivial spec has been written conforming to DDIS
- The Glossary covers all DDIS-specific terminology
- LLM provisions are demonstrated in this document's own element specifications

## 0.8 Performance Budgets (for Specifications, Not Software)

### 0.8.1 Specification Size Budgets

| System Complexity | Target Spec Length | Rationale |
|---|---|---|
| Small (< 5K LOC target) | 500–1,500 lines | Formal model + invariants + key ADRs |
| Medium (5K–50K LOC target) | 1,500–5,000 lines | Full DDIS treatment |
| Large (> 50K LOC target) | 5,000–15,000 lines | May split into sub-specs |

### 0.8.2 Proportional Weight Guide

| Section | % of Total | Why |
|---|---|---|
| Preamble + PART 0 | 15–20% | Dense: formal model, invariants, ADRs, quality gates |
| PART I: Foundations | 8–12% | First principles, state machines, complexity analysis |
| PART II: Core Implementation | 35–45% | THE HEART: algorithms, data, protocols, negative specs, verification prompts |
| PART III: Interfaces | 8–12% | API schemas, adapters, external contracts |
| PART IV: Operations | 10–15% | Testing, operational playbook, roadmap |
| Appendices + Part X | 10–15% | Reference material, glossary, error taxonomy, master TODO |

Domain-specific specs may adjust by +/-20%.

> **Meta-standard note**: This document has a heavier PART 0 (~30%) because it defines 20 invariants and 11 ADRs for the meta-standard itself. The +/-20% adjustment applies.

### 0.8.3 Authoring Time Budgets

| Element | Expected Time | Notes |
|---|---|---|
| First-principles model | 2–4 hours | Hardest; requires deep domain understanding |
| One invariant | 15–30 minutes | Including violation scenario and test |
| One ADR | 30–60 minutes | Including genuine alternative analysis |
| One implementation chapter | 2–4 hours | Including algorithm, examples, test strategy |
| Negative specs per chapter | 15–30 minutes | Adversarial thinking |
| Verification prompt | 10–15 minutes | Derived from invariants and negative specs |
| End-to-end trace | 1–2 hours | Requires all subsystems drafted first |
| Glossary | 1–2 hours | Best done last |

### 0.8.4 Specification Quality Measurement

| Metric | Measurement Method | Target |
|---|---|---|
| Time to first implementer question | Timer from start to first spec-gap question | > 2 hours |
| LLM hallucination rate | Unauthorized behaviors / total decisions | < 5% with negative specs |
| Cross-reference resolution time | Time to locate a referenced section | < 30 seconds |
| Gate passage rate | % of gates passing on first attempt | > 80% |

---

## 0.9 Public API Surface (of DDIS Itself)

1. **Document Structure Template** (§0.3) — the skeleton.
2. **Element Specifications** (PART II) — what each element must contain.
3. **Quality Criteria** (§0.5, §0.7) — how to validate conformance.
4. **Voice and Style Guide** (ADR-005) — how to write within the structure.
5. **Anti-Pattern Catalog** (embedded in element specs) — what bad specs look like.
6. **Error Taxonomy** (§6.3) — classification of spec authoring errors.
7. **Completeness Checklist** (Part X) — mechanical conformance validation.

---

## 0.10 Open Questions (for future DDIS versions)

1. **Multi-document specs**: How should sub-specs reference each other across spec boundaries? (See §1.5 for initial framework.)

2. **Formal verification bridge**: Pathway from falsifiable invariants to machine-checked properties?

3. **Composability across specs**: Cross-spec invariant and ADR referencing. (See §1.5.)

---

## 0.13 Modularization Protocol [Conditional]

REQUIRED when monolithic spec exceeds 4,000 lines or context window cannot hold spec plus reasoning budget. OPTIONAL but recommended for 2,500–4,000 line specs.

> Namespace note: INV-001 through INV-020 and ADR-001 through ADR-011 are DDIS meta-standard identifiers. Application specs define their OWN namespace (e.g., APP-INV-001) — never reuse INV-NNN.

### 0.13.1 The Scaling Problem

Two failure modes when spec exceeds context window: (1) **Truncation** — LLM drops beginning, losing invariants. (2) **Naive splitting** — breaks cross-references, orphans invariants. The modularization protocol prevents both. (Motivated by INV-008, INV-014.)

### 0.13.2 Core Concepts

| Concept | Definition |
|---|---|
| **Monolith** | A DDIS spec as a single document. All specs start as monoliths. |
| **Module** | Self-contained spec unit for one subsystem. Always assembled into a bundle. |
| **Constitution** | Cross-cutting material (invariants, ADRs, glossary) organized in tiers. |
| **Domain** | Architectural grouping of related modules. |
| **Bundle** | Assembled document: system constitution + domain constitution + deep context + module. |
| **Manifest** | YAML declaring modules, domains, invariant ownership, assembly rules. |

(All terms defined in Glossary.)

### 0.13.3 The Tiered Constitution

Three tiers, no overlapping content. (Locked by ADR-006.)

| Tier | Budget | Content |
|---|---|---|
| **1: System** | 200–400 lines | Design goal, non-negotiables, ALL invariant/ADR declarations (ID + 1-line), glossary, gates, context budget |
| **2: Domain** | 200–500 lines | Domain formal model, FULL invariant definitions, FULL ADR analysis, cross-domain interfaces |
| **3: Cross-Domain** | 0–600 lines | Full definitions for OTHER-domain invariants this module interfaces with |

**Assembly**: `bundle = Tier 1 + Tier 2 + Tier 3 + Module`. Target: 1,200–4,500 lines. Hard ceiling: 5,000.

**Two-tier simplification**: < 20 invariants and constitution <= 400 lines → Tier 1 has FULL definitions, skip Tiers 2-3.

### 0.13.4 Essential Rules

**Rule 1**: Cross-module references through constitution only. (INV-012, ADR-007.)
**Rule 2**: Shared types defined in constitution, not modules.
**Rule 3**: End-to-end trace is a special cross-cutting module.

### 0.13.5 Module Header

```yaml
# Module Header: [Module Name]
# Domain: [Domain Name]
# Maintains: APP-INV-017, APP-INV-018
# Interfaces: APP-INV-003 (via EventStore)
# Implements: APP-ADR-003
# Adjacent: EventStore, Scheduler
# Assembly: Tier 1 + [domain] + cross-domain deep
#
# NEGATIVE SPECIFICATION:
# - Must NOT directly access TUI rendering state
# - Must NOT bypass the reservation system for file writes
# - Must NOT assume event ordering beyond APP-INV-017
```

### 0.13.6 Decision Flowchart

```
Spec > 4,000 lines? → MODULE (required)
Spec 2,500–4,000 AND context < 8K? → MODULE (recommended)
Otherwise → MONOLITH

If MODULE:
  < 20 invariants+ADRs AND constitution ≤ 400 lines? → TWO-TIER
  Otherwise → THREE-TIER
```

For file layout, manifest schema, assembly rules, consistency checks, cascade protocol, and migration procedure, define in the spec's Appendix E at decomposition time.

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
    Guard: All §0.3 sections have content (not placeholders).

  Drafted   →[author adds cross-refs]→    Threaded
    Guard: Every section has >= 1 outbound reference. Orphan sections identified.

  Threaded  →[gates 1-5 pass]→            Gated
    Guard: All mechanical gates pass. Failures documented in Master TODO.

  Gated     →[gates 6-7 pass]→            Validated
    Guard: Human AND LLM implementer confirm readiness.

  Validated →[implementation begins]→     Living
    Guard: At least one team has started work. Change tracking enabled.

  Living    →[gap discovered]→            Drafted (partial regression)
    Guard: Gap is architectural. Affected sections marked for re-validation.

Invalid transitions (all REJECT):
  Skeleton → Gated/Threaded — Cannot gate/thread empty sections.
  Drafted → Validated/Gated — Must thread cross-references first.
  Threaded → Validated      — Must pass mechanical gates first.
  Gated → Living            — Must validate with external implementer first.
  Any → Skeleton            — Cannot un-write sections (use version control).
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
| ADR | O(alternatives x analysis_depth) | O(alternatives) per ADR | O(1) per ADR (check that alternatives are genuine) |
| Algorithm | O(algorithm_complexity x edge_cases) | O(pseudocode_length) | O(worked_examples) |
| Cross-reference | O(1) per reference | O(1) per reference | O(sections^2) for full graph analysis |
| End-to-end trace | O(subsystems x interactions) | O(subsystems) | O(1) (follow the trace) |
| Negative specification | O(adversarial_thinking) | O(1) per constraint | O(1) per constraint (check implementation) |
| Verification prompt | O(invariants_per_chapter) | O(1) per prompt | O(1) (execute the prompt) |

The quadratic cost of cross-reference verification motivates automated tooling.

### 1.4 End-to-End Trace: Authoring an ADR Through the DDIS Process

This trace follows ADR-002 (Invariants Must Be Falsifiable) from recognition through authoring to validation, exercising §0.2, §0.1.2, §0.5, §0.6, §3.4, §3.5, §0.7, and ADR-004.

**Step 1: Recognition (§0.2.1)** — "Verifiability over trust" (consequence 3) raises a decision: what formality level for invariants?

**Step 2: Non-Negotiable Check (§0.1.2)** — "Invariants are falsifiable" commits. Three alternatives exist, requiring an ADR.

**Step 3: ADR Creation (§3.5)** — ADR-002 per format: Problem, three Options, Decision (Option C with WHY NOT), Tests ("Validated by INV-003").

**Step 4: Invariant Derivation (§3.4)** — ADR-002 motivates INV-003 per format. Violation scenario ("the system shall be performant") demonstrates concretely.

**Step 5: Cross-Reference Threading** — ADR-002 → INV-003 → Gate 4 → INV-003; §3.4 references both; anti-pattern demonstrates violation.

**Step 6: Quality Gate Validation (§0.7)**
- **Gate 2**: ADR-002 traces to §0.2.1 consequence 3 → formal model. Chain intact.
- **Gate 3**: Three genuine alternatives covered.
- **Gate 4**: INV-003 has counterexample + validation.
- **Gate 5**: ADR-002, INV-003, Gate 4, §3.4 form connected subgraph.
- **Gate 7**: Give §3.4 to an LLM — it should produce invariants with violation scenarios, not aspirational statements.

**Step 7: Self-Bootstrap (ADR-004)** — ADR-002 applied to DDIS's own invariants. INV-001: can we construct a violation? Yes (unreferenced section). Test? Yes (audit). INV-001 passes INV-003.

A single ADR touches 7 structural elements across 5 chapters — validating INV-001 and INV-006.

### 1.5 Specification Composition Properties

When multiple DDIS specs compose (e.g., System B depends on System A):

- **Namespace Isolation**: Each spec owns APP-INV-NNN. Cross-spec: `[SpecName]:APP-INV-NNN`.
- **ADR Visibility**: Consuming specs reference dependency ADRs read-only; supersession requires the dependency to evolve.
- **Cross-Spec Contracts**: Intersection defined by providing spec's Public API Surface (§0.9), treated as axiomatic.
- **Composition Invariant**: `∀ inv_a, inv_b: Spec_B.depends_on(inv_a) → ¬contradicts(inv_a, inv_b)`

Initial framework; see §0.10 for unresolved aspects.

---

# PART II: CORE STANDARD — ELEMENT SPECIFICATIONS

The heart of DDIS. Each section specifies one structural element: what it must contain, quality criteria, how it relates to other elements, and what good versus bad looks like. Each includes woven LLM-specific provisions (ADR-008).

## Chapter 2: Preamble Elements

### 2.1 Design Goal

**What it is**: A single sentence (<= 30 words) that states the system's reason for existing.

**Required properties**:
- States the core value proposition, not the implementation
- Uses bold for emphasis on the 3–5 key properties
- Readable by a non-technical stakeholder

**Quality criteria**: A reader who sees only the design goal should be able to decide whether this system is relevant to them.

**DO NOT** state the design goal in terms of implementation technology ("Build a Rust-based event-sourced system"). State it in terms of value ("scrollback-native, zero-flicker terminal apps"). An LLM reading an implementation-focused design goal will over-constrain its solution space. (Validates INV-017.)

**DO NOT** exceed 30 words — a design goal longer than one sentence becomes a design essay that LLMs treat as implementation requirements rather than directional guidance. (Validates INV-007.)

**DO NOT** use unmeasurable qualities ("robust", "scalable", "enterprise-grade") — LLMs generate boilerplate prose when given abstract adjectives instead of concrete properties. (Validates INV-017.)

**Anti-pattern**: "Design goal: Build a distributed task coordination system using event sourcing and advisory reservations." -- This describes implementation, not value.

**Good example** (FrankenTUI): "Design goal: **scrollback-native, zero-flicker, agent-ergonomic, and high-performance** Rust terminal apps."

**Cross-references**: The design goal establishes vocabulary used throughout. Each bolded property should correspond to at least one invariant and one quality gate.

---

### 2.2 Core Promise

**What it is**: A single sentence (<= 40 words) describing what the system makes possible, from the user's perspective.

**Required properties**:
- Written from the user's viewpoint, not the architect's
- States concrete capabilities, not abstract properties
- Uses "without" clauses to highlight what would normally be sacrificed

**Quality criteria**: If you showed only this sentence to a potential user, they should understand what the system gives them and what it doesn't cost them.

**DO NOT** use abstract qualities without concrete meaning ("robust", "scalable", "enterprise-grade"). (Validates INV-017.)

**DO NOT** promise implementation details ("uses React", "built on PostgreSQL") — the core promise describes user-facing value. Technical choices belong in ADRs. (Validates INV-002.)

**DO NOT** omit "without" clauses — a promise that only states what the system does leaves constraints implicit, creating hallucination sites. (Validates INV-017.)

---

### 2.3 Document Note

**What it is**: A short disclaimer (2–4 sentences) about code blocks and where correctness lives.

**Why it exists**: Without this note, implementers treat code blocks as copy-paste targets. The document note redirects trust from code to invariants and tests. LLMs reproduce code blocks verbatim unless explicitly told otherwise.

**DO NOT** omit this note even if it seems obvious. (Validates INV-017.)

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
- For LLM implementers: includes a step about reading negative specifications and verification prompts

**Quality criteria**: A new team member reading only this section knows exactly how to engage with the document.

### Verification Prompt for Chapter 2 (Preamble Elements)

After writing your spec's preamble, verify:
1. [ ] Design goal is <= 30 words and states value, not implementation technology (INV-017)
2. [ ] Core promise uses "without" clauses and contains no abstract buzzwords (INV-017)
3. [ ] Document note explicitly states code blocks are design sketches, not copy-paste targets (INV-008)
4. [ ] How-to-use list starts with "Read PART 0" and includes LLM-specific step for negative specs and verification prompts
5. [ ] Your preamble does NOT use marketing language ("enterprise-grade", "cutting-edge") — these cause LLMs to generate generic boilerplate

---

## Chapter 3: PART 0 Elements

### 3.1 Non-Negotiables (Engineering Contract)

**What it is**: 5–10 philosophical commitments that must never be compromised, even under pressure. Stronger than invariants — these are commitments, not testable properties.

**Required format**: `- **[Property name]** [One sentence: what this means concretely]`

**Quality criteria**: An implementer could imagine a tempting situation to violate it; the non-negotiable clearly says no. Not a restatement of a technical invariant.

**DO NOT** restate invariants as non-negotiables — they serve different purposes. (Validates INV-017.)

**DO NOT** list more than 10 — excess dilutes the ones that matter. (Validates INV-007.)

**DO NOT** write non-negotiables no reasonable person would violate ("the system must not corrupt data") — constrain tempting shortcuts. (Validates INV-017.)

**Relationship to invariants**: Non-negotiables justify groups of invariants. "Deterministic replay is real" (non-negotiable) justifies "Same event log → identical state" (invariant).

---

### 3.2 Non-Goals

**What it is**: A list of 5–10 things the system explicitly does NOT attempt.

**Why it exists**: Scope creep is the most common spec failure. Non-goals give implementers permission to say "out of scope." For LLMs, non-goals prevent adding "helpful" features not in the spec.

**Quality criteria**: Someone has actually asked for this (or will), and the non-goal explains briefly why it's excluded.

**DO NOT** list absurd non-goals that nobody would request. Non-goals should exclude things that are tempting, not impossible. (Validates INV-017.)

---

### 3.3 First-Principles Derivation

**What it is**: The formal model from which the entire architecture derives. Makes every section feel *inevitable* rather than *asserted*.

**Required components**:

1. **"What IS a [System]?"** — A mathematical or pseudo-mathematical definition establishing the system as a formally defined state machine or function.

2. **Consequences** — 3–5 bullet points explaining what this formal definition implies for the architecture. Each consequence should feel like a discovery, not an assertion.

3. **Fundamental Operations Table** — Every primitive operation with its mathematical model and complexity target.

**Quality criteria**: After reading this section, an implementer should be able to derive the system's architecture independently. If the architecture is a surprise, the derivation is incomplete.

**DO NOT** assert the architecture without deriving it from the formal model. An LLM given an asserted architecture will not understand the constraints behind it and will make downstream decisions that violate the model. (Validates INV-001, INV-017.)

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

**Quality criteria for each invariant**: Falsifiable (constructible counterexample), Consequential (violation causes observable bad behavior), Non-trivial (not a tautology or compiler-enforced), Testable (validation method is specific enough to implement).

**Quantity guidance**: A medium-complexity system typically has 10–25 invariants.

**DO NOT** write invariants without violation scenarios — violates INV-003. **DO NOT** write invariants that merely restate type system guarantees. **DO NOT** write aspirational invariants without measurable criteria. (Validates INV-003, INV-017.)

**Anti-patterns**:
```
BAD: "INV-001: The system shall be performant."
  - Not falsifiable, no violation scenario, not testable

GOOD: "INV-003: Event Log Determinism
  Same event sequence applied to same initial state produces identical final state.
  ∀ events, ∀ state₀: reduce(state₀, events) = reduce(state₀, events)
  Violation: A reducer reads wall-clock time, causing different states on replay.
  Validation: Replay test — process 10K events, snapshot, replay, byte-compare.
  // WHY THIS MATTERS: If replay diverges, we lose auditability and debugging."
```

---

### 3.5 Architecture Decision Records (ADRs)

**What it is**: A record of each significant design decision, including alternatives rejected and why.

**Required format per ADR**:
```
### ADR-NNN: [Descriptive Title]
#### Problem  — [1–3 sentences]
#### Options  — [2–4 genuine options with concrete pros/cons]
#### Decision — [Chosen option + rationale]
  // WHY NOT [rejected]? [Brief explanation]
#### Confidence [Optional] — [High | Medium | Low]
#### Consequences — [2–4 implications]
#### Tests — [How we'll know if this was right]
```

**Quality criteria**: Genuine alternatives (a competent engineer could choose any option), concrete tradeoffs (measurable, not vague), consequential (swapping requires > 1 day refactoring).

**Confidence field**: Optional. High (validated), Medium (pending validation), Low (speculative, requires spike — cite in §6.1.1).

**DO NOT** create strawman ADRs where one option is obviously superior. **DO NOT** omit WHY NOT annotations — these are the most valuable part for LLM implementers. (Validates INV-002, INV-017.)

**Churn-magnets**: After all ADRs, identify which decisions cause the most downstream rework if changed. Lock first, spike earliest (§6.1.1).

---

### 3.6 Quality Gates

**What it is**: 4–8 stop-ship criteria, ordered by priority.

**Required properties per gate**: A gate is a **predicate** (passing or failing at any point in time), references specific invariants or test suites, is ordered so a failing Gate N makes Gate N+1 irrelevant, and at least one gate validates LLM implementation readiness (Gate 7 in §0.7).

**Quality criteria**: A project manager should be able to assess gate status in < 30 minutes using the referenced tests.

**DO NOT** define gates without concrete measurement procedures. "Code quality is high" is not a gate. "All invariants have passing tests" is a gate. (Validates INV-003, INV-017.)

---

### 3.7 Performance Budgets and Design Point

**What it is**: A table of performance targets anchored to a specific design point.

**Required components**: (1) **Design point** — specific scenario (hardware, workload, scale). (2) **Budget table** — operation → target → measurement method. (3) **Measurement harness** — how to run benchmarks. (4) **Adjustment guidance** — what to do if design point differs.

**Quality criteria**: An implementer can run the benchmarks and get a pass/fail signal without asking anyone.

**DO NOT** include performance claims without numbers, design points, or measurement methods. (Validates INV-005, INV-017.)

---

### 3.8 Negative Specifications

**What it is**: Explicit "DO NOT" constraints per implementation chapter, co-located with the subsystem they constrain. (Locked by ADR-009, validates INV-017.)

**Why it exists**: LLMs fill specification gaps with plausible but unauthorized behaviors (§0.2.2). Imperative, co-located constraints are the most effective countermeasure.

**Required format**: `- **DO NOT** [specific prohibited behavior]. [Reason or invariant reference.]` (>= 3 per subsystem.)

**Quality criteria**: Addresses a plausible LLM action, is falsifiable, references an invariant or ADR, is subsystem-specific. 3–8 per subsystem; >15 suggests the positive spec is ambiguous.

**DO NOT** write generic negative specs ("DO NOT write bad code"). (Validates INV-017.)

**Anti-pattern**: `"DO NOT use global variables"` — generic advice. **Good**: `"DO NOT bypass the reservation system for file writes. All mutations go through ReservationManager (INV-022)."`

**Self-bootstrapping**: This document's "DO NOT" paragraphs throughout §2.1–§3.8 demonstrate the pattern.

### Verification Prompt for Chapter 3 (PART 0 Elements)

After writing your spec's PART 0, verify:
1. [ ] Every non-negotiable could tempt an implementer to violate it under pressure (§3.1)
2. [ ] Every non-goal is something someone would plausibly request (INV-017)
3. [ ] The first-principles model is formal enough that the architecture can be derived independently (INV-001)
4. [ ] Every invariant has all five components: statement, formal expression, violation scenario, validation, WHY THIS MATTERS (INV-003)
5. [ ] Every ADR has >= 2 genuine alternatives where a competent engineer could choose differently (INV-002)
6. [ ] Performance budgets have numbers, design points, and measurement methods (INV-005)
7. [ ] Your PART 0 does NOT contain invariants without violation scenarios, strawman ADRs, or unfalsifiable claims

---

## Chapter 4: PART I Elements

### 4.1 Full Formal Model

**What it is**: The expanded version of the first-principles derivation from §0.2. While the executive summary gives the 1-page version, PART I gives the full treatment: complete state definition, complete input/event taxonomy, complete output/effect taxonomy, state transition semantics, and composition rules.

### 4.2 State Machines

**What it is**: Every stateful component gets a formal state machine.

**Required per state machine**: State diagram (ASCII art or description), State x Event table (no empty cells), guard conditions on transitions, invalid transition policy (ignore? error? log?), entry/exit actions.

**Quality criteria**: The state x event table has no empty cells. Every cell either names a transition or explicitly says "no transition" or "error."

**DO NOT** define state machines with only happy-path transitions. LLMs will implement only the transitions you show them. If you omit invalid transition handling, the LLM will either ignore invalid transitions (silent corruption) or crash. (Validates INV-010, INV-017.)

### 4.3 Complexity Analysis

**What it is**: Complexity bounds for every fundamental operation defined in the first-principles model.

**Required**: Big-O bounds with constants where they matter for the design point. "O(n) where n = active_agents, expected <= 300" is more useful than "O(n)."

**DO NOT** provide complexity bounds without anchoring to the design point. (Validates INV-005.)

### Verification Prompt for Chapter 4 (PART I Elements)

After writing your spec's PART I, verify:
1. [ ] The full formal model includes complete state, input, output, and transition definitions (§4.1)
2. [ ] Every state machine has a state x event table with NO empty cells (INV-010)
3. [ ] Invalid transition policies are explicit for every state machine (INV-010, INV-017)
4. [ ] Complexity analysis includes constants at the design point (§4.3)
5. [ ] Your PART I does NOT define state machines with only happy-path transitions or complexity bounds without design-point context

---

## Chapter 5: PART II Elements

### 5.1 Implementation Chapters

**What it is**: One chapter per major subsystem — where the spec earns its value.

**Required components per chapter**:

1. **Purpose statement** (2–3 sentences): What this subsystem does and why. References the formal model.
2. **Formal types**: Data structures with memory layout analysis where relevant. Include `// WHY NOT` annotations on non-obvious choices (§5.4).
3. **Algorithm pseudocode**: Every non-trivial algorithm, with complexity analysis inline.
4. **State machine** (if stateful): Full state machine per §4.2.
5. **Invariants preserved** (RESTATED): Which INV-NNN this subsystem maintains. **Restate each invariant's one-line statement, not just the ID** — required by INV-018.
6. **Negative specifications**: 3–8 "DO NOT" constraints per §3.8. (Required by INV-017.)
7. **Worked example(s)**: At least one concrete scenario with specific values (§5.2).
8. **Edge cases and error handling**: Malformed inputs, resource exhaustion, invariant threats.
9. **Test strategy**: Test types (unit, property, integration, replay, stress) for this subsystem.
10. **Performance budget**: This subsystem's share of the overall budget.
11. **Verification prompt**: Structured self-check per §5.6.
12. **Meta-instructions** (if applicable): Implementation ordering directives per §5.7.
13. **Cross-references**: To ADRs, invariants, other subsystems, the formal model.

**Quality criteria**: An implementer could build this subsystem from this chapter alone.

**DO NOT** write implementation chapters before locking their ADR dependencies. **DO NOT** reference invariants by ID alone — restate them (INV-018). (Validates INV-001, INV-017, INV-018.)

---

### 5.2 Worked Examples

**What it is**: A concrete scenario with specific values showing the subsystem processing realistic input.

**Required**: Concrete values (`task_id = T-042`, not "some task"), state before/during/after, at least one non-trivial aspect (edge case, conflict, boundary).

**DO NOT** use variables or placeholders. LLMs over-index on examples (§0.2.2). (Validates INV-017.)

**Anti-pattern**:
```
BAD: "When a task is completed, the scheduler updates the DAG."

GOOD: "Agent A-007 completes T-042. Before: T-042=InProgress, T-043 depends [T-042, T-041],
  T-041=Done. After: T-042=Done, T-043=Ready. Edge: If T-043 was cancelled during T-042,
  it remains Cancelled — dependency completion does not resurrect cancelled tasks."
```

---

### 5.3 End-to-End Trace

**What it is**: A single worked scenario traversing ALL major subsystems — proving the pieces fit together, not just work individually. (Validates INV-001.)

**Required**: Trace one event from ingestion through every subsystem to output, show exact data at each boundary, identify invariants exercised, include at least one cross-subsystem interaction that could fail.

**Self-bootstrapping**: See §1.4.

---

### 5.4 WHY NOT Annotations

**What it is**: Inline comments next to design choices explaining the road not taken.

**When to use**: Whenever a choice might look suboptimal without full context. If an implementer might think "I can improve this by doing X" and X was rejected, add a WHY NOT.

**Format**: `// WHY NOT [alternative]? [Brief tradeoff. Reference ADR-NNN if full ADR exists.]`

**Relationship to ADRs**: WHY NOT = micro-justifications for local choices. ADRs = macro-justifications for architectural choices. If a WHY NOT grows beyond 3 lines, it should become an ADR.

---

### 5.5 Comparison Blocks

**What it is**: Side-by-side SUBOPTIMAL vs CHOSEN comparisons with quantified reasoning, for choices where the quantitative difference is the justification.

**Format**: `// SUBOPTIMAL: [Rejected] — [quantified downsides] // CHOSEN: [Selected] — [quantified advantages, ADR-NNN ref]`

---

### 5.6 Verification Prompts

**What it is**: A structured self-check at each chapter's end, verifying output against the spec before moving on. (Locked by ADR-010.)

**Required format**: Checklist with >= 1 positive invariant check, >= 1 negative check (references §3.8), >= 1 integration check. All reference specific INV-NNN identifiers.

**DO NOT** write generic prompts ("did you test your code?"). Each check must be subsystem-specific. (Validates INV-017.)

**Self-bootstrapping**: Chapters 2–7 each include a verification prompt block. The meta-standard prompt:

> **Verification Prompt for a DDIS-conforming spec:**
> 1. [ ] Every implementation chapter has >= 3 negative specifications (INV-017)
> 2. [ ] Every implementation chapter restates its preserved invariants (INV-018)
> 3. [ ] An explicit implementation ordering exists as a DAG (INV-019)
> 4. [ ] Every element spec chapter has a verification prompt block (INV-020)
> 5. [ ] Five random sections trace backward to the formal model (INV-001, Gate 2)
> 6. [ ] The cross-reference graph has no orphan sections (INV-006, Gate 5)
> 7. [ ] No aspirational invariants without violation scenarios (INV-003)

---

### 5.7 Meta-Instructions

**What it is**: Directives to LLM implementers providing ordering, sequencing, and process guidance.

**Format**: `> **META-INSTRUCTION**: [Directive] > Reason: [Why]`

**When to use**: Implementation order matters (INV-019), a common shortcut would violate an invariant, or the spec leaves a micro-decision but constrains the process.

**DO NOT** use for things the spec should state directly. Meta-instructions are process guidance ("implement X before Y"), not content ("X must have property P"). (Validates INV-017.)

**Example**: `> **META-INSTRUCTION**: Implement the event store before the scheduler. > Reason: Scheduler depends on event store types (EventId, EventPayload).`

**Self-bootstrapping**: See §0.3.1.

### Verification Prompt for Chapter 5 (PART II Elements)

After writing your spec's implementation chapters, verify:
1. [ ] Each chapter has all 13 required components from §5.1
2. [ ] Preserved invariants are RESTATED (minimum: ID + one-line statement), not bare ID references (INV-018)
3. [ ] Each chapter has >= 3 subsystem-specific negative specifications per §3.8 (INV-017)
4. [ ] Worked examples use concrete values, not variables (§5.2)
5. [ ] Verification prompts include positive, negative, AND integration checks (§5.6)
6. [ ] Meta-instructions use the prescribed format with dependency reasons (§5.7, INV-019)
7. [ ] Your implementation chapters do NOT reference invariants by ID alone (INV-018), use "see above" (INV-006), or include generic negative specs (INV-017)
8. [ ] Every element spec chapter includes a verification prompt block (INV-020)

---

## Chapter 6: PART IV Elements

### 6.1 Operational Playbook

**What it is**: Prevents the most common failure mode of detailed specs: infinite refinement without shipping.

**Required sections**:

#### 6.1.1 Phase -1: Decision Spikes
Run tiny experiments to de-risk the hardest unknowns before building. Each spike produces an ADR. **Required per spike**: What question it answers, maximum time budget (1–3 days), exit criterion (one ADR).

#### 6.1.2 Exit Criteria per Phase
Every phase in the roadmap must have a specific, testable exit criterion. Not "phase complete when done" but "phase complete when X, Y, Z are demonstrated."

**Anti-pattern**: "Phase 2: Implement the scheduler. Exit: Scheduler works."
**Good example**: "Phase 2: Implement the scheduler. Exit: Property test demonstrates fair scheduling across 50 agents with no starvation for > 1000 ticks."

#### 6.1.3 Merge Discipline
What every PR touching invariants, reducers, or critical paths must include: tests, a note on which invariants it preserves, benchmark comparison if touching a hot path.

#### 6.1.4 Minimal Deliverables Order
Build order maximizing the "working subset" at each stage. Must be an explicit DAG with dependency reasons (INV-019).

#### 6.1.5 Immediate Next Steps (First PRs)
The literal first 5–6 things to implement, in dependency order. Not strategic — tactical. Converts the spec from "a plan to study" into "a plan to execute now."

---

### 6.2 Testing Strategy

**What it is**: A taxonomy of test types (adapt to domain):

| Test Type | What It Validates | Example |
|---|---|---|
| Unit | Function correctness | Reservation conflict detection |
| Property | Invariant preservation under random inputs | replay(snapshot, events) = direct_state |
| Integration | Subsystem composition | Task completion triggers scheduling cascade |
| Stress | Design point limits | 300 agents, 10K tasks, 60s sustained |
| Replay | Determinism | Process N events, snapshot, replay, byte-compare |
| Adversarial | Malicious/malformed input handling | Forged task_id event |
| LLM conformance | Spec faithfulness | Implementation matches negative specs (Gate 7) |

---

### 6.3 Error Taxonomy

**What it is**: A classification of errors with handling strategy per class.

**Required properties**: Each error class has severity (fatal, degraded, recoverable, ignorable), handling strategy (crash, retry, degrade, log-and-continue), and cross-references to threatened invariants.

**DO NOT** conflate error severity with handling strategy. A "recoverable" error with "crash" handler signals inconsistency that an LLM will implement inconsistently. (Validates INV-017.)

### Verification Prompt for Chapter 6 (PART IV Elements)

After writing your spec's operational chapters, verify:
1. [ ] Phase -1 decision spikes have time budgets and ADR exit criteria (§6.1.1)
2. [ ] Every phase has a specific, testable exit criterion (§6.1.2, INV-003)
3. [ ] Minimal deliverables order is an explicit DAG with dependency reasons (INV-019)
4. [ ] Testing strategy includes at minimum: unit, property, integration, and stress types (§6.2)
5. [ ] Error taxonomy maps each class to severity, handling strategy, and threatened invariants (§6.3)
6. [ ] Your operational chapters do NOT use aspirational exit criteria or error classes without severity

---

## Chapter 7: Appendix Elements

### 7.1 Glossary

**What it is**: Every domain-specific term, defined in 1–3 sentences with a cross-reference to where it's formally specified.

**Required properties**: Alphabetized, each entry includes `(see §X.Y)`, terms with both common and domain-specific meanings distinguish the two.

**DO NOT** define terms with circular references ("task: a unit of work in the task system"). **DO NOT** assume common-English meaning is sufficient — LLMs default to the most common meaning. (Validates INV-009, INV-017.)

---

### 7.2 Risk Register

**What it is**: Top 5–10 risks with concrete mitigations.

**Required per risk**: Risk description, impact, mitigation, detection method.

---

### 7.3 Master TODO Inventory

**What it is**: A comprehensive, checkboxable task list organized by subsystem, cross-referenced to phases and ADRs.

**Required properties**: Organized by subsystem (not by phase alone), each item small enough for a single PR, cross-references to ADRs/invariants, checkboxable format (`- [ ]`).

**DO NOT** organize by phase alone — subsystem organization ensures an LLM implementing one subsystem finds all related tasks without scanning the entire list. (Validates INV-017.)

### Verification Prompt for Chapter 7 (Appendix Elements)

After writing your spec's appendices, verify:
1. [ ] Glossary defines every domain-specific term with a cross-reference (INV-009)
2. [ ] Glossary distinguishes domain-specific from common-English meaning where applicable (INV-009)
3. [ ] Risk register includes detection methods, not just mitigations (§7.2)
4. [ ] Master TODO is organized by subsystem and cross-referenced to ADRs and phases (§7.3)
5. [ ] Your appendices do NOT contain circular glossary definitions or risks without detection methods

---
