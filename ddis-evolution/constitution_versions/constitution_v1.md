# DDIS: Decision-Driven Implementation Specification

## Version 2.0

> **Design goal:** A formal standard for writing implementation specifications that are precise enough for an **LLM or junior engineer** to implement correctly without guessing, while remaining readable enough that a senior engineer would choose to read them voluntarily.

> **Core promise:** A specification conforming to DDIS contains everything needed to implement the described system — architecture, algorithms, invariants, decisions, test strategies, performance budgets, negative specifications, verification prompts, and execution plan — in a single cohesive document where every section earns its place by serving the sections around it.

> **Document note:** This standard is self-bootstrapping: it is written in the format it defines. Code blocks are design sketches for illustration. The correctness contract lives in the invariants, not in any particular syntax. Where this standard prescribes a structural element, that element is demonstrated within this document — search for "[SELF-BOOTSTRAP]" annotations to locate each demonstration.

> **How to use this plan:**
> 1. Read PART 0 end-to-end before touching any implementation chapter.
> 2. Identify the **churn-magnets** — ADR-001 (fixed structure) and ADR-009 (woven LLM provisions) cause the most downstream rework if revisited.
> 3. Use the Master TODO (PART X) as your execution checklist.
> 4. **Non-negotiable process requirement:** Every invariant you write must have a concrete violation scenario before you move on.
> 5. When writing a spec for LLM consumption, read §0.2.3 (LLM Consumption Model) first — it shapes every structural decision.
> 6. Run the self-validation checklist (§12.1) before declaring any gate passed.

---

# PART 0: EXECUTIVE BLUEPRINT

## §0.1 Executive Summary

DDIS (Decision-Driven Implementation Specification) is a standard for writing technical specifications that bridge the gap between architectural vision and correct implementation. Its **primary optimization target is LLM consumption** — the most common implementer reading a DDIS-conforming spec will be a large language model.

Most specifications fail in one of two ways: they are too abstract (the implementer must guess at critical details) or too mechanical (they prescribe code without explaining why, making evolution impossible). DDIS avoids both failure modes by requiring a **causal chain** from first principles through decisions to implementation details, where every element justifies its existence by serving the elements around it.

DDIS synthesizes techniques from several traditions — Architecture Decision Records (Nygard), Design by Contract (Meyer), temporal invariants (Lamport), performance budgeting (game engine development), and test-driven specification — into a unified document structure. The synthesis is the contribution: these techniques are well-known individually but rarely composed into a single coherent standard.

### §0.1.1 What DDIS Is

DDIS is a document standard. It specifies:

- What structural elements a specification must contain
- How those elements must relate to each other (the cross-reference web)
- What quality criteria each element must meet
- How to validate that a specification is complete
- What each element must NOT contain (negative specifications — see INV-017)
- How to structure content for LLM consumption (§0.2.3)

DDIS is domain-agnostic. It can describe a terminal rendering kernel, an agent coordination system, a database engine, a compiler, or any system where correctness matters and multiple people (or LLMs) will implement from the spec.

### §0.1.2 Non-Negotiables (Engineering Contract)

[SELF-BOOTSTRAP: This section demonstrates the §3.1 element specification for non-negotiables.]

- **Causal chain is unbroken** — Every implementation detail traces back through a decision, through an invariant, to a first principle. (Validated by INV-001, Gate 2.)
- **Decisions are explicit and locked** — Every design choice that could reasonably go another way is captured in an ADR with genuine alternatives considered. (Validated by INV-002, Gate 3.)
- **Invariants are falsifiable** — Every invariant can be violated by a concrete scenario and detected by a concrete test. (Validated by INV-003, Gate 4.)
- **No implementation detail is unsupported** — Every algorithm, data structure, state machine, and protocol has: pseudocode, complexity analysis, at least one worked example, and a test strategy. (Validated by INV-004.)
- **Cross-references form a web, not a list** — ADRs reference invariants. Invariants reference tests. Tests reference performance budgets. The design point references first principles. (Validated by INV-006, Gate 5.)
- **The document is self-contained** — A competent implementer with the spec alone can build a correct v1. (Validated by INV-008, Gate 6.)
- **LLM failure modes are explicitly prevented** — Every element includes negative specifications that block the most common LLM hallucination patterns. (Validated by INV-017, Gate 7.)

### §0.1.3 Non-Goals (Explicit)

[SELF-BOOTSTRAP: This section demonstrates the §3.2 element specification for non-goals.]

- **To replace code.** A spec is not an implementation. Pseudocode illustrates intent; it is not compilable.
- **To eliminate judgment.** DDIS constrains macro-decisions so micro-decisions are locally safe. An implementer still chooses variable names, error messages, and local optimizations.
- **To be a project management framework.** The Master TODO and roadmap are execution aids, not sprint planning.
- **To prescribe notation.** DDIS requires formal models but does not mandate any specific formalism (TLA+, Alloy, Z, or plain mathematics are all acceptable).
- **To guarantee correctness.** The spec is a contract for intent, not a machine-checked proof. It reduces risk; it does not eliminate it.
- **To optimize for human reading at the expense of LLM effectiveness.** When human readability and LLM implementation success conflict, LLM effectiveness wins (see §0.2.3, ADR-009). Human readability remains a strong secondary requirement.

## §0.2 First-Principles Derivation

### §0.2.1 What IS an Implementation Specification?

A specification is a function from intent to artifact:

```
Spec: (Problem, Constraints, Knowledge) → Document
where:
  Document enables: Implementer × Document → Correct_System
```

The quality of a specification is measured by one criterion: **does an implementer produce a correct system from it, without requiring information not in the document?**

When the implementer is an LLM, "without requiring information not in the document" becomes acute: an LLM cannot ask clarifying questions mid-implementation (or if it can, each question costs context and risks losing earlier context). The spec must anticipate what the LLM will get wrong and preemptively constrain it.

Consequences:

1. **Completeness over elegance.** A verbose spec that leaves nothing ambiguous is better than a terse spec that leaves critical details to inference. (But see INV-007: verbosity without structure is noise.)
2. **Decisions over descriptions.** The hardest part is making the hundreds of design decisions that determine correctness. An LLM without explicit decisions will hallucinate plausible ones.
3. **Verifiability over trust.** Every claim must be testable. An LLM cannot exercise judgment about which claims to trust.
4. **Negative constraints over positive descriptions alone.** Saying what the system must NOT do prevents the most common LLM failure mode: generating plausible but incorrect behavior.

### §0.2.2 The Causal Chain

Every DDIS element exists to prevent a specific failure mode. The causal chain makes this explicit:

| Failure Mode | Symptom | DDIS Element That Prevents It |
|---|---|---|
| Implementer builds the wrong abstraction | Core types don't fit the domain | First-principles formal model (§0.2) |
| Two implementers make incompatible choices | Modules don't compose | Architecture Decision Records (§0.6) |
| System works but violates a safety property | Subtle correctness bugs | Numbered invariants with tests (§0.5) |
| System is correct but too slow | Performance death by a thousand cuts | Performance budgets with benchmarks (§0.8) |
| Nobody knows if the system is "done" | Infinite refinement | Quality gates + Definition of Done (§0.7) |
| New contributor can't understand the system | Oral tradition required | Cross-reference web + glossary |
| Spec covers happy path but not edge cases | Production failures on unusual inputs | Worked examples + end-to-end traces (§5.2, §5.3) |
| Spec is so long nobody reads it | Shelfware | Proportional weight guide + voice guidance (Ch. 8–9) |
| LLM hallucinates plausible but wrong details | Silent correctness bugs | Negative specifications + verification prompts (INV-017, INV-019) |
| LLM loses critical context in long specs | Invariant violations in late chapters | Structural redundancy at point of use (INV-018) |
| LLM implements in wrong order, breaking deps | Subtle integration failures | Implementation meta-instructions (INV-020) |

```
First Principles (formal model of the problem)
  ↓ justifies
Non-Negotiables + Invariants (what must always be true)
  ↓ constrained by
Architecture Decision Records (choices that could go either way)
  ↓ implemented via
Algorithms + Data Structures + Protocols (pseudocode, state machines)
  ↓ bounded by
Negative Specifications (what must NOT happen — INV-017)
  ↓ verified by
Test Strategies + Performance Budgets (how we know it's correct and fast)
  ↓ self-checked by
Verification Prompts (LLM self-check per chapter — INV-019)
  ↓ shipped via
Quality Gates + Master TODO (stop-ship criteria, execution checklist)
```

### §0.2.3 LLM Consumption Model

[SELF-BOOTSTRAP: This section is the foundational justification for INV-017 through INV-020 and ADR-008 through ADR-011. All LLM-specific provisions in this standard trace back to the three principles defined here.]

**Why this section exists:** DDIS's primary consumer is an LLM implementing from the spec. LLMs have specific cognitive characteristics that differ from human implementers. This section defines a model of those characteristics and derives structural requirements from them.

**Principle L1: Minimize the Hallucination Gap.** An LLM will fill gaps in the spec with plausible content. The more gaps, the more hallucinations. Therefore: every element must specify not just what IS required but what is NOT (negative specifications). The spec must be explicit about every decision — implicit decisions become hallucinated decisions.

*Structural consequence:* INV-017 (Negative Specification Coverage), ADR-008. Every element specification in PART II must include an anti-pattern section and explicit "do NOT" constraints.

**Principle L2: Context-Window Awareness.** LLMs process specs within a finite context window. Information at the beginning of a long document receives less attention than information near the current generation point. Critical invariants stated only once at the top of a 5,000-line spec may be effectively invisible when the LLM implements Chapter 12.

*Structural consequence:* INV-018 (Structural Redundancy at Point of Use), ADR-010. Key invariants must be restated in every implementation chapter that relies on them — not by full repetition, but by a compact reminder: `// REMINDER: INV-003 requires a concrete violation scenario for every invariant defined in this subsystem.`

**Principle L3: Self-Sufficiency per Chapter.** An LLM benefits from being able to verify its own output against the spec before moving to the next chapter. Without explicit verification criteria, the LLM cannot distinguish between "I implemented this correctly" and "I implemented something that looks plausible."

*Structural consequence:* INV-019 (Verification Prompts), ADR-011. Every implementation chapter must end with a verification prompt — a checklist the LLM can evaluate its own output against.

**Principle L4: Ordering Explicitness.** LLMs implement in the order content appears unless explicitly directed otherwise. If Chapter 7 depends on types defined in Chapter 4, this must be stated as a meta-instruction, not left to inference.

*Structural consequence:* INV-020 (Implementation Meta-Instructions). Explicit ordering directives: "Implement §4 before §7 because EventStore types are consumed by the Scheduler."

### §0.2.4 Fundamental Operations of a Specification

