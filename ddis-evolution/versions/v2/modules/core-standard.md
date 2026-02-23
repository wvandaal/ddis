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

Validation: Give the spec to a competent engineer unfamiliar with the project. Track every question they ask. If questions reveal missing spec information, INV-008 is violated.

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

- Authors must sometimes determine where a domain-specific concept fits
- Readers gain predictability; validation tools can check conformance mechanically

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
B) **Required** — every non-trivial section must have inbound and outbound references, using explicit identifiers (§X.Y, INV-NNN, ADR-NNN).

#### Decision

**Option B: Required.** Cross-references transform a collection of sections into a unified specification. Without them, the causal chain (INV-001) cannot be verified. For LLMs, explicit identifiers (§X.Y, INV-NNN, ADR-NNN) are the ONLY reliable navigation — implicit references like "see above" fail (§0.2.2).

#### Consequences

- Higher authoring cost (every section requires thinking about its relationships)
- Much higher reader value; enables graph-based validation of completeness

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

- More trustworthy (tested by self-application) but more complex (meta-level and object-level interleave)
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

**Option B: Voice guidance.** Specifications fail when either too dry to read or too casual to trust. DDIS prescribes a specific voice: technically precise but human, a senior engineer explaining to a peer they respect. (See §8.1, guidance-operations module.) For LLMs, explicit voice guidance reduces generic boilerplate.

#### Consequences

- Specs feel more unified and readable
- Authors must sometimes revise natural writing habits

#### Tests

- Qualitative review: sample 5 sections, assess whether each sounds like a senior engineer talking to a peer. If any sounds like a textbook, marketing copy, or bureaucratic report, the voice is wrong.

---

### ADR-008: LLM Provisions Woven Throughout, Not Isolated

#### Problem

DDIS introduces structural provisions for LLM consumption (negative specifications, verification prompts, meta-instructions, structural redundancy). How should these provisions be integrated into the standard?

#### Options

A) **Isolated chapter** — a "Chapter N: LLM Considerations" appendix.
- Pros: Easy to find; easy to skip for human implementers.
- Cons: LLM provisions distant from the element they modify get forgotten — exactly the "implicit reference failure" of §0.2.2.

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
- Cons: Anti-patterns are document-level guidance, not subsystem-level requirements. LLMs need co-located "DO NOT" constraints — a list 500 lines away has minimal effect.

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
- Cons: Catches bugs after code is written. For LLMs, rewriting is expensive (new API call, new context). A self-check DURING implementation is cheaper than a test AFTER.

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
- Cons: Loses reasoning history. Future maintainers cannot understand why the original decision was made or reversed.

B) **Mark-and-supersede with cross-reference cascade** — Mark old ADR as superseded, retain as record, create new ADR with fresh identifier, cascade-update all references.
- Pros: Preserves history; "WHY NOT the old approach?" prevents re-exploration; cascade ensures consistency.
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
> - **§3.4 (Invariants)**: LLM writes invariants → must use prescribed five-part format; must NOT produce aspirational invariants or omit violation scenarios.
> - **§4.2 (State Machines)**: LLM writes a state machine → state × event table with NO empty cells, guards, invalid transition policy; must NOT produce happy-path-only transitions.
> - **§5.1 (Implementation Chapters)**: LLM writes an implementation chapter → all 13 required components present; must NOT reference invariants by ID alone (INV-018) or omit negative specs (INV-017).
> - **§6.1 (Operational Playbook)**: LLM writes a playbook → decision spikes with time budgets and ADR exit criteria; deliverable order as explicit DAG, not flat list; no aspirational exit criteria.

**Gate 7 test protocol:**

1. **Select** 2 representative implementation chapters (one high-complexity, one moderate).
2. **Assemble** test input per chapter: chapter text + glossary + referenced invariants. No other sections.
3. **Prompt** the LLM: "Implement this subsystem based on the following specification. Do not add features not described in the spec."
4. **Score** the output on four axes: (a) hallucinated requirements, (b) negative spec violations, (c) missing invariant preservation, (d) architectural clarifying questions.
5. **Pass criteria**: (a) = 0, (b) = 0, (c) all chapter-header invariants addressed, (d) ≤ 1 per chapter. (See SPEC-BENCH-002 in §0.8.4.)
6. **Failure remediation**: missing negative spec → add per INV-017 (format in §3.8, element-specifications module); missing restatement → fix per INV-018; ambiguity → clarify in chapter.

### Definition of Done (for this standard)

DDIS 3.0 is "done" when:
- This document passes Gates 1–7 applied to itself
- At least one non-trivial spec has been written conforming to DDIS without structural workarounds
- The Glossary (Appendix A) covers all DDIS-specific terminology
- LLM provisions are demonstrated in this document's own element specifications (self-bootstrapping)

