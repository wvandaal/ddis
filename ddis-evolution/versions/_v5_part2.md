## 0.3 Document Structure (Required)

A DDIS-conforming specification must contain the following structure. Sections may be renamed to fit the domain but the structural elements are mandatory unless explicitly marked [Optional].

```
PREAMBLE
  Design goal (one sentence)
  Core promise (user-facing, one sentence)
  Document note (about code sketches and where correctness lives)
  How to use this plan (numbered practical steps)
  Conformance level declaration (Essential | Standard | Complete)

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
    - Implementation mapping (spec-to-code traceability)
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

**INV-023: Example Correctness**

*Every worked example in the specification is verifiable against the spec's own invariants — the before state, operation, and after state are consistent with the formal model and no invariant is violated unless the example explicitly demonstrates a violation scenario.*

```
∀ example ∈ worked_examples:
  let (state_before, operation, state_after) = example.components
  ∀ inv ∈ applicable_invariants(example.subsystem):
    inv.holds(state_after) ∨ example.is_violation_scenario_for(inv)
```

Violation scenario: A worked example for the task scheduler shows a task transitioning from Ready to Done, skipping InProgress. An LLM reproduces this transition in its implementation, violating the state machine invariant.

Validation: For each worked example, identify the applicable invariants. Verify the example's after-state satisfies all applicable invariants (unless the example is explicitly demonstrating a violation).

// WHY THIS MATTERS: LLMs over-index on examples (§0.2.3). An incorrect example is actively harmful — it will be faithfully reproduced. Example correctness is a safety property, not polish.

---

**INV-024: Conditional Section Coherence**

*Every section marked [Optional] or [Conditional] includes decision criteria — a predicate that determines when the section is required. Sections without markers are unconditionally required.*

```
∀ section ∈ spec where section.marker ∈ {Optional, Conditional}:
  section.has(decision_criteria) ∧
  decision_criteria.is_evaluable_by(spec_author)
```

Violation scenario: A section marked [Conditional] says 'include if needed' without specifying what 'needed' means. An author omits a section that their system actually requires because the decision criteria are vague.

Validation: For each [Optional] or [Conditional] section, verify the decision criteria are concrete enough that two authors assessing the same system would reach the same include/exclude decision.

// WHY THIS MATTERS: Vague conditionality is a hidden optionality that undermines spec completeness. Decision criteria make the include/exclude judgment reproducible.

---

**INV-025: Spec-to-Implementation Traceability**

*Every invariant and every algorithm in the specification maps to at least one implementation artifact (file, function, or test) in the implementation mapping, and every implementation artifact maps back to at least one spec element.*

```
∀ inv ∈ Invariants ∪ Algorithms:
  ∃ artifact ∈ implementation_mapping:
    artifact.spec_elements.contains(inv)
∀ artifact ∈ implementation_mapping:
  artifact.spec_elements.count ≥ 1
```

Violation scenario: A spec defines INV-003 (deterministic replay) but the implementation mapping has no entry for it. Six months later, a developer refactors the replay module without knowing INV-003 exists — the invariant is violated silently because nothing connected it to the code.

Validation: Parse the implementation mapping (§5.9). For each invariant and algorithm in the spec, verify at least one artifact entry exists. For each artifact entry, verify it references at least one spec element. Coverage ratio = mapped_elements / total_elements; target ≥ 0.9.

// WHY THIS MATTERS: Specs without traceability to code are documentation, not engineering artifacts. The mapping is what makes invariants enforceable — without it, the spec and the code drift apart silently. (New in 5.0; locked by ADR-016.)

---

**INV-026: Verification Coverage Completeness**

*Every invariant appears in at least one verification prompt (§5.6) across the spec's implementation chapters. An invariant that no verification prompt checks is an invariant that no LLM self-check will catch.*

```
∀ inv ∈ Invariants:
  ∃ chapter ∈ implementation_chapters:
    chapter.verification_prompt.references(inv)