| Operation | What It Does | DDIS Element |
|---|---|---|
| **Define** | Establish what the system IS, formally | First-principles model, formal types |
| **Constrain** | State what must always hold | Invariants, non-negotiables |
| **Exclude** | State what the system must NOT do or be | Negative specifications, non-goals (INV-017) |
| **Decide** | Lock choices where alternatives exist | ADRs |
| **Describe** | Specify how components work | Algorithms, state machines, protocols |
| **Exemplify** | Show the system in action | Worked examples, end-to-end traces |
| **Bound** | Set measurable limits | Performance budgets, design point |
| **Verify** | Define how to confirm correctness | Test strategies, quality gates, verification prompts (INV-019) |
| **Order** | Sequence implementation steps | Meta-instructions, phased roadmap (INV-020) |
| **Sequence** | Order the work | Phased roadmap, decision spikes, first PRs |
| **Lexicon** | Define terminology | Glossary |

## §0.3 Document Structure (Required)

A DDIS-conforming specification must contain the following structure. Sections may be renamed to fit the domain but the structural elements are mandatory unless explicitly marked [Optional] or [Conditional].

```
PREAMBLE
  Design goal (one sentence)
  Core promise (user-facing, one sentence)
  Document note (about code sketches and where correctness lives)
  How to use this plan (numbered practical steps)

PART 0: EXECUTIVE BLUEPRINT
  §0.1  Executive Summary
  §0.2  First-Principles Derivation (formal model)
          §0.2.3 LLM Consumption Model                          [Required]
  §0.3  Architecture Overview (rings, layers, or crate map)
  §0.4  Workspace / Module Layout
  §0.5  Invariants (numbered: INV-001, INV-002, ...)
  §0.6  Architecture Decision Records (ADR-001, ADR-002, ...)
  §0.7  Quality Gates (stop-ship criteria) + Definition of Done
  §0.8  Performance Budgets + Design Point
  §0.9  Public API Surface (target sketches)
  §0.10 Open Questions (resolve early, track as ADRs)           [Optional]
  §0.11 Non-Negotiables (engineering contract)
  §0.12 Non-Goals (explicit exclusions)
  §0.13 Modularization Protocol (specs > context window)        [Conditional]

PART I: FOUNDATIONS
  §1.1  Formal Model (state machines, types)
  §1.2  Completeness Properties
  §1.3  Complexity Analysis
  §1.4  End-to-End Trace (one scenario through all subsystems)  [Required]

PART II: CORE IMPLEMENTATION (per subsystem)
  [Each chapter ends with a Verification Prompt — INV-019]
  [Each chapter includes Negative Specifications — INV-017]

PART III: INTERFACES
PART IV: OPERATIONS
APPENDICES (Glossary, Risks, Error Taxonomy, Formats, Benchmarks)
PART X: MASTER TODO INVENTORY
```

### §0.3.1 Ordering Rationale

The ordering follows the **dependency chain of understanding**: first principles → invariants → ADRs → implementation → interfaces → operations → appendices. An implementer reading top-to-bottom builds understanding incrementally. No section requires forward references to be understood.

**LLM-specific ordering note (per L4):** When a section depends on content in a non-adjacent section, include an explicit forward or backward reference with the invariant/ADR ID. Never write "see above" — always write "see §X.Y" or "see INV-NNN."

## §0.4 This Standard's Architecture

DDIS has a simple ring architecture:

1. **Core Standard (sacred)**: The mandatory structural elements, their required contents, quality criteria, and relationships.
2. **Guidance (recommended)**: Voice, proportional weight, anti-patterns, worked examples. Absence does not make a spec non-conforming, but presence significantly improves LLM implementation quality.
3. **Tooling (optional)**: Checklists, templates, validation procedures.

## §0.5 Invariants of the DDIS Standard

### Base Invariants (INV-001 through INV-010)

**INV-001: Causal Traceability**
*Every implementation section traces to at least one ADR or invariant, which traces to the formal model.*
```
∀ section ∈ PART_II:
  ∃ adr ∈ ADRs ∪ inv ∈ Invariants:
    section.references(adr ∨ inv) ∧ (adr ∨ inv).derives_from(formal_model)
```
Violation scenario: An implementation chapter describes a caching layer but references no ADR or invariant — the reader cannot determine WHY caching was chosen or what properties it must preserve.
Validation: Pick 5 random implementation sections. Follow cross-references backward. If any chain breaks before reaching the formal model, INV-001 is violated.
// WHY THIS MATTERS: Without causal traceability, implementers (especially LLMs) cannot distinguish load-bearing design decisions from incidental ones, leading to incorrect "optimizations" that break invariants.

**INV-002: Decision Completeness**
*Every design choice where a reasonable alternative exists is captured in an ADR.*
Violation scenario: The spec prescribes event sourcing for persistence but no ADR explains why not CRUD — an implementer might "simplify" to CRUD, breaking replay determinism.
Validation: Adversarial review — for each implementation section, ask "could this reasonably be done differently?" If yes and no ADR covers the choice, INV-002 is violated.
// WHY THIS MATTERS: LLMs are especially prone to substituting familiar patterns for unfamiliar ones. Without an ADR, the LLM may "improve" the design by choosing a more common alternative.

**INV-003: Invariant Falsifiability**
*Every invariant can be violated by a concrete scenario and detected by a named test.*
```
∀ inv ∈ Invariants:
  ∃ scenario: scenario.violates(inv)
  ∧ ∃ test: test.detects(scenario)
```
Violation scenario: "INV-042: The system shall be performant." — No concrete violation exists because "performant" is undefined.
Validation: For each invariant, spend 60 seconds constructing a counterexample. If you cannot, the invariant is either trivially true (remove it) or unfalsifiable (rewrite it).
// WHY THIS MATTERS: Unfalsifiable invariants are specification noise — they consume context without constraining behavior.

**INV-004: Algorithm Completeness**
*Every described algorithm includes: pseudocode, complexity analysis, at least one worked example, and error/edge case handling.*
Violation scenario: The spec describes "efficient scheduling" but provides no pseudocode — the implementer must invent the algorithm, likely incorrectly.
Validation: Mechanical check: does each algorithm section contain all four components?
// WHY THIS MATTERS: LLMs perform dramatically better when given explicit algorithms vs. prose descriptions. Missing pseudocode is the #1 predictor of hallucinated implementations.

**INV-005: Performance Verifiability**
*Every performance claim is tied to a specific benchmark scenario, a design point, and a measurement methodology.*
Violation scenario: "The system should respond in under 100ms" — no design point (what hardware? what load?), no benchmark, no measurement method.
Validation: For each performance number, locate the corresponding benchmark and design point.
// WHY THIS MATTERS: Without concrete measurement criteria, LLMs may generate implementations that appear to meet performance targets but were never tested against them.

**INV-006: Cross-Reference Density**
*No section is an island — every non-trivial section has at least one inbound and one outbound reference.*
```
∀ section ∈ Sections where |section| > 5 lines:
  |inbound_refs(section)| ≥ 1 ∧ |outbound_refs(section)| ≥ 1
```
Violation scenario: A glossary term is defined but never referenced, or an implementation chapter references no invariant.
Validation: Build a directed reference graph of all sections. Any node with in-degree 0 or out-degree 0 (except the root Preamble) violates INV-006.
// WHY THIS MATTERS: Orphan sections are invisible to LLMs navigating by reference chains. Unconnected content is effectively absent.

**INV-007: Signal-to-Noise Ratio**
*Every section earns its place by serving at least one other section or preventing a named failure mode.*
Violation scenario: A 200-line section on "General Best Practices" that no other section references and prevents no specific failure.
Validation: For each section, state in one sentence why removing it would make the spec worse. If you cannot, the section violates INV-007.
// WHY THIS MATTERS: Every unnecessary line consumes context budget (per L2). Noise dilutes signal and increases the probability that the LLM loses track of critical invariants.

**INV-008: Self-Containment**
*The specification, combined with general programming competence, is sufficient to build a correct v1.*
Violation scenario: The spec references "the standard ReBAC authorization model" without defining it — an implementer unfamiliar with ReBAC cannot proceed.
Validation: Give spec to an unfamiliar implementer (or LLM). Track every question that reveals missing information. Each question is an INV-008 violation.
// WHY THIS MATTERS: LLMs cannot look things up mid-implementation (or if they can, doing so risks context loss). Every external dependency is a hallucination opportunity.

**INV-009: Glossary Coverage**
*Every domain-specific term used in the specification is defined in the glossary.*
Violation scenario: The spec uses "churn-magnet" repeatedly but the glossary omits it.
Validation: Extract all non-common-English terms; check each against the glossary.
// WHY THIS MATTERS: LLMs assign meanings to undefined terms based on training data, which may not match the spec's intended meaning.

**INV-010: State Machine Completeness**
*Every state machine defines all states, all transitions, all guards, and behavior for invalid transitions.*
Violation scenario: A spec defines states {Open, InProgress, Done} with transitions {Open→InProgress, InProgress→Done} but never specifies what happens if someone attempts Open→Done.
Validation: Enumerate the full state × event cross-product. Every cell must be filled (even if the fill is "rejected with error X").
// WHY THIS MATTERS: LLMs will generate handlers for defined transitions and silently ignore undefined ones, creating silent state corruption bugs.

### LLM-Specific Invariants (INV-017 through INV-020)

These invariants are motivated by the LLM Consumption Model (§0.2.3). They apply to all DDIS specs regardless of whether the intended implementer is an LLM, because they also improve human implementation quality — but they are REQUIRED specifically because LLMs are the primary consumer.

**INV-017: Negative Specification Coverage**
*Every element specification in a DDIS-conforming spec includes explicit "do NOT" constraints — anti-patterns and negative requirements that prevent the most likely misimplementations.*

```
∀ element_spec ∈ PART_II:
  |negative_constraints(element_spec)| ≥ 1
```

Violation scenario: An implementation chapter for a scheduler specifies the algorithm but never says "do NOT use priority inversion-prone locking" — the LLM generates a textbook implementation with a known priority inversion bug.
Validation: For each implementation chapter, count negative constraints. If zero, INV-017 is violated.
// WHY THIS MATTERS: Principle L1 — LLMs fill gaps with plausible content. Negative specifications close the most dangerous gaps. [Derives from §0.2.3 Principle L1.]

**INV-018: Structural Redundancy at Point of Use**
*Key invariants and constraints are restated (in compact reminder form) at every point where they govern implementation behavior, not solely at their point of definition.*

Violation scenario: INV-003 (falsifiability) is defined in §0.5 but the implementation chapter template in §5.1 does not remind the author that every invariant defined within a subsystem must include a violation scenario — the author (or LLM) forgets.
Validation: For each implementation chapter, list the invariants it must satisfy. Check that each is either defined in the chapter or referenced by a compact reminder.
// WHY THIS MATTERS: Principle L2 — invariants at the top of a long document lose salience. Compact reminders prevent context decay. [Derives from §0.2.3 Principle L2.]

**INV-019: Verification Prompts**
*Every implementation chapter ends with a self-check prompt: a checklist that allows an implementer (human or LLM) to verify their output against the spec's requirements for that chapter.*

```
∀ chapter ∈ PART_II:
  chapter.ends_with(verification_prompt)
  ∧ |verification_prompt.items| ≥ 3
```