## 0.8 Performance Budgets (for Specifications, Not Software)

Specifications have performance characteristics too. A 40-hour spec is too long. A 2-hour spec probably omits critical details.

### 0.8.1 Specification Size Budgets

| System Complexity | Target Spec Length | Rationale |
|---|---|---|
| Small (single crate, < 5K LOC target) | 500–1,500 lines | Enough for formal model + invariants + key ADRs |
| Medium (multi-crate, 5K–50K LOC target) | 1,500–5,000 lines | Full DDIS treatment |
| Large (multi-service, > 50K LOC target) | 5,000–15,000 lines | May split into sub-specs linked by a master |

### 0.8.2 Proportional Weight Guide

Not all sections are equal. These proportions prevent bloat and starvation. Domain-specific specs may adjust by ±20%.

**For domain specifications:**

| Section | % of Total | Why |
|---|---|---|
| Preamble + PART 0 | 15–20% | Dense: formal model, invariants, ADRs, quality gates |
| PART I: Foundations | 8–12% | First principles, state machines, complexity analysis |
| PART II: Core Implementation | 35–45% | THE HEART: algorithms, data structures, protocols, examples, negative specs, verification prompts |
| PART III: Interfaces | 8–12% | API schemas, adapters, external contracts |
| PART IV: Operations | 10–15% | Testing, operational playbook, roadmap |
| Appendices + Part X | 10–15% | Reference material, glossary, error taxonomy, master TODO |

**For meta-standards (self-bootstrapping specs about specification authoring):**

A meta-standard's "implementation" IS its own definitions — invariants, ADRs, quality gates, and element specifications all reside in PART 0 and PART II. The domain-spec proportions do not apply directly because there are no external algorithms or protocols to describe. Meta-standards use these adjusted proportions:

| Section | % of Total | Why |
|---|---|---|
| Preamble + PART 0 | 45–60% | Contains the entire standard definition: invariants, ADRs, gates, modularization protocol |
| PART I: Foundations | 3–6% | Formal model of specifications as artifacts — concise by nature |
| PART II: Element Specifications | 20–30% | Templates and guidance for each structural element |
| PART III: Guidance | 4–8% | Voice, style, and cross-reference patterns |
| PART IV: Operations | 3–6% | Authoring sequence, validation, evolution |
| Appendices + Part X | 6–12% | Reference material, glossary, error taxonomy, master TODO |

DO NOT apply domain-spec proportions to a meta-standard and flag a violation. The causal chain is: meta-standards define the structure that domain specs follow, so their weight distribution reflects authoring (definitions, invariants, ADRs) rather than implementation (algorithms, protocols). See Chapter 9 (guidance-operations module) for diagnostic signals of imbalanced weight and guidance on identifying the spec's "heart."

**Self-application — Proportional weight verification** (ADR-004): The modular form of this standard (3,811 lines total) distributes PART 0 across the constitution and two modules. Aggregate proportions against meta-standard targets:

| Category | Modular components | Lines | % of Total | Target | Status |
|---|---|---|---|---|---|
| Preamble + PART 0 | constitution (524) + core-standard (894) + modularization (855) | 2,273 | 59.6% | 45–60% | Within ±20% |
| PART I: Foundations | Included in core-standard above | — | — | 3–6% | Embedded |
| PART II: Element Specifications | element-specifications | 918 | 24.1% | 20–30% | Within |
| PART III + IV + Appendices | guidance-operations | 620 | 16.3% | 13–26% combined | Within |

The modular decomposition shifts per-file percentages (each module is 16–24% of total), but aggregate proportions remain within the ±20% adjustment band. The "heart" of this meta-standard — invariant definitions, ADRs, element specification templates — correctly dominates at ~84% (PART 0 + PART II).

### 0.8.3 Authoring Time Budgets

These are rough guides for experienced authors:

| Element | Expected Authoring Time | Notes |
|---|---|---|
| First-principles model | 2–4 hours | Hardest part; requires deep domain understanding |
| One invariant (high quality) | 15–30 minutes | Including violation scenario and test strategy |
| One ADR (high quality) | 30–60 minutes | Including genuine alternative analysis |
| One implementation chapter | 2–4 hours | Including algorithm, examples, test strategy |
| Negative specs per chapter | 15–30 minutes | Requires adversarial thinking: "what would an LLM get wrong?" |
| Verification prompt per chapter | 10–15 minutes | Derived from invariants and negative specs |
| End-to-end trace | 1–2 hours | Requires all subsystems to be drafted first |
| Glossary | 1–2 hours | Best done last, by extracting terms from the full spec |

### 0.8.4 Specification Quality Measurement