```

Violation scenario: INV-007 (no starvation) is defined in the invariants section but no implementation chapter's verification prompt asks "can you construct a scenario where a task starves?" An LLM implements the scheduler, passes the scheduler's verification prompt (which checks other invariants), and ships a starvation bug.

Validation: Extract all invariant IDs from the spec. For each, search all verification prompts for a reference to that ID. Any invariant with zero verification prompt references violates INV-026.

// WHY THIS MATTERS: Verification prompts are the LLM's self-check mechanism (§0.2.3, Principle L3). An invariant missing from all verification prompts is invisible to the LLM's self-check — it can only be caught by external review. (New in 5.0.)

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

---

## 0.6 Architecture Decision Records

### ADR-001: Document Structure Is Fixed, Not Flexible

#### Problem
Should DDIS prescribe a fixed document structure, or allow authors to organize freely?

#### Options
A) **Fixed structure** (prescribed section ordering and hierarchy)
- Pros: Predictable for readers and LLMs; mechanical completeness checking; easier to teach.
- Cons: May feel rigid; some domains fit the structure better than others.

B) **Content requirements only** (prescribe what, not where)
- Pros: Flexibility; authors can organize by whatever axis makes sense.
- Cons: Every spec is unique; readers must re-learn structure each time; LLMs cannot predict where to find elements.

C) **Fixed skeleton with flexible interior**
- Pros: Balance of predictability and flexibility.
- Cons: The "flexible interior" often means "no structure at all."

#### Decision
**Option A: Fixed structure.** The value of DDIS is that a reader who has seen one DDIS spec can navigate any other. This is worth the cost of occasionally awkward section placement. LLMs benefit significantly from structural predictability (§0.2.3) — fixed formats reduce variance in output quality.

The structure may be renamed and domain-specific sections may be added within any PART, but the required elements (§0.3) must appear and the PART ordering must be preserved.

#### Consequences
- Authors must sometimes figure out where domain-specific concepts "live" in the DDIS structure
- Readers and LLMs gain predictability and can skip to known locations
- Validation tools can check structural conformance mechanically

#### Tests
- (Validated by [[INV-001|causal traceability]] and [[INV-006|cross-reference density]]) If content is placed in an unexpected location, cross-references will break, surfacing the misplacement.

---

### ADR-002: Invariants Must Be Falsifiable, Not Merely True

#### Problem
Should invariants be aspirational properties or formal contracts with concrete violation scenarios?

#### Options
A) **Aspirational invariants** — Pros: Easy to write. Cons: Cannot be tested; useless for verification.
B) **Formal invariants with proof obligations** (TLA+-style) — Pros: Machine-checkable. Cons: Requires formal methods expertise; most implementers can't read them.
C) **Falsifiable invariants** — Pros: Testable and readable. Cons: Not machine-checkable; relies on human judgment.

#### Decision
**Option C: Falsifiable invariants.** Every invariant must include: a plain-language statement, a semi-formal expression, a violation scenario, and a validation method.

// WHY NOT Option B? The goal is implementation correctness by humans and LLMs, not machine-checked proofs. The authoring cost of full formal verification exceeds the benefit for most systems.

#### Tests
- (Validated by [[INV-003|invariant falsifiability]]) Every invariant in a DDIS spec must have a constructible counterexample.

---

### ADR-003: Cross-References Are Mandatory, Not Optional Polish

#### Problem
Should cross-references between sections be recommended or required?

#### Options
A) **Recommended** — encourage authors to add cross-references where helpful.
B) **Required** — every non-trivial section must have inbound and outbound references.

#### Decision
**Option B: Required.** Cross-references are the mechanism that transforms a collection of sections into a unified specification. Without them, sections exist in isolation and the causal chain ([[INV-001|every section traces to formal model]]) cannot be verified.

#### Consequences
- Higher authoring cost (every section requires thinking about its relationships)
- Much higher reader value (any section can be understood in context)
- Enables graph-based validation of spec completeness

#### Tests
- (Validated by [[INV-006|cross-reference density]]) Build the reference graph; no orphan sections.

---

### ADR-004: Self-Bootstrapping as Validation Strategy

#### Problem
How do we validate that the DDIS standard itself is coherent and complete?

#### Options
A) **External validation** — write the standard in prose, validate by review.
B) **Self-bootstrapping** — write the standard in its own format, validate by self-conformance.

#### Decision
**Option B: Self-bootstrapping.** This document is both the standard and its first conforming instance. If the standard is unclear, the author discovers this while attempting to apply it to itself.

// WHY NOT Option A? A standard that cannot be applied to itself is suspect.

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
**Option B: Voice guidance.** Specifications fail when they are too dry to read or too casual to trust. DDIS prescribes a specific voice: technically precise but human. (See §8.1.) LLMs benefit significantly from explicit voice guidance — without it, they produce generic boilerplate or hedged academic prose (§0.2.3).

#### Tests
- Qualitative review: sample 5 sections, assess whether each sounds like a senior engineer talking to a peer.

---

### ADR-006: Tiered Constitution over Flat Root [Conditional — modular specs only]

#### Problem
When a DDIS spec is modular (§0.13), constitutional context must accompany every module bundle. How should this constitutional context be structured?

#### Options
A) **Flat root** — one file containing everything. Cons: Doesn't scale past ~20 invariants / ~10 ADRs.
B) **Two-tier** — system constitution (full definitions) + modules. Works for small modular specs (< 20 invariants).
C) **Three-tier** — system constitution (declarations) + domain constitution (definitions) + cross-domain deep context + module. Scales to large specs.

#### Decision
**Option C as the full protocol, with Option B as a blessed simplification** for small specs. The `tier_mode` field in the manifest selects between them.

// WHY NOT Option A? At scale, the flat root consumes 30–37% of the context budget before the module even starts.

#### Tests
- (Validated by [[INV-014|bundle budget compliance]]) and ([[INV-011|module completeness]])

---

### ADR-007: Cross-Module References Through Constitution Only [Conditional — modular specs only]

#### Problem
When a DDIS spec is modular, how should modules reference content in other modules?

#### Options
A) **Direct references** — Cons: Creates invisible dependencies, defeating modularization.
B) **Through constitution only** — Module A references APP-INV-032, which lives in the constitution. Pros: Enforces isolation mechanically.

#### Decision
**Option B: Through constitution only.** [[INV-012|cross-module isolation]] enforces this mechanically.

// WHY NOT Option A? It breaks [[INV-011|module completeness]].

#### Tests
- (Validated by [[INV-012|no direct cross-module references]])

---

### ADR-008: Negative Specifications Required per Implementation Chapter

#### Problem
Should DDIS require implementation chapters to include explicit "must NOT" constraints?

#### Options
A) **Not required** — Lower authoring burden. Cons: Most authors omit them. LLMs fill the gap with plausible but incorrect behavior from training data.
B) **Required per implementation chapter** — Each chapter includes at least one "must NOT" constraint targeting the most plausible misinterpretation. Pros: Directly addresses LLM hallucination gap (§0.2.3).
C) **Required per spec, not per chapter** — Centralized. Cons: Violates context-window self-sufficiency (§0.2.3, Principle L2).

#### Decision
**Option B: Required per implementation chapter.** Quality criteria (§3.8) ensure they are not trivial.

// WHY NOT Option C? It violates Principle L2 — an LLM processing a single chapter wouldn't see negative specs defined elsewhere.

#### Tests
- (Validated by [[INV-017|negative spec per chapter]]) and Gate 7.

---

### ADR-009: Structural Redundancy over DRY for Cross-References

#### Problem
Should cross-references include only an identifier (DRY principle) or restate the substance?

#### Options
A) **ID-only references** (DRY) — Cons: Useless when the reader doesn't have the definition in context.
B) **Substance-restated references** — "Preserves [[INV-003|same event sequence → identical final state]]." Pros: Self-contained chapters. Cons: Staleness risk.
C) **Full definition at point of use** — Cons: Massive duplication; very high staleness risk.

#### Decision
**Option B: Substance-restated references.** This trades DRY for LLM self-sufficiency per chapter.

// WHY NOT Option A? An LLM reading "Preserves INV-003" with no context will either ignore the constraint or hallucinate what it means.
// WHY NOT Option C? Full duplication creates unacceptable staleness risk.

Staleness risk is mitigated by machine-readable cross-reference syntax ([[INV-022|parseable cross-refs]]) enabling automated staleness detection, and by the Specification Error Taxonomy (Appendix D).

#### Tests
- (Validated by [[INV-018|substance restated at point of use]])

---

### ADR-010: Verification Prompts Required per Implementation Chapter

#### Problem
Should DDIS require implementation chapters to end with a structured self-check prompt?

#### Options
A) **Not required** — Rely on external review. Cons: Errors caught at review, not during generation.
B) **Required per implementation chapter** — Each chapter ends with a verification prompt referencing specific invariants and negative specs. Pros: LLMs self-check during generation.
C) **Single verification prompt for the whole spec** — Cons: Violates context-window self-sufficiency.

#### Decision
**Option B: Required per implementation chapter.** Quality criteria (§5.6) ensure prompts are specific.

// WHY NOT Option C? Same reasoning as ADR-008 Option C — must be at point of use.

#### Tests
- (Validated by [[INV-019|verification prompt per chapter]])

---

### ADR-011: LLM Provisions Woven Throughout, Not Isolated

#### Problem
Should LLM-specific provisions be a separate chapter or woven throughout the element specifications?

#### Options
A) **Separate chapter** — "Chapter 14: LLM Considerations." Cons: Authors treat them as an afterthought.
B) **Woven throughout** — Each element specification includes LLM-specific quality criteria. Pros: Guidance encountered at point of use.

#### Decision
**Option B: Woven throughout.** Each element specification in PART II includes LLM-specific quality criteria and anti-patterns where relevant.

// WHY NOT Option A? It produces the anti-pattern the improvement prompt warned against: "The Afterthought LLM Section."

#### Tests
- At least 60% of element specifications in PART II include LLM-specific guidance.

---

### ADR-012: Confidence Levels on Architecture Decisions

**Confidence: Decided**

#### Problem
Not all ADRs carry equal certainty. Some are well-validated by experience; others are provisional bets that should be revisited after a spike or prototype. Should DDIS formalize this distinction?

#### Options
A) **No confidence annotation** — All ADRs are treated as equally final. Pros: Simplicity. Cons: Teams either treat provisional decisions as sacred (delaying necessary revisits) or treat all decisions as mutable (defeating the purpose of ADRs).
B) **Binary confidence: Decided / Provisional** — Each ADR declares its confidence level and, if Provisional, a review trigger. Pros: Lightweight; surfaces which decisions need revisiting. Cons: Slightly higher authoring cost per ADR.
C) **Multi-level confidence scale** (High / Medium / Low / Experimental) — Pros: Granular. Cons: Subjective — what's "Medium" to one author is "High" to another. Granularity without calibration is noise.

#### Decision
**Option B: Binary confidence.** Each ADR includes a `Confidence` field:
- **Decided** — Validated by experience, spike, or strong reasoning. Changing this requires a new ADR superseding it.
- **Provisional (review trigger: X)** — Best current judgment. MUST specify a concrete trigger for re-evaluation (e.g., "after first 1000 events in production" or "after spike on alternative B").

// WHY NOT Option C? The distinction that matters in practice is "can we build on this?" vs "we might need to revisit." Finer gradations add authoring cost without changing behavior.

#### Consequences
- Provisional ADRs surface technical debt explicitly
- Review triggers prevent "provisional forever" — each has a concrete re-evaluation point
- LLMs can distinguish load-bearing decisions from tentative ones when implementing

#### Tests
- Every ADR has a Confidence field. Every Provisional ADR has a non-empty review trigger. (Automated by Gate 8 when using machine-readable syntax.)

---

### ADR-013: Machine-Readable Cross-Reference Syntax

**Confidence: Decided**

#### Problem
Cross-references are the structural backbone of a DDIS spec ([[INV-006|no orphan sections]]). But freeform reference styles ("see INV-003," "per the determinism invariant," "(INV-003: determinism)") make automated validation impossible. Should DDIS prescribe a parseable syntax?

#### Options
A) **No prescribed syntax** — Authors use natural language references. Pros: No learning curve. Cons: Automated cross-reference validation, staleness detection, and density checks are impossible. Quality depends entirely on manual review.
B) **Wiki-link syntax: `[[TARGET|substance]]`** — A lightweight, widely-recognized pattern. Pros: Parseable by regex `\[\[.+?\|.+?\]\]`; familiar from wikis and note-taking tools; renders naturally in Markdown viewers (as `[[INV-003|determinism]]`); enables automated graph construction. Cons: Authors must learn the syntax; older Markdown renderers show raw brackets.
C) **Custom XML tags: `<ref target="INV-003">substance</ref>`** — Pros: Unambiguous parsing. Cons: Verbose; disrupts reading flow; unfamiliar to most spec authors; LLMs may hallucinate invalid XML.

#### Decision
**Option B: Wiki-link syntax.** References use `[[TARGET-ID|substance summary]]` where TARGET-ID is `INV-NNN`, `ADR-NNN`, `§N.N`, or `APP-INV-NNN` (for domain specs). The substance summary is a brief restatement satisfying [[INV-018|substance restated at point of use]].

// WHY NOT Option A? The single biggest blocker to automated spec testing is unparseable references. Manual validation doesn't scale.
// WHY NOT Option C? XML is hostile to authors and LLMs alike. The wiki-link pattern achieves the same parsing benefits with less friction.

#### Consequences
- Cross-reference graphs can be built automatically
- Stale restatements can be flagged by comparing substance to source definitions
- Reference density ([[INV-006|no orphans]]) becomes a CI check
- Authors need a 30-second introduction to the `[[ID|substance]]` pattern
- Gate 8 (Specification Testability) becomes operational

#### Tests
- (Validated by [[INV-022|parseable cross-refs]]) Every invariant/ADR reference matches the pattern and resolves to an existing element.

---

### ADR-014: Graduated Conformance Levels

**Confidence: Decided**

#### Problem
DDIS prescribes 22+ invariants, 8 quality gates, negative specifications, verification prompts, meta-instructions, machine-readable cross-references, composability protocols, and modularization. For a team writing their first spec, this is overwhelming. Adoption friction causes teams to either attempt everything and abandon, or ignore the standard entirely. Should DDIS define conformance levels?

#### Options
A) **Single level** — All or nothing. Pros: Simplicity; no ambiguity about what's required. Cons: High adoption barrier; teams that need some structure get none because Complete is too much.
B) **Three levels: Essential / Standard / Complete** — Each level adds requirements. Pros: Teams adopt incrementally; Essential is achievable in a day; Standard adds LLM provisions; Complete adds tooling and composability. Cons: Slightly more complex standard; specs must declare their level.
C) **Checklist with percentages** — 'Conforming if 80% of elements present.' Pros: Flexible. Cons: Which 20% can you skip? Different authors skip different elements, making conformance meaningless.

#### Decision
**Option B: Three levels.** Defined in §0.2.6. The level boundary is drawn at natural capability thresholds: Essential = correct specs, Standard = LLM-ready specs, Complete = enterprise-scale specs.

// WHY NOT Option C? Percentage-based conformance creates the illusion of compliance. A spec missing all negative specifications but present for everything else is 90% complete but catastrophically incomplete for LLM consumption.

#### Consequences
- Specs must declare conformance level in the preamble
- Validation tools check against the declared level, not always against Complete
- Teams have a clear upgrade path from Essential → Standard → Complete

#### Tests
- Every conformance level in §0.2.6 has a concrete, checkable list of required elements.
- (Validated by [[INV-024|conditional section coherence]]) — conformance level determines which conditional sections apply.

---

### ADR-015: Formal ADR Lifecycle

**Confidence: Decided**

#### Problem
ADR-012 added Decided/Provisional confidence levels. But in living specs (§13.1), ADRs also get superseded, and teams sometimes need to propose decisions before committing. The full lifecycle — from proposal through decision to potential supersession — is informal. Should DDIS formalize it?

#### Options
A) **Keep binary confidence only** (Decided / Provisional) — Pros: Simple. Cons: No formal mechanism for proposals or supersession; 'Superseded by ADR-NNN' is mentioned but not structured.
B) **Formal lifecycle: Proposed → Decided|Provisional → Superseded** — Pros: Every ADR state has clear semantics and transition rules; supersession is traceable. Cons: Slightly more ADR overhead.
C) **Full state machine with Draft/Review/Approved/Deprecated** — Pros: Complete governance. Cons: Bureaucratic; DDIS is a spec standard, not a governance framework.

#### Decision
**Option B: Three-state lifecycle.** Each ADR has a lifecycle state:
- **Proposed** — Under evaluation. Has options and analysis but no decision. MUST have a decision deadline. Cannot be referenced as a constraint by implementation chapters.
- **Decided** — Committed. Load-bearing. Changing requires a new ADR that supersedes this one.
- **Provisional** — Decided but with a review trigger (per [[ADR-012|confidence levels]]). Same as Decided for implementation purposes, but flagged for re-evaluation.
- **Superseded** — Replaced by a newer ADR. Retained for history. MUST reference the superseding ADR. All cross-references to a superseded ADR must be updated to point to its replacement.

Transitions:
```
Proposed →[commit]→ Decided       Guard: genuine alternatives evaluated
Proposed →[commit]→ Provisional   Guard: review trigger specified
Decided  →[supersede]→ Superseded Guard: new ADR exists with rationale
Provisional →[review trigger fires]→ Decided    Guard: trigger condition met
Provisional →[review trigger fires]→ Superseded Guard: new ADR better
```

// WHY NOT Option C? The distinction between Draft and Proposed, or Approved and Decided, adds ceremony without changing behavior.

#### Consequences
- Proposed ADRs are visible but not constraining — prevents building on uncommitted decisions
- Superseded ADRs remain in the document for historical context
- The lifecycle is a proper state machine ([[INV-010|state machine completeness]])

#### Tests
- Every ADR declares its lifecycle state. No ADR lacks a state.
- Every Superseded ADR references its replacement. Every Proposed ADR has a decision deadline.

---

### ADR-016: Structured Implementation Mapping

**Confidence: Decided**

#### Problem
DDIS specs trace from first principles through invariants and ADRs to implementation chapters. But the final link — from spec elements to actual code artifacts (files, functions, tests) — is unstructured. When a developer modifies a file, they have no structured way to know which invariants that file enforces. When an invariant changes, there is no structured way to find the code that must be updated. Should DDIS prescribe a format for this mapping?

#### Options
A) **No prescribed mapping** — Developers maintain mental models of which code enforces which invariants. Pros: Zero overhead. Cons: The mapping exists only in oral tradition; new team members and LLMs cannot access it; spec-code drift is invisible.
B) **Structured mapping table per chapter** — Each implementation chapter ends with a table mapping its invariants, algorithms, and negative specs to target files/functions/tests. Pros: Lightweight; lives in the spec; searchable; LLMs can use it to verify coverage during Pass 4 (§0.2.7). Cons: Slightly more authoring overhead; can become stale.
C) **External traceability tool** — Use a separate tool (e.g., a requirements management system) to maintain the mapping. Pros: Rich tooling. Cons: Requires tooling adoption; the mapping lives outside the spec, violating self-containment ([[INV-008|spec answers all implementation questions]]); LLMs cannot access external tools during implementation.

#### Decision
**Option B: Structured mapping table per chapter.** The mapping lives in the spec (preserving self-containment), is lightweight (a table, not a database), and is consumable by both humans and LLMs. The format is specified in §5.9.

// WHY NOT Option A? The mapping exists whether you write it down or not — the question is whether it's accessible. Undocumented mappings are the #1 cause of invariant violations during refactoring.
// WHY NOT Option C? External tools break self-containment and are inaccessible to LLMs during implementation.

Staleness risk is mitigated by automated validation: Test 8 (§12.3) checks that every invariant appears in at least one mapping entry. The implementation mapping is updated as part of the living spec workflow (§13.1).

#### Consequences
- Spec-to-code traceability becomes explicit and searchable
- LLMs can verify coverage completeness during Pass 4 (§0.2.7)
- Refactoring becomes safer: before modifying a file, check which invariants it enforces
- Slightly higher authoring overhead (one table per implementation chapter)

#### Tests
- (Validated by [[INV-025|spec-to-code traceability]]) Every invariant maps to at least one artifact.
- (Validated by Gate 10) Implementation mapping coverage ≥ 90%.