Violation scenario: An LLM implements the event store chapter but there is no verification checklist — the LLM cannot self-evaluate whether it handled append-only invariants, compaction rules, and replay determinism.
Validation: Check each implementation chapter for a terminal verification prompt section with at least 3 checklist items.
// WHY THIS MATTERS: Principle L3 — LLMs benefit from self-evaluation criteria. Without them, errors propagate silently into subsequent chapters. [Derives from §0.2.3 Principle L3.]

**INV-020: Implementation Meta-Instructions**
*When implementation chapters have ordering dependencies, the spec contains explicit meta-instructions stating the required order and the reason.*

Violation scenario: Chapter 7 (Scheduler) uses types defined in Chapter 4 (EventStore), but no meta-instruction states this dependency — the LLM implements the Scheduler first and invents incompatible types.
Validation: Build a dependency graph of implementation chapters. Every edge must correspond to an explicit meta-instruction in the spec.
// WHY THIS MATTERS: Principle L4 — LLMs implement in document order unless directed otherwise. Unstated dependencies produce integration failures. [Derives from §0.2.3 Principle L4.]

### Modularization Invariants (INV-011 through INV-016)

These invariants apply only when a spec uses the modularization protocol (§0.13). They are declared here for completeness; full definitions are in the modularization-protocol module.

**INV-011: Module Completeness** — An LLM receiving a properly assembled bundle can implement the module's subsystem without information from any other module.
Violation: Module references another module's internal data structure not in the constitution.
Validation: Give bundle to LLM; track questions requiring other modules' content. (INV-008 applied per-bundle.)

**INV-012: Cross-Module Isolation** — Modules reference each other only through constitutional elements, never direct internal references.
Violation: "Use the same batching strategy as EventStore's flush_batch()."
Validation: Search module text for references to other modules' internal sections.

**INV-013: Invariant Ownership Uniqueness** — Every invariant is maintained by exactly one module (or the system constitution).
Violation: Two modules both list APP-INV-017 in their `maintains` field.
Validation: Parse manifest; check for duplicate ownership.

**INV-014: Bundle Budget Compliance** — Every assembled bundle fits within the manifest's hard ceiling line budget.
Violation: Constitution + module exceeds 5,000 lines.
Validation: Count lines after assembly.

**INV-015: Declaration-Definition Consistency** — Tier 1 declarations faithfully summarize their Tier 2 full definitions.
Violation: Declaration says "append-only" but the full definition now allows compaction.
Validation: Side-by-side comparison of each declaration against its definition.

**INV-016: Manifest-Spec Synchronization** — The manifest accurately reflects the current state of all spec files.
Violation: A new module file exists but is not listed in the manifest.
Validation: Compare manifest entries against filesystem.

## §0.6 Architecture Decision Records

### ADR-001: Document Structure Is Fixed, Not Flexible

**Problem:** Should DDIS prescribe a fixed document structure or allow authors to choose their own organization?

**Options:**
| Option | Pros | Cons |
|--------|------|------|
| A. Fixed structure | Predictability; any DDIS reader can navigate any DDIS spec; LLMs benefit from structural consistency | Less flexibility for unusual domains |
| B. Flexible with guidelines | Adaptable to domain | Every spec is organized differently; readers must re-learn navigation; LLMs cannot leverage structural expectations |
| C. Menu of approved structures | Some variety with some predictability | Combinatorial explosion; validation complexity; LLMs must pattern-match against multiple structures |

**Decision:** Fixed structure (Option A). The value of DDIS is that a reader (human or LLM) who has seen one DDIS spec can navigate any other. Sections may be renamed to fit the domain but structural elements and ordering are mandatory.
// WHY NOT flexible? Because the #1 failure mode of flexible standards is that each author reinvents the wheel. LLMs especially benefit from consistent structure (per L2).
**Consequences:** Some domain specs will have thin sections. This is acceptable — a thin section signals "this system is simple in this dimension," which is valuable information.
**Validates:** INV-001, INV-006.

### ADR-002: Invariants Must Be Falsifiable, Not Merely True

**Problem:** What formality level should invariants have?

**Options:**
| Option | Pros | Cons |
|--------|------|------|
| A. Natural language only | Easy to write | Ambiguous; untestable; LLMs interpret loosely |
| B. Full formal verification (TLA+/Alloy) | Machine-checkable | Excludes most authors; opaque to LLMs without tooling |
| C. Falsifiable with semi-formal expression | Testable; readable; each includes violation scenario | More work per invariant |

**Decision:** Falsifiable invariants (Option C). Each includes: plain-language statement, semi-formal expression, violation scenario, validation method, and WHY THIS MATTERS annotation.
// WHY NOT full formal verification? Because the goal is implementation correctness by humans and LLMs, not machine-checked proofs. The violation scenario is the key — it forces the author to think concretely about what "wrong" looks like.
**Validates:** INV-003.

### ADR-003: Cross-References Are Mandatory, Not Optional Polish

**Problem:** Should cross-references between sections be required or recommended?

**Options:**
| Option | Pros | Cons |
|--------|------|------|
| A. Optional (nice to have) | Less authoring overhead | Orphan sections; broken context for LLMs |
| B. Required, enforced by validation | Connected graph; LLMs can navigate; no orphans | More authoring work |

**Decision:** Required (Option B). Every non-trivial section must have inbound and outbound references.
// WHY NOT optional? Because orphan sections are invisible to LLMs navigating the spec by reference chains (INV-006). A section that nothing references is effectively absent.
**Validates:** INV-006.

### ADR-004: Self-Bootstrapping as Validation Strategy

**Problem:** How should DDIS validate its own completeness?

**Options:**
| Option | Pros | Cons |
|--------|------|------|
| A. External test suite | Independent validation | May drift from the standard |
| B. Self-bootstrapping (eat your own cooking) | Standard is both definition and first test case | Recursive complexity |

**Decision:** Self-bootstrapping (Option B). This document is both the standard and its first conforming instance. Where DDIS prescribes an element, that element is demonstrated within this document.
// WHY NOT external test suite only? Because a standard that cannot pass its own criteria has no credibility. Self-bootstrapping is the ultimate dogfooding.
**Validates:** The entire standard.

### ADR-005: Voice Is Specified, Not Left to Author Preference

**Problem:** Should DDIS prescribe the writing style of conforming specs?

**Options:**
| Option | Pros | Cons |
|--------|------|------|
| A. No voice guidance | Maximum freedom | Inconsistent quality; LLMs produce bureaucratic prose |
| B. Voice guidance specified | Consistent, readable specs | Constrains author expression |

**Decision:** Voice guidance specified (Option B). The voice of a senior engineer explaining to a peer they respect — technically precise but human.
// WHY NOT no guidance? Because LLMs default to bureaucratic or academic prose without explicit voice direction. Specified voice dramatically improves LLM-authored spec quality.
**Validates:** INV-007.

### ADR-008: Negative Specifications Required Per Element

**Problem:** Should DDIS require explicit "do NOT" constraints (negative specifications) for each element, or leave this to author judgment?

**Options:**
| Option | Pros | Cons |
|--------|------|------|
| A. Optional (author discretion) | Less overhead | LLMs hallucinate when gaps exist (per L1) |
| B. Required per implementation chapter | Prevents most common LLM failure mode | More authoring work per element |
| C. Required only for security-critical elements | Balanced | Inconsistent; LLMs hallucinate in non-security contexts too |

**Decision:** Required per implementation chapter (Option B). Every implementation chapter must include at least one negative specification.
// WHY NOT optional? Because Principle L1 (minimize hallucination gap) demonstrates that LLMs fill every gap with plausible content. Negative specs close the most dangerous gaps. The authoring cost (~5 minutes per chapter) is trivial compared to the debugging cost of a hallucinated implementation.
**Derives from:** §0.2.3 Principle L1.
**Validates:** INV-017.

### ADR-009: LLM Provisions Woven Throughout, Not Isolated

**Problem:** Where should LLM-specific structural provisions live in the standard?

**Options:**
| Option | Pros | Cons |
|--------|------|------|
| A. Single "LLM Considerations" appendix | Easy to find; easy to skip | Bolt-on feel; provisions not enforced by element specs; LLM optimization becomes optional |
| B. Woven into every element specification | LLM optimization is structural, not optional; each element spec explicitly addresses LLM failure modes | More complex element specs |
| C. Separate "LLM Profile" that overlays the base standard | Modular; can evolve independently | Two documents to maintain; inconsistency risk |

**Decision:** Woven throughout (Option B). LLM provisions appear in: the formal model (§0.2.3), invariants (INV-017–020), quality gates (Gate 7), element specifications (negative specs, verification prompts, meta-instructions per element), and the voice guide (LLM-specific prose patterns).
// WHY NOT appendix? Because an appendix is by definition skippable. LLM optimization is the primary design goal of DDIS — it must be structural, not supplementary. The Afterthought LLM Section is an explicit anti-pattern (see Appendix C).
**Derives from:** §0.2.3 (all principles).
**Validates:** INV-017, INV-018, INV-019, INV-020.

### ADR-010: Structural Redundancy Over DRY for Invariants

**Problem:** Should invariants be stated only at their definition site (DRY principle) or restated at points of use?

**Options:**
| Option | Pros | Cons |
|--------|------|------|
| A. Define once, reference always | DRY; single source of truth | LLMs lose context (per L2); invariants invisible at implementation point |
| B. Full repetition at every use site | Always visible | Maintenance nightmare; inconsistency risk |
| C. Compact reminder at use sites, full definition at source | Visible where needed; single authoritative source; low maintenance cost | Slightly more verbose |

**Decision:** Compact reminder at use sites (Option C). Format: `// REMINDER: INV-NNN — [one-line restatement]`. The full definition remains in §0.5. Reminders are not authoritative — if they drift from the definition, the definition wins.
// WHY NOT full DRY? Because Principle L2 shows that LLMs lose context in long documents. A 5,000-line spec where invariants appear only in §0.5 is a spec where the LLM forgets critical constraints by §7.
**Derives from:** §0.2.3 Principle L2.
**Validates:** INV-018.

### ADR-011: Verification Prompts Per Implementation Chapter

**Problem:** Should implementation chapters include self-verification checklists?

**Options:**
| Option | Pros | Cons |
|--------|------|------|
| A. No (trust the implementer) | Shorter chapters | LLMs cannot self-evaluate; errors propagate |
| B. Verification prompt per chapter | LLM self-evaluation; catch errors early | ~10 lines per chapter overhead |
| C. Single verification checklist at end of spec | Centralized | Too late; LLM has already generated all chapters; no incremental checking |

**Decision:** Verification prompt per chapter (Option B). Each chapter ends with a prompt like: "Before proceeding, verify: (1) all types match the formal model in §1.1, (2) INV-003 is satisfied for each invariant defined here, (3) no algorithm lacks a worked example."
// WHY NOT end-of-spec checklist? Because LLMs generate sequentially. An error in Chapter 4 that isn't caught until a checklist at Chapter 15 means Chapters 5–14 are built on a broken foundation.
**Derives from:** §0.2.3 Principle L3.
**Validates:** INV-019.

### ADR-006: Tiered Constitution over Flat Root