**Design point for these metrics**: A medium-complexity DDIS-conforming spec (1,500–5,000 lines per §0.8.1) consumed by a competent engineer (≥ 3 years experience in the domain) or a current-generation LLM (≥ 100K context window). Simpler specs (< 1,000 lines) will exceed these targets easily; specs at the upper extreme (> 5,000 lines) should apply these per-module after modularization (§0.13).

To validate the performance budgets above, measure these metrics during implementation:

| ID | Metric | Measurement Method | Target |
|---|---|---|---|
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
|---|---|---|---|---|---|---|
| **Skeleton** | → Drafted | REJECT: empty sections | REJECT: empty sections | REJECT: empty sections | REJECT: not validated | REJECT: nothing to gap |
| **Drafted** | No transition (already filled) | → Threaded | REJECT: not threaded | REJECT: not threaded | REJECT: not validated | → Drafted (re-draft) |
| **Threaded** | → Drafted (partial regression) | No transition (already threaded) | → Gated | REJECT: gates 1–5 first | REJECT: not validated | → Drafted (partial regression) |
| **Gated** | → Drafted (partial regression) | No transition | No transition (already gated) | → Validated | REJECT: not validated | → Drafted (partial regression) |
| **Validated** | → Drafted (partial regression) | No transition | No transition | No transition (already validated) | → Living | → Drafted (partial regression) |
| **Living** | No transition | No transition | No transition | No transition | No transition (already living) | → Drafted (partial regression) |

Every "REJECT" cell names the policy. Every "No transition" cell indicates the event is inapplicable in that state (idempotent). Every "partial regression" returns to Drafted for re-validation per the Living → Drafted transition guard.

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

**Self-application — Safety verification**: Do any two sections of this DDIS standard prescribe contradictory behavior? Test case: §3.8 (Negative Specifications) prescribes "3–8 negative specs per subsystem" while §5.6 (Verification Prompts) prescribes "at least one negative check per verification prompt." These are complementary, not contradictory — §3.8 defines the constraints, §5.6 requires verifying them. No contradiction found. A contradiction *would* exist if §3.8 said "DO NOT constraints are optional" while INV-017 required them — but §3.8 explicitly states they are required.

**Self-application — Liveness verification**: Can an author following this standard answer "How should I handle LLM hallucination of unauthorized features?" The answer chain: §0.2.2 (LLM Consumption Model) identifies the problem → §3.8 (Negative Specifications, element-specifications module) provides the mechanism → INV-017 requires ≥3 per chapter → §5.6 (Verification Prompts, element-specifications module) verifies compliance → Gate 7 tests LLM output. The spec reaches a Validated answer through 5 cross-referencing sections.

### 1.3 Complexity of Specification Elements

| Element | Authoring Complexity | Reading Complexity | Verification Complexity |
|---|---|---|---|
| Invariant | O(domain_understanding) | O(1) per invariant | O(1) per invariant (construct counterexample) |
| ADR | O(alternatives × analysis_depth) | O(alternatives) per ADR | O(1) per ADR (check that alternatives are genuine) |
| Algorithm | O(algorithm_complexity × edge_cases) | O(pseudocode_length) | O(worked_examples) |
| Cross-reference | O(1) per reference | O(1) per reference | O(sections²) for full graph analysis |
| End-to-end trace | O(subsystems × interactions) | O(subsystems) | O(1) (follow the trace) |
| Negative specification | O(adversarial_thinking) | O(1) per constraint | O(1) per constraint (check implementation) |
| Verification prompt | O(invariants_per_chapter) | O(1) per prompt | O(1) (execute the prompt) |

The quadratic cost of cross-reference verification is why automated tooling (§0.10, question 1) would be valuable.

**Meta-standard operation complexity** (anchored to a design point: 3,000-line domain spec, 15 invariants, 8 ADRs, 6 implementation chapters):

| Operation | Complexity | Design Point Estimate |
|---|---|---|
| Authoring a complete spec | O(sections × cross_refs) | ~120 section × ~4 refs = ~480 threading decisions |
| Validating Gates 1–5 | O(sections²) for cross-ref graph + O(invariants) for falsifiability | ~120² / 2 = ~7,200 edge checks + 15 counterexamples |
| Running Gate 7 (LLM readiness) | O(chapters × LLM_calls) | 6 chapters × 1 LLM call each = 6 eval sessions |
| Cross-reference cascade (§0.13.12) | O(affected_invariants × dependent_modules) | ~3 invariants × ~2 modules = ~6 cascade steps |
| Modularization decision (§0.13.7) | O(1) — single flowchart evaluation | 1 check against 2,500-line threshold |

The O(sections²) cost of Gate 5 validation dominates. For large specs (> 5,000 lines), automated cross-reference tooling transforms this from a multi-hour manual audit to minutes.

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