**Decision:** Three-tier as full protocol, two-tier as blessed simplification for small specs (< 20 invariants, ≤ 400-line system constitution).
// WHY NOT flat root? At scale, it consumes 30–37% of context budget before the module starts.
**Validates:** INV-014.
*Full details in modularization-protocol module.*

### ADR-007: Cross-Module References Through Constitution Only

**Decision:** Module A references invariants in the constitution, never Module B's internal sections.
// WHY NOT direct references? Breaks INV-011 — Module A's bundle would need Module B's implementation details.
**Validates:** INV-012.
*Full details in modularization-protocol module.*

## §0.7 Quality Gates

**Gate 1: Structural Conformance** — All required elements from §0.3 present, including §0.2.3 LLM Consumption Model.
*Measurement:* Checklist comparison of actual sections against §0.3 template.

**Gate 2: Causal Chain Integrity** — Five random implementation sections trace backward to the formal model without breaks.
*Measurement:* Pick 5 sections, follow cross-references. Zero broken chains. (Validates INV-001.)

**Gate 3: Decision Coverage** — Zero "obvious alternatives" not covered by an ADR.
*Measurement:* Adversarial review of each implementation chapter. (Validates INV-002.)

**Gate 4: Invariant Falsifiability** — Every invariant has a constructible counterexample and a named test.
*Measurement:* Attempt to construct a counterexample for each invariant within 60 seconds. (Validates INV-003.)

**Gate 5: Cross-Reference Web** — Reference graph is connected; no orphan sections.
*Measurement:* Build directed graph of all section references. Zero nodes with in-degree 0 (except Preamble) or out-degree 0. (Validates INV-006.)

**Gate 6: Implementation Readiness** — An implementer can begin work without asking clarifying questions about architecture, algorithms, data models, or invariants.
*Measurement:* Give spec to unfamiliar implementer/LLM. Track questions. Zero architectural questions. (Validates INV-008.)

**Gate 7: LLM Implementation Readiness** — An LLM receiving the spec (or a bundle from a modularized spec) can implement the described system without hallucinating architectural decisions, inventing undefined types, or violating stated invariants.
*Measurement procedure:*
1. Give the spec (or one bundle) to an LLM with no other context.
2. Ask the LLM to implement the first subsystem.
3. Check: (a) Did the LLM invent any types not in the spec? (b) Did the LLM make any design decisions not covered by ADRs? (c) Did the LLM's output satisfy all applicable invariants? (d) Did the LLM use the verification prompt to self-check?
4. If any answer to (a), (b), or (c) is "yes," or (d) is "no," Gate 7 fails.
*Falsification scenario:* Give spec to LLM without negative specifications. LLM generates a plausible but incorrect caching layer not mentioned in the spec. Gate 7 catches this because the hallucinated component violates "no types not in the spec."
(Validates INV-017, INV-018, INV-019, INV-020.)

### Definition of Done (for this standard)

DDIS 2.0 is "done" when:
- This document passes Gates 1–7 applied to itself
- At least one non-trivial spec has been written conforming to DDIS 2.0
- The glossary covers all DDIS-specific terminology
- The end-to-end trace (§1.4) demonstrates a complete element lifecycle

## §0.8 Performance Budgets

### Specification Size Budgets

| System Complexity | Target Spec Length | Rationale |
|---|---|---|
| Small (< 5K LOC target) | 500–1,500 lines | Formal model + invariants + key ADRs |
| Medium (5K–50K LOC target) | 1,500–5,000 lines | Full DDIS treatment |
| Large (> 50K LOC target) | 5,000–15,000 lines | Should use modularization protocol (§0.13) |

### Proportional Weight Guide

| Section | % of Total | Why |
|---|---|---|
| Preamble + PART 0 | 15–20% | Dense: formal model, invariants, ADRs, quality gates, LLM consumption model |
| PART I: Foundations | 8–12% | First principles, state machines, complexity analysis, end-to-end trace |
| PART II: Core Implementation | 35–45% | THE HEART — includes negative specs and verification prompts per chapter |
| PART III: Interfaces | 8–12% | API schemas, adapters |
| PART IV: Operations | 10–15% | Testing, operational playbook |
| Appendices + Part X | 10–15% | Glossary, error taxonomy, reference material |

### Authoring Time Budgets

| Element | Expected Time | Notes |
|---|---|---|
| First-principles model | 2–4 hours | Hardest part — derive, don't assert |
| LLM consumption model (§0.2.3) | 1–2 hours | Identify domain-specific LLM failure modes |
| One invariant | 15–30 minutes | Including violation scenario and WHY THIS MATTERS |
| One ADR | 30–60 minutes | Including genuine alternative analysis |
| One implementation chapter | 2–4 hours | Including algorithm, examples, negative specs, verification prompt |
| End-to-end trace | 1–2 hours | Requires all subsystems drafted first |
| Glossary | 1–2 hours | Best done last |

### Measurement Harness

**How to measure "time to first question from an implementer":** Give the spec to an LLM with the instruction "implement subsystem X; ask a question if anything is ambiguous." Measure tokens generated before the first question. Target: the LLM produces a complete implementation without asking. Each question identifies a gap in the spec; patch the gap and re-test.

## §0.9 Public API Surface

DDIS exposes:
- **Document Structure Template** (§0.3) — the mandatory skeleton
- **Element Specifications** (PART II of this standard) — what each structural element must contain
- **Quality Criteria** (§0.5 invariants, §0.7 gates) — how to validate
- **LLM Consumption Model** (§0.2.3) — how to optimize for LLM implementers
- **Voice and Style Guide** (Ch. 8) — how to write
- **Anti-Pattern Catalog** (Ch. 8.3, Appendix C) — how NOT to write
- **Completeness Checklist** (Part X) — execution tracking

## §0.10 Open Questions

[Currently none. All open questions from DDIS 1.0 have been resolved in 2.0 via ADRs 008–011 and INV-017–020.]

---

# PART I: FOUNDATIONS

## §1.1 A Specification as a State Machine

[SELF-BOOTSTRAP: This section demonstrates INV-010 compliance — all states, transitions, guards, and invalid transition policy are defined.]

```
States: {Skeleton, Drafted, Threaded, Gated, Validated, Living}

Transitions:
  Skeleton  → Drafted    [guard: all required sections from §0.3 have content]
  Drafted   → Threaded   [guard: cross-reference graph is connected (INV-006)]
  Threaded  → Gated      [guard: Gates 1–5 pass]
  Gated     → Validated   [guard: Gates 6–7 pass; external LLM test completed]
  Validated → Living      [guard: implementation has begun]
  Living    → Drafted     [guard: gap discovered requiring structural changes]

Invalid transitions (all others):
  Skeleton  → Threaded   REJECTED: Cannot have cross-refs without content
  Skeleton  → Gated      REJECTED: Cannot pass gates without threading
  Skeleton  → Validated   REJECTED: Cannot validate empty spec
  Drafted   → Gated      REJECTED: Must thread cross-refs first
  Drafted   → Validated   REJECTED: Must pass gates first
  Threaded  → Validated   REJECTED: Must pass gates first
  Threaded  → Living      REJECTED: Must pass gates and validate first
  Gated     → Living      REJECTED: Must validate (external check) first
  Living    → Skeleton    REJECTED: Cannot regress past Drafted
  Any       → Skeleton    REJECTED: Skeleton is entry-only (except initial creation)

  On attempted invalid transition: log warning with current state, attempted
  target, and missing guard condition. Do not silently ignore.
```

### State × Event Table

| Current State | Add Content | Thread Cross-Refs | Run Gates 1–5 | Run Gates 6–7 | Begin Impl | Discover Gap |
|---|---|---|---|---|---|---|
| Skeleton | → Drafted (if guard met) | REJECTED | REJECTED | REJECTED | REJECTED | N/A |
| Drafted | Stay Drafted | → Threaded (if guard met) | REJECTED | REJECTED | REJECTED | Stay Drafted |
| Threaded | Stay Threaded | Stay Threaded | → Gated (if pass) | REJECTED | REJECTED | → Drafted |
| Gated | Stay Gated | Stay Gated | Stay Gated | → Validated (if pass) | REJECTED | → Drafted |
| Validated | Stay Validated | Stay Validated | Stay Validated | Stay Validated | → Living | → Drafted |
| Living | Stay Living | Stay Living | Stay Living | Stay Living | Stay Living | → Drafted |

## §1.2 Completeness Properties

**Safety**: No contradictory prescriptions between sections. If section A says "use X" and section B says "use Y" for the same concern, one must be removed or reconciled via ADR.

**Liveness**: The spec eventually answers every architectural question an implementer would ask. Tracked by Gate 6 (implementation readiness) and Gate 7 (LLM implementation readiness).

## §1.3 Complexity of Specification Elements

| Element | Authoring Complexity | Reading Complexity | Verification Complexity |
|---|---|---|---|
| Invariant | O(domain_understanding) | O(1) | O(1) per counterexample |
| ADR | O(alternatives × depth) | O(alternatives) | O(1) per option |
| Algorithm | O(complexity × edge_cases) | O(pseudocode_length) | O(worked_examples) |
| Negative spec | O(failure_modes) | O(1) | O(1) per constraint |
| Verification prompt | O(invariants_per_chapter) | O(1) | O(checklist_items) |
| Cross-reference | O(1) | O(1) | O(sections²) for full graph |
| End-to-end trace | O(subsystems × interactions) | O(subsystems) | O(1) |

## §1.4 End-to-End Trace

[SELF-BOOTSTRAP: This section demonstrates the §5.3 element specification for end-to-end traces. It traces a single ADR through the entire DDIS authoring process.]

**Scenario:** An author writing a DDIS-conforming spec for a task scheduler recognizes a design decision: should tasks be scheduled FIFO or by priority?

**Step 1: Recognition (§0.2 → §3.5)**
Author recognizes two reasonable alternatives exist. Per INV-002 (decision completeness), this requires an ADR. Author creates ADR-NNN using the template from §3.5.

**Step 2: Analysis (§3.5)**
Author fills in Options table with genuine alternatives:
- Option A: FIFO — simple, predictable, no starvation
- Option B: Priority queue — responsive to urgency, risk of starvation
Per ADR anti-pattern check: both options have genuine advocates (a real-time system engineer would choose B; a batch system engineer would choose A).

**Step 3: Decision + WHY NOT (§5.4)**
Author decides: Priority queue with starvation prevention (age-based priority boost).
`// WHY NOT FIFO? Because the design point (§0.8) requires 95th-percentile latency < 50ms for high-priority tasks, incompatible with FIFO under load.`

**Step 4: Invariant derivation (§3.4)**
The decision implies an invariant: `INV-NNN: No task waits more than K scheduling cycles without execution.`
This invariant has: plain-language statement, semi-formal expression (`∀ task ∈ ready_queue: task.wait_cycles ≤ K`), violation scenario (a low-priority task starves for K+1 cycles), validation method (property test with random priority distributions).
// REMINDER: INV-003 — Every invariant must have a concrete violation scenario.

**Step 5: Implementation chapter (§5.1)**
The scheduler implementation chapter references ADR-NNN and INV-NNN. It includes:
- Pseudocode for priority queue with age boost
- Worked example with 5 tasks at different priorities
- Negative specification: "Do NOT use a simple priority queue without starvation prevention — this violates INV-NNN."
- Verification prompt: "Verify: (1) Does your implementation boost priority after K cycles? (2) Can you construct a scenario where a task starves? If yes, INV-NNN is violated."

**Step 6: Test strategy (§6.2)**
Property test: generate 10,000 random task streams, assert no task exceeds K wait cycles.

**Step 7: Gate validation (§0.7)**
- Gate 2: Trace scheduler chapter → ADR-NNN → INV-NNN → formal model. Chain unbroken. ✓
- Gate 3: FIFO alternative covered by ADR-NNN. ✓
- Gate 4: Starvation scenario violates INV-NNN. ✓
- Gate 7: LLM given the scheduler chapter produces implementation with age boost and starvation prevention. No hallucinated types. ✓

**Invariants exercised:** INV-001 (traceability), INV-002 (decision completeness), INV-003 (falsifiability), INV-004 (algorithm completeness), INV-017 (negative spec), INV-019 (verification prompt).

---

# PART II: CORE STANDARD — Element Specifications

Each section specifies one structural element: what it must contain, quality criteria, what good vs. bad looks like, negative specifications (what it must NOT be — per INV-017), and cross-references.

// REMINDER: INV-017 — Every element specification must include explicit "do NOT" constraints.
// REMINDER: INV-006 — Every non-trivial section must have inbound and outbound references.

## Chapter 2: Preamble Elements

### §2.1 Design Goal

**What it is**: A single sentence (≤ 30 words) stating the system's reason for existing.

**Required properties**: States core value proposition (not implementation). Uses bold for 3–5 key properties. Readable by non-technical stakeholder.

**Quality criteria**: A reader seeing only the design goal can decide relevance.

**Negative specification (do NOT):**
- Do NOT describe implementation ("Build a distributed task coordination system using event sourcing") — describe value.
- Do NOT exceed 30 words — this is a hook, not a summary.
- Do NOT use buzzwords ("enterprise-grade", "scalable", "robust") — use specific properties.

**Good example**: "Design goal: **scrollback-native, zero-flicker, agent-ergonomic, and high-performance** Rust terminal apps."
**Bad example**: "Build a distributed task coordination system using event sourcing and advisory reservations." ← Describes implementation, not value.

**Cross-references**: Each bolded property should correspond to at least one invariant and one quality gate.

### §2.2 Core Promise

**What it is**: A single sentence (≤ 40 words) describing capabilities from the user's perspective.

**Required properties**: User's viewpoint. Concrete capabilities. Uses "without" clauses to highlight what isn't sacrificed.

**Negative specification (do NOT):**
- Do NOT use the implementer's perspective — this is for users.
- Do NOT use meaningless buzzwords ("robust, scalable, enterprise-grade").

**Bad example**: "The system provides robust, scalable, enterprise-grade coordination." ← Meaningless.

### §2.3 Document Note

**What it is**: 2–4 sentence disclaimer about code blocks and where correctness lives.

**Template**:
> Code blocks are **design sketches**. The correctness contract lives in the invariants, tests, and ADRs — not in pseudo-code syntax.

**Negative specification (do NOT):**
- Do NOT omit this section — LLMs may treat pseudocode as literal implementation requirements without it.

### §2.4 How to Use This Plan

**What it is**: 4–6 item numbered list with practical reading/execution guidance. Must start with "Read PART 0 end-to-end," identify churn-magnets, point to Master TODO, identify at least one non-negotiable process requirement.

**Negative specification (do NOT):**
- Do NOT write generic advice ("read carefully") — every item must reference specific sections or elements.

## Chapter 3: PART 0 Elements

### §3.1 Non-Negotiables (Engineering Contract)

5–10 properties defining what the system IS. Stronger than invariants — philosophical commitments that must never be compromised.

**Format**: `- **[Property name]** — [One concrete sentence]. (Validated by INV-NNN, Gate N.)`

**Quality criteria**: An implementer could imagine a tempting violation scenario; the non-negotiable clearly says no. Not a restatement of a technical invariant — it's the "why" that justifies groups of invariants.

**Negative specification (do NOT):**
- Do NOT write non-negotiables that cannot be violated ("The system exists") — if nobody would tempt the alternative, it's not a non-negotiable.
- Do NOT duplicate invariants — non-negotiables are the philosophical layer ABOVE invariants.

### §3.2 Non-Goals

5–10 explicit exclusions. The immune system against scope creep.

**Quality criteria**: Someone has actually asked for this (not absurd exclusions). Brief explanation of why excluded.

**Negative specification (do NOT):**
- Do NOT include absurd exclusions nobody would request ("Non-goal: Building a quantum computer").
- Do NOT omit this section — LLMs are especially prone to scope creep without explicit boundaries.

### §3.3 First-Principles Derivation

The formal model making the architecture feel *inevitable* rather than *asserted*.

**Required**: (1) Mathematical system definition as state machine or function. (2) 3–5 consequence bullets. (3) Fundamental operations table.

**Quality criteria**: After reading, an implementer could derive the architecture independently.

**Negative specification (do NOT):**
- Do NOT assert architecture without deriving it — "We use microservices" without explaining WHY from first principles violates INV-001.
- Do NOT use a formalism the domain doesn't warrant — a CRUD app doesn't need temporal logic.

### §3.4 Invariants

Numbered properties that must hold at all times.

**Required format**:
```
**INV-NNN: [Name]**
*[Plain-language statement]*
  [Semi-formal expression]
Violation scenario: [Concrete description]
Validation: [Named test strategy]
// WHY THIS MATTERS: [Consequences of violation]
```

**Quality criteria**: Falsifiable (constructible counterexample). Consequential (violation causes bad behavior). Non-trivial (not a type constraint). Testable.

**Quantity**: 10–25 for medium-complexity systems.

**Negative specification (do NOT):**
- Do NOT write unfalsifiable invariants: "The system shall be performant." ← What scenario violates this?
- Do NOT write trivially-true invariants: "TaskId values are unique." ← Enforced by type system; not worth an invariant.
- Do NOT omit violation scenarios — an invariant without a violation scenario violates INV-003.
- Do NOT omit the WHY THIS MATTERS annotation — without it, implementers (especially LLMs) cannot prioritize invariants.

### §3.5 Architecture Decision Records

**Required format**: Problem → Options (≥2, ≤4, genuine alternatives) → Decision with WHY NOT → Consequences → Tests.

**Quality criteria**: Genuine alternatives (a competent engineer would choose each in some context). Concrete tradeoffs (specific, measurable). Consequential decision (> 1 day refactoring to change).

**Negative specification (do NOT):**
- Do NOT include strawman options nobody would choose. **The Strawman ADR** is the single most common ADR defect.
- Do NOT write ADRs for trivial decisions (variable naming, formatting) — only for decisions where the alternative would change system behavior.
- Do NOT omit WHY NOT annotations — they are the most valuable part of an ADR for LLM implementers, because they prevent the LLM from "improving" the design by choosing the rejected alternative.

**Churn-magnets**: After all ADRs, identify which decisions cause the most downstream rework if changed. These are the ones to validate earliest.

### §3.6 Quality Gates

4–8 stop-ship predicates, ordered by priority. Each references specific invariants/tests. Failing Gate N makes Gate N+1 irrelevant.

**Required**: Each gate has a name, pass criteria, measurement procedure (how to actually test it), and the invariants it validates.

**Negative specification (do NOT):**
- Do NOT write gates without measurement procedures — "Gate: the system is fast" cannot be evaluated.
- Do NOT write more than 8 gates — diminishing returns; focus on the critical path.

### §3.7 Performance Budgets and Design Point

**Required**: (1) Design point (hardware, workload, scale). (2) Budget table: operation → target → measurement. (3) Measurement harness description. (4) Adjustment guidance.

**Negative specification (do NOT):**
- Do NOT write aspirational budgets without a design point: "The system should respond in under 100ms" — on what hardware? Under what load?
- Do NOT omit the measurement harness — a budget without a measurement method is a wish, not a budget. (Violates INV-005.)

### §3.8 Negative Specifications

[NEW IN DDIS 2.0 — Required element per INV-017, ADR-008]

**What it is**: Explicit constraints on what the system (or element) must NOT do or be. Each negative specification names the failure mode it prevents and the invariant it protects.

**Required format**:
```
NEGATIVE: [What must NOT happen]
Prevents: [Named failure mode]
Protects: INV-NNN
```

**Quality criteria**: Each negative specification prevents a failure mode that an implementer (especially an LLM) might plausibly produce. Not paranoid — targeted at the most likely misimplementations.

**Where they appear**: (1) In each implementation chapter. (2) In each element specification (like this one). (3) In module headers (for modularized specs).

**Negative specification (do NOT):**
- Do NOT write paranoid negative specs: "Do NOT use the system to launch nuclear weapons." ← Nobody was going to.
- Do NOT duplicate positive requirements: if INV-003 says "invariants must be falsifiable," the negative spec should NOT be "do not write unfalsifiable invariants" — it should add NEW information: "do not confuse type-system guarantees with invariants."
- Do NOT write more than 5 negative specs per implementation chapter — diminishing returns, context budget.

**Cross-references**: INV-017, ADR-008, §0.2.3 Principle L1.

## Chapter 4: PART I Elements

### §4.1 Full Formal Model
Expanded first-principles derivation: complete state, input/event taxonomy, output/effect taxonomy, transition semantics, composition rules.

### §4.2 State Machines
Every stateful component gets: state diagram, state × event table (no empty cells per INV-010), guard conditions, invalid transition policy, entry/exit actions.

**Negative specification (do NOT):**
- Do NOT leave cells empty in the state × event table — every state × event pair must have a defined response, even if the response is "REJECTED: invalid transition."
- Do NOT omit invalid transition policy — LLMs will generate code that silently ignores invalid transitions, creating state corruption.

### §4.3 Complexity Analysis
Big-O bounds with constants where they matter for the design point.

## Chapter 5: PART II Elements

### §5.1 Implementation Chapters

One chapter per major subsystem. **Required components** (12 items — 10 base + 2 LLM-specific):

1. Purpose statement (2–3 sentences, references formal model)
2. Formal types with memory layout analysis
3. Algorithm pseudocode with inline complexity
4. State machine (if stateful) — per INV-010
5. Invariants preserved (INV-NNN list)
6. Worked example(s) with specific values — per §5.2
7. Edge cases and error handling
8. Test strategy (unit, property, integration, replay, stress)
9. Performance budget (subsystem's share)
10. Cross-references (ADRs, invariants, other subsystems, formal model)
11. **Negative specifications** — at least one "do NOT" constraint per INV-017
12. **Verification prompt** — self-check checklist per INV-019

// REMINDER: INV-004 — Every algorithm needs pseudocode, complexity, example, and edge cases.
// REMINDER: INV-017 — At least one negative specification per chapter.
// REMINDER: INV-019 — Ends with verification prompt.

**Quality criteria**: An implementer could build the subsystem from this chapter alone (plus the formal model and relevant invariants).

**Negative specification (do NOT):**
- Do NOT write chapters without pseudocode — prose descriptions of algorithms are the #1 cause of LLM implementation divergence.
- Do NOT write chapters that depend on other chapters without explicit meta-instructions (per INV-020).

### §5.2 Worked Examples

Concrete scenarios with specific values (not variables). Shows state before, operation, state after. Includes at least one non-trivial aspect (edge case, concurrent operation, error recovery).

**Negative specification (do NOT):**
- Do NOT write abstract examples with variables: "When a task is completed, the scheduler updates the DAG." ← No concrete values, no before/after.
- Do NOT use only happy-path examples — include at least one edge case or error scenario per example set.

### §5.3 End-to-End Trace

Single scenario traversing ALL subsystems. Shows exact data at each boundary. Identifies invariants exercised at each step. Includes at least one cross-subsystem interaction that could go wrong.

**Negative specification (do NOT):**
- Do NOT trace only the happy path — the trace must exercise at least one error handling or edge case path.
- Do NOT use abstract data — every value must be concrete.

**Cross-references**: Validated by INV-001. See §1.4 for a worked example of an end-to-end trace.

### §5.4 WHY NOT Annotations

Inline comments explaining the road not taken. Use when an implementer might think "I can improve this by doing X" and X was considered and rejected.

Format: `// WHY NOT [alternative]? [Brief tradeoff. Reference ADR-NNN if exists.]`

If annotation grows beyond 3 lines, promote to ADR.

### §5.5 Comparison Blocks

Side-by-side ❌ SUBOPTIMAL vs ✅ CHOSEN with quantified reasoning.

### §5.6 Verification Prompts

[NEW IN DDIS 2.0 — Required element per INV-019, ADR-011]

**What it is**: A checklist at the end of each implementation chapter that allows an implementer to self-verify their output.

**Required format**:
```
### Verification Prompt
Before proceeding to the next chapter, verify:
- [ ] All types match the formal model defined in §X.Y
- [ ] INV-NNN is satisfied: [specific check for this chapter]
- [ ] At least one worked example exercises the primary algorithm
- [ ] Negative specifications are not violated by the implementation
- [ ] [Domain-specific check for this subsystem]
```

**Quality criteria**: At least 3 items. Each item is mechanically checkable (not "code is good"). Items reference specific invariants or sections.

**Negative specification (do NOT):**
- Do NOT write vague checks: "Verify the implementation is correct." ← Not mechanically checkable.
- Do NOT duplicate the quality gates — verification prompts are chapter-scoped, gates are spec-scoped.
- Do NOT make prompts optional — they are required per INV-019.

**Cross-references**: INV-019, ADR-011, §0.2.3 Principle L3.

### §5.7 Implementation Meta-Instructions

[NEW IN DDIS 2.0 — Required element per INV-020]

**What it is**: Explicit directives about implementation ordering, sequencing constraints, and dependency chains between chapters.

**Required format**:
```
META-INSTRUCTION: Implement §X before §Y because [reason — typically a type or invariant dependency].
```

**Where they appear**: At the beginning of each implementation chapter that has dependencies on other chapters, AND in the Master TODO roadmap.

**Quality criteria**: Every cross-chapter type dependency or invariant dependency is covered by a meta-instruction. The dependency reason is specific (not "because it's simpler").

**Negative specification (do NOT):**
- Do NOT omit meta-instructions when dependencies exist — LLMs implement in document order and will invent types if the defining chapter hasn't been processed yet.
- Do NOT create circular meta-instructions — if A depends on B and B depends on A, restructure the chapters.

**Cross-references**: INV-020, §0.2.3 Principle L4.

## Chapter 6: PART IV Elements

### §6.1 Operational Playbook

**§6.1.1 Phase -1: Decision Spikes** — Tiny experiments that de-risk unknowns. Each produces an ADR. Max time budget per spike.

**§6.1.2 Exit Criteria per Phase** — Specific, testable conditions. Not "scheduler works" but "property test demonstrates fair scheduling across 50 agents with no starvation for > 1000 ticks."

**§6.1.3 Merge Discipline** — What every PR touching invariants or critical paths must include.

**§6.1.4 Minimal Deliverables Order** — Build order maximizing "working subset" at each stage. Must be consistent with meta-instructions (INV-020).

**§6.1.5 Immediate Next Steps** — First 5–6 things to implement in dependency order.

### §6.2 Testing Strategy

| Test Type | What It Validates | Example |
|---|---|---|
| Unit | Individual function correctness | Reservation conflict detection |
| Property | Invariant preservation under random inputs | Replay determinism |
| Integration | Subsystem composition | Task completion triggers scheduling |
| Stress | Behavior at design point limits | 300 agents, 10K tasks, 60s |
| Replay | Determinism | Process N events, snapshot, replay, byte-compare |
| Adversarial | Robustness against malicious input | Forged task_id |

### §6.3 Error Taxonomy

Each error class has: severity (fatal/degraded/recoverable/ignorable), handling strategy (crash/retry/degrade/log), cross-references to threatened invariants.

**Negative specification (do NOT):**
- Do NOT catch all errors with a generic handler — each error class requires specific handling per its severity.
- Do NOT invent error classes not derived from invariant violations — every error threatens a specific invariant.

## Chapter 7: Appendix Elements

### §7.1 Glossary
Alphabetized domain-specific terms, 1–3 sentences each, with cross-reference to formal definition. Distinguish common vs. domain-specific meanings.

**Negative specification (do NOT):**
- Do NOT define common programming terms (unless the spec gives them domain-specific meaning).
- Do NOT omit cross-references to where the term is formally defined — a glossary entry without a section reference violates INV-006.

### §7.2 Risk Register
Top 5–10 risks with: description, impact, mitigation, detection method.

### §7.3 Master TODO Inventory
Checkboxable tasks organized by subsystem (not phase), each small enough for a single PR, cross-referenced to ADRs/invariants. Must include meta-instructions for ordering (per INV-020).

---

# PART III: Guidance (Recommended)

## Chapter 8: Voice and Style

### §8.1 The DDIS Voice

**Technically precise but human.** The voice of a senior engineer explaining their system to a peer they respect.

**Properties**:
- Uses concrete examples, not abstract descriptions
- Admits uncertainty where it exists
- Is direct about tradeoffs
- Does not hedge every statement
- Uses humor sparingly and only when it clarifies
- Never uses marketing language ("enterprise-grade", "cutting-edge")
- Never uses bureaucratic language ("it is recommended that", "the system shall")

**LLM-specific voice guidance:**
- Prefer tables and structured data over dense prose — LLMs parse structured data more reliably.
- Use explicit section numbers in all cross-references — never "see above" or "as mentioned earlier."
- When introducing a type or concept, bold it on first use and define it immediately — do not rely on the reader (LLM) inferring from context.

**Calibration examples**:

```
✅ GOOD: "The kernel loop is single-threaded by design — not because concurrency is
hard, but because serialization through the event log is the mechanism that gives
us deterministic replay for free. (Locked by ADR-003.)"

❌ BAD (academic): "The kernel loop utilizes a single-threaded architecture paradigm
to facilitate deterministic replay capabilities within the event-sourced persistence
layer."

❌ BAD (casual): "We made the kernel single-threaded and it's awesome!"

❌ BAD (bureaucratic): "It is recommended that the kernel loop shall be implemented
in a single-threaded manner to support the deterministic replay requirement as
specified in section 4.3.2.1."

❌ BAD (LLM-hostile): "As discussed earlier, the loop is single-threaded."
← "As discussed earlier" is meaningless to an LLM losing context. Say "per ADR-003."
```

### §8.2 Formatting Conventions

- **Bold** for terms being defined, non-negotiable properties, critical warnings
- `Code` for types, function names, file names
- `// Comments` for inline justifications and WHY NOT annotations
- `// REMINDER: INV-NNN` for structural redundancy at point of use (per INV-018)
- Tables for structured data (preferred over prose for LLM consumption)
- Blockquotes for preamble elements only
- ASCII diagrams preferred over external images

### §8.3 Anti-Pattern Catalog

**The Hedge Cascade**:
```
❌ "It might be worth considering the possibility of potentially using..."
✅ "The kernel loop is single-threaded. This gives us deterministic replay. See ADR-003."
```

**The Orphan Section**: References nothing and is referenced by nothing. Either connect it or remove it. (Violates INV-006.)

**The Trivial Invariant**: "INV-042: The system uses UTF-8 encoding." Either enforced by platform (not worth an invariant) or belongs in Non-Negotiables.

**The Strawman ADR**: Every option must have a genuine advocate. If Option B is obviously terrible, it is not a genuine alternative — find the real alternative or demote the decision from ADR to implementation note.

**The Percentage-Free Performance Budget**: "The system should respond quickly." Without a number, design point, and measurement method, this is a wish. (Violates INV-005.)

**The Spec That Requires Oral Tradition**: If an implementer must ask questions the spec should answer, patch the gap back. (Violates INV-008.)

**The Afterthought LLM Section**: A single "Chapter 14: LLM Considerations" appendix bolted onto an otherwise LLM-unaware spec. LLM optimization must be woven throughout — into element specifications, voice guidance, quality gates, and invariants. (Violates ADR-009.)

**The Implicit Cross-Reference**: "See above" or "as mentioned earlier." Always use explicit section numbers: "see §3.4" or "per INV-003." LLMs cannot resolve implicit references.

## Chapter 9: Proportional Weight Deep Dive

### §9.1 Identifying the Heart

Every system has 2–3 subsystems where most complexity and bugs live. These should receive 40–50% of PART II line budget.

**How to identify**: Which subsystems have the most invariants? Most ADRs? Most cross-references? If you cut the spec in half, which would you keep?

### §9.2 Signals of Imbalanced Weight

- 5 invariants + 50 lines of spec = **starved** — critical subsystem underspecified
- 1 invariant + 500 lines of spec = **bloated** — simple subsystem overspecified
- PART 0 longer than PART II = **top-heavy** — too much framework, not enough substance
- Appendices longer than PART II = **reference displacing substance**
- Negative specs absent from PART II = **hallucination-prone** — LLMs will fill gaps (per L1)

## Chapter 10: Cross-Reference Patterns

### §10.1 Reference Syntax

Recommended conventions (be consistent within a spec):
```
(see §3.2)                    — section reference
(validated by INV-004)        — invariant reference
(locked by ADR-003)           — decision reference
(measured by Benchmark B-001) — performance reference
(defined in Glossary: "task") — glossary reference
// REMINDER: INV-NNN — ...    — structural redundancy marker
```

### §10.2 Reference Density Targets

| Section Type | Minimum Outbound References |
|---|---|
| Implementation chapter | 4 (ADR + invariant + other chapter + negative spec) |
| ADR | 2 (invariant + implementation chapter) |
| Invariant | 1 (test or validation method) |
| Performance budget | 2 (benchmark + design point) |
| Test strategy | 2 (invariant + implementation chapter) |
| Negative specification | 1 (invariant it protects) |
| Verification prompt | 2 (invariant + section to verify against) |

---

# PART IV: Operations — Applying and Evolving DDIS

## Chapter 11: Applying DDIS to a New Project

### §11.1 The Authoring Sequence

Write in this order (not document order) to minimize rework:

1. Design goal + Core promise
2. First-principles formal model
3. LLM consumption model (§0.2.3) — identify domain-specific LLM failure modes early
4. Non-negotiables
5. Invariants (including negative specification constraints per INV-017)
6. ADRs
7. Implementation chapters — heaviest subsystems first, with negative specs and verification prompts
8. End-to-end trace
9. Performance budgets
10. Test strategies
11. Cross-references (thread the web)
12. Glossary (extract from complete spec)
13. Master TODO with meta-instructions for ordering
14. Operational playbook

### §11.2 Common Mistakes in First DDIS Specs

1. **Writing implementation chapters before ADRs.** You'll rewrite when ADRs imply different choices.
2. **Writing the glossary first.** You don't know your terminology until the spec is written.
3. **Treating the end-to-end trace as optional.** It's the most effective quality check. (See §1.4.)
4. **Under-investing in WHY NOT annotations.** Every non-obvious choice needs one.
5. **Skipping the negative specifications.** LLMs especially benefit from explicit "do NOT" constraints. This is the #1 mistake for LLM-targeted specs.
6. **Omitting verification prompts.** Without them, LLMs cannot self-check and errors accumulate.
7. **Writing "see above" instead of explicit section references.** LLMs cannot resolve implicit references.
8. **Treating the LLM consumption model as optional.** It's the foundational justification for half the structural decisions.

## Chapter 12: Validating a DDIS Specification

### §12.1 Self-Validation Checklist

1. Trace 5 random implementation sections backward to formal model. Any breaks? (Gate 2)
2. For each ADR, would a competent engineer genuinely choose each rejected option? (Gate 3)
3. For each invariant, spend 60 seconds constructing a violation scenario. (Gate 4)
4. Build the cross-reference graph. Orphan sections? (Gate 5)
5. Read as a first-time implementer. Where did you have to guess? (Gate 6)
6. Give one implementation chapter (plus formal model and invariants) to an LLM. Did it hallucinate? (Gate 7)
7. Check: does every implementation chapter have ≥1 negative spec? (INV-017)
8. Check: does every implementation chapter end with a verification prompt? (INV-019)
9. Check: are all cross-chapter dependencies covered by meta-instructions? (INV-020)

### §12.2 External Validation

Give the spec to an implementer/LLM and track:
- Questions the spec should have answered (gaps — violates INV-008)
- Incorrect implementations the spec didn't prevent (ambiguities — violates INV-017)
- Sections skipped due to confusion (voice/clarity issues — violates INV-007)
- Hallucinated types or decisions not in the spec (LLM failure — violates Gate 7)

## Chapter 13: Evolving a DDIS Specification

### §13.1 The Living Spec

Once implementation begins:
- **Gaps** are patched into the spec, not into oral tradition (INV-008)
- **Superseded ADRs** are marked "Superseded by ADR-NNN" (not deleted — historical record)
- **New invariants** may be added with full INV-NNN format
- **Performance budgets** may be revised with documented rationale
- **Negative specifications** should be added when implementation reveals new failure modes (per INV-017)
- **Verification prompts** should be updated when new invariants are added to a chapter (per INV-019)

### §13.2 Spec Versioning

`Major.Minor` where:
- **Major**: formal model, non-negotiable, or LLM consumption model changes
- **Minor**: ADRs, invariants, implementation chapters, negative specs, or verification prompts added/revised

---

## §0.13 Modularization Protocol [Conditional]

REQUIRED when the monolithic specification exceeds 4,000 lines or when the target context window cannot hold the full spec plus reasoning budget. OPTIONAL but recommended for specs between 2,500–4,000 lines.

> Namespace note: INV-001 through INV-020 and ADR-001 through ADR-011 are DDIS meta-standard identifiers. Application specs define their OWN namespace (e.g., APP-INV-001).

### §0.13.1 The Scaling Problem

When a spec exceeds the implementer's context window, two failure modes emerge:

1. **Truncation**: The LLM silently drops content from the beginning, losing invariants and the formal model.
2. **Naive splitting**: Arbitrary splits break cross-references, orphan invariants, and force guessing.

The modularization protocol prevents both by defining principled decomposition with formal completeness guarantees. (Motivated by INV-008, informed by §0.2.3 Principle L2.)

### §0.13.2 Core Concepts

- **Monolith**: A single-document DDIS spec. All specs start here.
- **Module**: A self-contained unit covering one major subsystem. Never read alone — always assembled into a bundle.
- **Constitution**: Cross-cutting material constraining all modules. Organized in tiers.
- **Domain**: An architectural grouping of related modules with tighter internal coupling.
- **Bundle**: The assembled document for LLM consumption: system constitution + domain constitution + cross-domain deep context + module.
- **Manifest**: Machine-readable YAML declaring all modules, domains, invariant ownership, and assembly rules.

### §0.13.3 The Tiered Constitution

Three tiers prevent the constitution from becoming a bottleneck. NO overlapping content between tiers. (Locked by ADR-006.)

```
Tier 1: System Constitution (200-400 lines, always)
  Declarations only: ID + 1-line for all invariants and ADRs
  Plus: design goal, non-negotiables, architecture overview, glossary, quality gates

Tier 2: Domain Constitution (200-500 lines, per-domain)
  Full definitions for domain-owned invariants and ADRs
  Domain formal model, interface contracts, performance budgets

Tier 3: Cross-Domain Deep Context (0-600 lines, per-module)
  Full definitions for OTHER-domain invariants this module interfaces with
  Zero overlap with Tier 2. Empty if module has no cross-domain interfaces.

Module (800-3,000 lines)
  Module header + full PART II content for one subsystem
  Includes: negative specifications, verification prompts, meta-instructions

Bundle = Tier 1 + Tier 2 + Tier 3 + Module
Target: 1,200-4,500 lines | Ceiling: 5,000 lines
```

#### Two-Tier Simplification
When total invariant + ADR count ≤ 20 and the system constitution fits in ≤ 400 lines: Tier 1 contains BOTH declarations AND full definitions. No Tier 2 or Tier 3. Assembly: `system_constitution + module → bundle`. Manifest uses `tier_mode: two-tier`.

### §0.13.4 Invariant Declarations vs. Definitions

**Declaration** (Tier 1, ~1 line):
```
APP-INV-017: Event log is append-only -- Owner: EventStore -- Domain: Storage
```

**Definition** (Tier 2, ~10-20 lines): Full statement, formal expression, violation scenario, validation method, WHY THIS MATTERS.

**Inclusion rules:**

| Relationship | Tier 1 | Tier 2 | Tier 3 |
|---|---|---|---|
| Module MAINTAINS invariant | Declaration | Full definition | — |
| INTERFACES, same domain | Declaration | Full definition | — |
| INTERFACES, other domain | Declaration | — | Full definition |
| No relationship | Declaration | — | — |

The same pattern applies to ADRs.

### §0.13.5 Module Header (Required per Module)

```yaml
# Module Header: [Module Name]
# Domain: [Domain Name]
# Maintains: APP-INV-017, APP-INV-018, APP-INV-019
# Interfaces: APP-INV-003 (via EventStore), APP-INV-032 (via Scheduler)
# Implements: APP-ADR-003, APP-ADR-011
# Adjacent modules: EventStore, Scheduler
# Assembly: Tier 1 + Domain + cross-domain deep
#
# NEGATIVE SPECIFICATION:
# - Must NOT directly access TUI rendering state
# - Must NOT bypass the reservation system
# - Must NOT reference other modules' internal sections (INV-012)
```

### §0.13.6 Cross-Module Reference Rules

**Rule 1**: Cross-module references go through the constitution, never direct. (INV-012, ADR-007.)
```
BAD:  "See section 7.3 in the Scheduler chapter"
GOOD: "See APP-INV-032, maintained by the Scheduler module"
```

**Rule 2**: Shared types are defined in the constitution, not in any module.

**Rule 3**: The end-to-end trace is a special cross-cutting module with `interfaces: all`.

### §0.13.7 Modularization Decision Flowchart

```
Spec > 4,000 lines? → Yes → MODULE (required)
                    → No  → Spec > 2,500 AND context < 8K? → Yes → MODULE (recommended)
                                                            → No  → MONOLITH

If MODULE:
  < 20 invariants+ADRs AND system constitution ≤ 400 lines → TWO-TIER
  Otherwise → THREE-TIER
```

### §0.13.8 File Layout

```
spec-project/
├── manifest.yaml
├── constitution/
│   ├── system.md                 # Tier 1
│   └── domains/                  # Tier 2 (absent in two-tier)
├── deep/                         # Tier 3 (only if cross-domain)
├── modules/                      # One per subsystem
└── bundles/                      # Generated (gitignored)
```

### §0.13.9 Manifest Schema

Key fields per module: `file`, `domain`, `maintains`, `interfaces`, `implements`, `adjacent`, `deep_context`, `negative_specs`, `budget_lines`. The `invariant_registry` maps every invariant to its owner, domain, and description.

### §0.13.10 Assembly Rules

**Three-tier**: Tier 1 + domain Tier 2 + Tier 3 (if exists) + module.
**Two-tier**: Tier 1 + module.
Both validate budget compliance (INV-014): ERROR if > ceiling, WARN if > target.

### §0.13.11 Consistency Validation

Nine mechanical checks:

- **CHECK-1**: Invariant ownership — each invariant maintained by exactly one module (INV-013)
- **CHECK-2**: Interface consistency — interfaced invariants are maintained elsewhere or system-owned
- **CHECK-3**: Adjacency symmetry — if A lists B as adjacent, B lists A
- **CHECK-4**: Domain membership — module's maintained invariants are in module's domain
- **CHECK-5**: Budget compliance — assembled bundles within ceiling (INV-014)
- **CHECK-6**: No orphan invariants — every invariant maintained or interfaced by some module
- **CHECK-7**: Cross-module isolation — no direct module-to-module references (INV-012)
- **CHECK-8**: Deep context correctness — cross-domain interfaces have deep context files (three-tier only)
- **CHECK-9**: File existence — all manifest paths exist; all module files are in manifest (INV-016)

### §0.13.12 Cascade Protocol

When constitutional content changes, affected modules must be re-validated.

| Change | Blast Radius |
|---|---|
| Invariant wording changed | Modules maintaining or interfacing |
| ADR superseded | Modules implementing that ADR |
| New invariant added | Module assigned as owner |
| Shared type changed | Same-domain + cross-domain users |
| Non-negotiable changed | ALL modules |
| Glossary term redefined | All modules using that term |
| LLM consumption model changed | ALL modules (per §0.2.3) |

### §0.13.13 Modularization Quality Gates

**Gate M-1**: All nine checks pass with zero errors.
**Gate M-2**: All bundles under ceiling; < 20% exceed target.
**Gate M-3**: LLM bundle sufficiency — zero questions requiring other module's content. (INV-011.)
**Gate M-4**: Declaration-definition faithfulness. (INV-015.)
**Gate M-5**: Cascade simulation — simulated invariant change correctly identifies affected modules. (INV-016.)

### §0.13.14 Migration: Monolith to Modular

1. Identify domains (2–5 based on architecture)
2. Extract system constitution (Tier 1)
3. Extract domain constitutions (Tier 2)
4. Extract modules with headers; convert direct cross-refs to constitutional refs
5. Create cross-domain deep context files (Tier 3)
6. Build manifest
7. Validate (all 9 checks)
8. Extract end-to-end trace as cross-cutting module
9. LLM validation on 2+ bundles (Gate M-3)

---

# APPENDICES

## Appendix A: Glossary

| Term | Definition |
|------|------------|
| **ADR** | Architecture Decision Record — structured record of a design choice with alternatives and rationale (§3.5) |
| **Bundle** | Assembled document for LLM consumption: constitution + module (§0.13.2) |
| **Cascade protocol** | Procedure for re-validating modules after constitutional changes (§0.13.12) |
| **Causal chain** | Traceable path from first principle through invariant/ADR to implementation detail (§0.2.2) |
| **Churn-magnet** | Decision causing most downstream rework if left open (§3.5) |
| **Constitution** | Cross-cutting material constraining all modules, organized in tiers (§0.13.3) |
| **Context window** | The maximum text an LLM can process in a single session (§0.2.3) |
| **Cross-reference** | Explicit link between spec sections forming the reference web (Ch. 10) |
| **DDIS** | Decision-Driven Implementation Specification — this standard |
| **Declaration** | Compact 1-line invariant/ADR summary in system constitution (§0.13.4) |
| **Definition** | Full invariant/ADR specification with formal expression and validation (§0.13.4) |
| **Design point** | Specific hardware/workload/scale scenario for performance validation (§3.7) |
| **Domain** | Architectural grouping of related modules (§0.13.2) |
| **End-to-end trace** | Worked scenario traversing all subsystems to validate INV-001 (§5.3, §1.4) |
| **Falsifiable** | Can be violated by concrete scenario and detected by concrete test (INV-003) |
| **First principles** | Formal model from which architecture derives (§3.3) |
| **Gate** | Stop-ship predicate that must be true before proceeding (§3.6) |
| **Hallucination gap** | Information absent from spec that an LLM fills with plausible but incorrect content (§0.2.3 L1) |
| **Invariant** | Numbered falsifiable property that must always hold (§3.4) |
| **Living spec** | Specification being updated as implementation reveals gaps (§13.1) |
| **LLM consumption model** | Model of LLM cognitive characteristics that derives structural requirements (§0.2.3) |
| **Manifest** | YAML file declaring modules, ownership, interfaces, assembly rules (§0.13.9) |
| **Meta-instruction** | Explicit directive about implementation ordering between chapters (§5.7, INV-020) |
| **Module** | Self-contained spec unit covering one subsystem, assembled into a bundle (§0.13.2) |
| **Monolith** | DDIS spec existing as a single document (§0.13.2) |
| **Negative specification** | Explicit constraint on what the system must NOT do (§3.8, INV-017) |
| **Non-negotiable** | Philosophical commitment defining what the system IS (§3.1) |
| **Proportional weight** | Line budget guidance preventing section bloat/starvation (§0.8, Ch. 9) |
| **Self-bootstrapping** | Property of this standard: written in the format it defines (ADR-004) |
| **Structural redundancy** | Restating key constraints at point of use, not just at definition (INV-018, ADR-010) |
| **Verification prompt** | Self-check checklist at end of implementation chapter (§5.6, INV-019) |
| **WHY NOT annotation** | Inline comment explaining why an alternative was rejected (§5.4) |
| **Worked example** | Concrete scenario with specific values showing a subsystem in action (§5.2) |

## Appendix B: Risk Register

| Risk | Impact | Mitigation | Detection |
|------|--------|-----------|-----------|
| Spec exceeds context window | LLM drops critical content | Modularization protocol (§0.13); proportional weight (Ch. 9) | Line count monitoring; Gate M-2 |
| LLM hallucinates design decisions | Silent correctness bugs | Negative specifications (INV-017); ADRs for every non-obvious choice (INV-002) | Gate 7 (LLM test); verification prompts (INV-019) |
| Invariant without violation scenario | False confidence in correctness | INV-003 requires violation scenario; Gate 4 tests constructibility | Self-validation checklist item 3 (§12.1) |
| Orphan sections | Content invisible to LLM navigating by references | INV-006 requires connected graph; Gate 5 tests connectivity | Cross-reference graph analysis |
| Strawman ADRs | Decisions not genuinely evaluated | ADR quality criteria (§3.5); anti-pattern catalog (§8.3) | Gate 3 adversarial review |
| Spec becomes oral tradition | Implementation diverges from spec | INV-008 (self-containment); living spec process (§13.1) | Gate 6 (implementation readiness) |
| Structural redundancy drifts from source | Contradictory information | ADR-010: reminders are non-authoritative; definitions win | CHECK: grep for `// REMINDER:` and compare against definitions |

## Appendix C: Error Taxonomy for Specification Authoring

[SELF-BOOTSTRAP: This section demonstrates the §6.3 element specification for error taxonomies, applied to the domain of specification authoring itself.]

| Error Class | Severity | Handling | Threatened Invariant | Example |
|---|---|---|---|---|
| **Unfalsifiable invariant** | Fatal | Rewrite to include violation scenario | INV-003 | "The system shall be secure" |
| **Strawman ADR** | Fatal | Replace rejected option with genuine alternative | INV-002 | Option B nobody would choose |
| **Orphan section** | Degraded | Add cross-references or remove section | INV-006 | Section referenced by nothing |
| **Missing negative spec** | Degraded | Add ≥1 "do NOT" constraint | INV-017 | Implementation chapter with no negative specs |
| **Implicit cross-reference** | Recoverable | Replace with explicit §X.Y or INV-NNN | INV-006, L2 | "See above" |
| **Missing verification prompt** | Degraded | Add checklist with ≥3 items | INV-019 | Implementation chapter with no self-check |
| **Missing meta-instruction** | Degraded | Add ordering directive | INV-020 | Chapter depends on another but doesn't say so |
| **Hallucination-prone gap** | Fatal | Add negative spec or explicit decision | INV-017, L1 | Undefined type that LLM will invent |
| **Context-window budget exceeded** | Fatal | Modularize per §0.13 | INV-014 | Spec > 5,000 lines without modularization |
| **Contradictory prescriptions** | Fatal | Reconcile via ADR | Safety property (§1.2) | Section A says "use X", section B says "use Y" |
| **Undefined domain term** | Recoverable | Add to glossary | INV-009 | Term used but not in glossary |
| **Incomplete state machine** | Degraded | Fill all state × event cells | INV-010 | Missing invalid transition policy |

---

# PART X: MASTER TODO INVENTORY

## Self-Conformance Checklist (DDIS 2.0 Applied to Itself)

| Element | Status | Notes |
|---|---|---|
| Design goal (§2.1) | ✅ Complete | Preamble |
| Core promise (§2.2) | ✅ Complete | Preamble |
| Document note (§2.3) | ✅ Complete | Preamble, includes [SELF-BOOTSTRAP] guidance |
| How to use (§2.4) | ✅ Complete | Preamble, 6 items |
| Executive summary (§0.1) | ✅ Complete | |
| First-principles derivation (§0.2) | ✅ Complete | Includes causal chain table |
| LLM consumption model (§0.2.3) | ✅ Complete | Principles L1–L4 with structural consequences |
| Architecture overview (§0.4) | ✅ Complete | Three-ring model |
| Invariants (§0.5) | ✅ Complete | INV-001–020 with violations and validation |
| ADRs (§0.6) | ✅ Complete | ADR-001–011 with genuine alternatives |
| Quality gates (§0.7) | ✅ Complete | Gates 1–7 + M-1–M-5 with measurement |
| Performance budgets (§0.8) | ✅ Complete | Includes measurement harness |
| Non-negotiables (§0.1.2) | ✅ Complete | 7 items with validation references |
| Non-goals (§0.1.3) | ✅ Complete | 6 items |
| State machine (§1.1) | ✅ Complete | Full state × event table, invalid transitions |
| End-to-end trace (§1.4) | ✅ Complete | ADR lifecycle through all steps |
| Element specifications (Ch. 2–7) | ✅ Complete | Including §3.8, §5.6, §5.7 |
| Negative specifications throughout | ✅ Complete | Every element spec includes "do NOT" |
| Verification prompts specified | ✅ Complete | §5.6 element spec |
| Meta-instructions specified | ✅ Complete | §5.7 element spec |
| Voice guide (Ch. 8) | ✅ Complete | Including LLM-specific guidance |
| Cross-reference patterns (Ch. 10) | ✅ Complete | Including new element types |
| Anti-pattern catalog (§8.3) | ✅ Complete | Including Afterthought LLM Section |
| Glossary (App. A) | ✅ Complete | 32 terms including new LLM terms |
| Risk register (App. B) | ✅ Complete | 7 risks with LLM-specific entries |
| Error taxonomy (App. C) | ✅ Complete | 12 error classes for spec authoring |
| Modularization protocol (§0.13) | ✅ Complete | Full protocol with cascade update for §0.2.3 |

## Quick-Reference Card

```
PREAMBLE: Design goal → Core promise → Document note → How to use
PART 0:   Summary → First principles → LLM consumption model → Architecture →
          Layout → Invariants → ADRs → Gates → Budgets → API →
          Non-negotiables → Non-goals → [Modularization protocol]
PART I:   Formal model → State machines → Complexity → End-to-end trace
PART II:  [Per subsystem: types → algorithm → state machine → invariants →
          example → negative specs → WHY NOT → tests → budget →
          cross-refs → verification prompt]
PART III: Protocol schemas → Adapters → UI contracts
PART IV:  Test taxonomy → Error taxonomy → Operational playbook
APPENDICES: Glossary → Risks → Error Taxonomy → Formats → Benchmarks
PART X:   Master TODO (checkboxable, by subsystem)

Every invariant: ID + statement + formal + violation + test + WHY THIS MATTERS
Every ADR: problem + options (genuine) + decision + WHY NOT + consequences
Every algorithm: pseudocode + complexity + example + edge cases
Every impl chapter: + negative specs + verification prompt + meta-instructions
Cross-refs: web, not list. No orphan sections. No "see above."
Voice: senior engineer to respected peer. No hedging. No marketing. No bureaucracy.
LLM: Provisions woven throughout. Not an appendix. Not optional.
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2024 | Initial standard: INV-001–010, ADR-001–005, Gates 1–6, modularization protocol (INV-011–016, ADR-006–007, Gates M-1–M-5) |
| 2.0 | 2025 | LLM consumption model (§0.2.3); INV-017–020 (negative specs, structural redundancy, verification prompts, meta-instructions); ADR-008–011; Gate 7 (LLM implementation readiness); element specs §3.8, §5.6, §5.7; end-to-end trace §1.4; error taxonomy Appendix C; full state machine with state × event table; measurement harness; LLM-specific voice guidance; anti-pattern additions |

<!-- END: DDIS 2.0 -->
