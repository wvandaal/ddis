## 0.6 Architecture Decision Records

### ADR-001: Document Structure Is Fixed, Not Flexible

#### Problem

Should DDIS prescribe a fixed document structure, or allow authors to organize freely as long as content requirements are met?

#### Options

A) **Fixed structure** (prescribed section ordering and hierarchy)
- Pros: Predictable for readers; mechanical completeness checking; easier to teach; LLMs can navigate any DDIS spec with the same expectations.
- Cons: May feel rigid; some domains fit the structure better than others.

B) **Content requirements only** (prescribe what, not where)
- Pros: Flexibility; authors can organize by whatever axis makes sense.
- Cons: Every spec is a unique snowflake; readers must re-learn structure each time; LLMs must discover structure per-spec, increasing error rate.

C) **Fixed skeleton with flexible interior** (prescribed top-level parts, flexible chapter organization within)
- Pros: Balance of predictability and flexibility.
- Cons: The "flexible interior" often means "no structure at all."

#### Decision

**Option A: Fixed structure.** The value of DDIS is that a reader who has seen one DDIS spec can navigate any other DDIS spec. This is worth the cost of occasionally awkward section placement. For LLM implementers, structural predictability (INV-018) is essential — it reduces the variance in implementation quality.

The structure may be renamed (e.g., "Kernel Invariants" instead of "Invariants") and domain-specific sections may be added within any PART, but the required elements (§0.3) must appear, and the PART ordering must be preserved.

Confidence: High — validated by multiple DDIS-conforming specs in practice.

// WHY NOT Option B? Every spec becomes a navigation puzzle. LLMs waste context on structure discovery instead of implementation.

#### Consequences

- Authors must sometimes figure out where a domain-specific concept "lives" in the DDIS structure
- Readers gain predictability and can skip to known locations
- Validation tools can check structural conformance mechanically

#### Tests

- (Validated by INV-001, INV-006, INV-018) If an author places content in an unexpected location, cross-references will either break or become strained, surfacing the misplacement.

---

### ADR-002: Invariants Must Be Falsifiable, Not Merely True

#### Problem

Should invariants be aspirational properties ("the system should be fast") or formal contracts with concrete violation scenarios?

#### Options

A) **Aspirational invariants** (state desired properties in natural language)
- Pros: Easy to write; captures intent.
- Cons: Cannot be tested; cannot be violated; useless for verification; LLMs cannot generate tests for them.

B) **Formal invariants with proof obligations** (TLA+-style temporal logic)
- Pros: Machine-checkable; mathematically rigorous.
- Cons: Requires formal methods expertise; most implementers can't read them; LLMs may misinterpret formal notation.

C) **Falsifiable invariants** (formal enough to test, informal enough to read)
- Pros: Each invariant has a concrete counterexample and a test; readable by working engineers and LLMs.
- Cons: Not machine-checkable; relies on human judgment for completeness.

#### Decision

**Option C: Falsifiable invariants.** Every invariant must include: a plain-language statement, a semi-formal expression (pseudocode, predicate logic, or precise English), a violation scenario (how could this break?), a validation method (how do we test it?), and a WHY THIS MATTERS annotation.

Confidence: High — falsifiable invariants validated across all DDIS specs.

// WHY NOT Option B? Because the goal is implementation correctness by humans and LLMs, not machine-checked proofs. The authoring cost of full formal verification exceeds the benefit for most systems.

#### Consequences

- Invariants are immediately actionable as test cases
- The violation scenario forces the author to think adversarially
- LLMs can use violation scenarios to generate edge-case tests

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

**Option B: Required.** Cross-references are the mechanism that transforms a collection of sections into a unified specification. Without them, sections exist in isolation and the causal chain (INV-001) cannot be verified. For LLMs, explicit cross-references are the only reliable way to connect constraints to implementation — LLMs cannot infer implicit connections.

Confidence: High — reference graph analysis confirms value of mandatory cross-refs.

// WHY NOT Option A? Recommended means optional. Optional means absent. Every DDIS spec authored without mandatory cross-references has had orphan sections discovered during validation.

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

Confidence: High — this document demonstrates self-bootstrapping feasibility.

// WHY NOT Option A? Because a standard that cannot be applied to itself is suspect. If the structure is good enough for implementation specs, it is good enough for a meta-spec.

#### Consequences

- The standard is simultaneously more trustworthy (tested by self-application) and more complex (meta-level and object-level interleave)
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

**Option B: Voice guidance.** Specifications fail when they are either too dry to read or too casual to trust. DDIS prescribes a specific voice: technically precise but human, the voice of a senior engineer explaining their system to a peer they respect. (See §8.1 for full guidance.)

Confidence: High — voice consistency empirically verified in authored specs.

// WHY NOT Option A? LLMs benefit significantly from explicit voice guidance — without it, they default to generic boilerplate that obscures critical details.

#### Consequences

- Specs feel more unified and readable
- Authors must sometimes revise natural writing habits
- LLMs produce more consistent output when voice is specified

#### Tests

- Qualitative review: sample 5 sections, assess whether each sounds like a senior engineer talking to a peer.

---

### ADR-006: Tiered Constitution over Flat Root [Conditional — modular specs only]

#### Problem

When a DDIS spec is modular for context-window compliance (§0.13), constitutional context must accompany every module bundle. How should this constitutional context be structured?

#### Options

A) **Flat root** — one file containing everything (all invariant definitions, all ADR analysis, all shared types).
- Pros: Simple; one file to maintain; no tier logic.
- Cons: Doesn't scale past ~20 invariants / ~10 ADRs.

B) **Two-tier** — system constitution (full definitions) + modules.
- Pros: Simple; works for small modular specs (< 20 invariants, system constitution ≤ 400 lines).
- Cons: System constitution grows linearly with invariant count; exceeds budget at medium scale.

C) **Three-tier** — system constitution (declarations only) + domain constitution (full definitions) + cross-domain deep context + module.
- Pros: Scales to large specs; domain grouping is already present in well-architected systems.
- Cons: One additional level of indirection; requires domain identification.

#### Decision

**Option C as the full protocol, with Option B as a blessed simplification** for small specs (< 20 invariants, system constitution ≤ 400 lines). The `tier_mode` field in the manifest selects between them.

Confidence: High — validated by modularization tooling and bundle assembly.

// WHY NOT Option A? At scale, the flat root consumes 30–37% of the context budget before the module starts. That's context waste, not context management.

#### Consequences

- Authors must identify 2–5 architectural domains when modularizing
- Two-tier specs can migrate to three-tier without restructuring modules (§0.13.14)

#### Tests

- (Validated by INV-014) Bundle budget compliance confirms tier mode keeps bundles within ceiling.
- (Validated by INV-011) Module completeness confirms constitutional context in each bundle is sufficient.

---

### ADR-007: Cross-Module References Through Constitution Only [Conditional — modular specs only]

#### Problem

When a DDIS spec is modular, how should modules reference content in other modules?

#### Options

A) **Direct references** — "see section 7.3 in the Scheduler module."
- Pros: Natural; mirrors how monolithic cross-references work.
- Cons: Creates invisible dependencies between modules; defeats modularization; violates INV-011.

B) **Through constitution only** — Module A references APP-INV-032 (in constitution). Module A never references Module B's internal sections.
- Pros: Enforces isolation mechanically; bundles are self-contained.
- Cons: Authors must extract all cross-module contracts into the constitution.

#### Decision

**Option B: Through constitution only.** INV-012 enforces this mechanically. Cross-module contracts are expressed as invariants or shared types in the constitution.

Confidence: High — validated by bundle completeness testing (INV-011).

// WHY NOT Option A? It breaks INV-011 (module completeness). If Module A references Module B's internals, Module A's bundle needs Module B's implementation content.

#### Consequences

- All cross-module contracts must be elevated to the constitution
- Modules become truly self-contained implementation units

#### Tests

- (Validated by INV-012) Mechanical check (CHECK-7 in §0.13.11) scans for direct cross-module references.

---

### ADR-008: Negative Specifications Woven Throughout

#### Problem

DDIS 1.0 focuses on what systems DO (algorithms, state machines). But specifying what systems must NOT do is equally important for preventing LLM hallucination. How should negative specifications be integrated?

#### Options

A) **Isolated appendix** — one "Negative Specifications" appendix listing all "do NOT" constraints.
- Pros: Easy to find; one location to audit.
- Cons: Disconnected from the implementation chapters they constrain. An LLM processing Chapter 7 (Scheduler) has lost the negative specs from the appendix. Violates the principle that constraints should be at point of use (§0.2.3).

B) **Woven into each implementation chapter** — each chapter includes its own negative specifications section.
- Pros: Constraints are at point of use; LLM has them in context when implementing that subsystem; no additional lookup required.
- Cons: Higher authoring burden; risk of inconsistency between chapters.

C) **Not formalized** — leave negative specifications to author judgment.
- Pros: No additional authoring burden.
- Cons: Most authors omit them. LLMs hallucinate freely in the gaps.

#### Decision

**Option B: Woven into each implementation chapter.** Every implementation chapter must include a "Negative Specifications" section listing at least 2 explicit "do NOT" constraints (INV-017). Negative specifications are also required in module headers for modular specs (§0.13.5).

Confidence: High — LLM implementation testing confirms negative specs reduce hallucination.

// WHY NOT Option A? The entire value of negative specifications for LLM consumption is that they are present when the LLM is processing the relevant chapter. An appendix is too far away.

// WHY NOT Option C? Empirical observation: LLMs implementing from specs without negative specifications add "helpful" features (caching, optimization, deduplication) that violate unstated invariants. Negative specifications are the highest-leverage improvement for LLM correctness.

#### Consequences

- Each implementation chapter grows by 5–15 lines (the negative specifications section)
- Authors must think adversarially about each subsystem ("what would an LLM add that I don't want?")
- LLM implementation correctness improves measurably

#### Tests

- (Validated by INV-017) Every implementation chapter has ≥ 2 negative specifications.
- Regression test: give a chapter with and without negative specs to an LLM; compare hallucination rate.

---

### ADR-009: LLM Provisions as Pervasive Concern

#### Problem

DDIS 2.0 adds structural provisions for LLM consumption (negative specifications, verification prompts, meta-instructions, structural redundancy). How should these provisions be organized?

#### Options

A) **Single "LLM Chapter"** — add a Chapter 14 covering all LLM considerations.
- Pros: Easy to find; clean separation.
- Cons: LLM provisions are relevant to every element specification, every quality gate, and every authoring step. Isolating them in one chapter means they are absent where they matter. An LLM writing a new DDIS spec reads Chapter 14 once, then forgets the guidance while writing implementation chapters.

B) **Woven throughout** — integrate LLM provisions into every relevant section: element specifications (how to structure for LLM parsing), quality gates (LLM-specific validation), invariants (properties that prevent LLM failure modes), authoring guidance (LLM validation steps).
- Pros: Provisions are at point of use; no section can be read without encountering the LLM lens; self-bootstrapping property is maintained.
- Cons: No single location to audit all LLM provisions; harder to enumerate.

C) **Separate addendum document** — publish a companion "DDIS for LLMs" guide.
- Pros: Keeps the main standard clean.
- Cons: Two documents to maintain; inevitable divergence; violates INV-008 (self-containment).

#### Decision

**Option B: Woven throughout.** LLM optimization is a pervasive concern — like security or performance, it cannot be addressed by a single chapter. The LLM Consumption Model (§0.2.3) provides the foundational justification. INV-017 through INV-020 enforce the structural properties. Each element specification in PART II includes LLM-specific quality criteria. Gate 7 provides end-to-end validation.

Confidence: High — this document demonstrates the woven approach.

// WHY NOT Option A? The "Afterthought LLM Section" anti-pattern. Adding one chapter and calling it done is precisely the failure mode this decision prevents.

// WHY NOT Option C? Violates INV-008 and the self-bootstrapping property. The standard must be self-contained.

#### Consequences

- The standard is longer but more integrated
- LLM provisions cannot be skipped by a reader working on any section
- The standard itself serves as the example of how to weave LLM considerations

#### Tests

- (Validated by INV-018) Structural predictability is maintained despite the additional provisions.
- Audit: verify that at least 60% of element specifications in PART II reference the LLM consumption model or an LLM-specific invariant.

---

### ADR-010: Verification Prompts Required per Implementation Chapter

#### Problem

How should LLM implementers verify their own output against the spec?

#### Options

A) **Single validation section** — one "Verification" chapter at the end of the spec.
- Pros: All verification in one place.
- Cons: By the time the LLM reaches it, the implementation is complete. Errors detected late are expensive to fix. The verification section may exceed context window if the spec is long.

B) **Per-chapter verification prompts** — each implementation chapter ends with a prompt the LLM can use to self-check its output.
- Pros: Verification is incremental; errors caught early; prompt is in context with the implementation it checks; LLMs can use it as a "test" for their own output.
- Cons: Additional authoring burden (~5–10 lines per chapter).

C) **External validation tool** — a separate tool checks the implementation against the spec.
- Pros: Automated; repeatable.
- Cons: Requires tooling that doesn't exist for most domains; doesn't help during the implementation phase.

#### Decision

**Option B: Per-chapter verification prompts.** Each implementation chapter must end with a verification prompt (INV-018, §5.6). The prompt lists the invariants this chapter must preserve and the properties the implementation must demonstrate.

Confidence: High — LLM self-check testing confirms verification prompts catch errors.

// WHY NOT Option A? Late verification means late error detection. The cost of fixing an error increases with the amount of dependent code already written.

// WHY NOT Option C? DDIS is a document standard, not a tooling standard. Verification prompts work with any LLM and require no external infrastructure.

#### Consequences

- Each implementation chapter grows by 5–10 lines (the verification prompt)
- LLMs can self-check incrementally, catching errors before they propagate
- The verification prompts also serve as documentation of what each chapter must achieve

#### Tests

- (Validated by INV-018) Every implementation chapter ends with a verification prompt following the template in §5.6.
- Effectiveness test: give a chapter + verification prompt to an LLM. Measure whether the LLM catches errors it would otherwise miss.

---

### ADR-011: Standardized Cross-Reference Syntax

#### Problem

DDIS 2.0 recommended consistent cross-reference conventions (§10.1) but did not mandate a specific syntax. Should DDIS formalize cross-reference syntax as an invariant?

#### Options

A) **Formalized standard syntax** — define exact reference forms, mandate via INV-021, enable automated validation.
- Pros: Machine-parseable; enables automated graph construction, stale detection (INV-020), orphan detection (INV-006); LLMs can reliably resolve references.
- Cons: Authoring friction; authors must learn and use specific forms.

B) **Recommended conventions only** (status quo from DDIS 2.0) — continue recommending forms without mandating them.
- Pros: Low authoring friction; flexible.
- Cons: No automated validation; LLMs cannot reliably resolve non-standard references; INV-006 and INV-020 require manual audit.

C) **Machine-readable annotations** — use HTML-like tags (e.g., `<ref target="INV-003"/>`) for machine parsing while displaying human-readable text.
- Pros: Fully machine-parseable; human display is flexible.
- Cons: Clutters Markdown source; non-standard in Markdown ecosystem; LLMs may hallucinate incorrect tag syntax.

#### Decision

**Option A: Formalized standard syntax.** The parenthetical reference forms are already natural in technical writing and widely used in DDIS 2.0. Mandating them via INV-021 enables automated validation with zero new notation — just consistency of existing practice.

Confidence: High — the parenthetical forms are already in widespread use across DDIS specs.

// WHY NOT Option B? Without a standard, automated validation of INV-006 and INV-020 is impossible. These invariants are core to DDIS's value; making them machine-checkable is high leverage.

// WHY NOT Option C? HTML-like tags are foreign to Markdown and would degrade readability. The parenthetical forms already serve as effective lookup keys for both humans and LLMs.

#### Consequences

- Automated tools can validate cross-reference graphs
- INV-020 (Restatement Freshness) can be checked by extracting restatement-reference pairs
- Authors use the same forms they were already using (low transition cost)

#### Tests

- (Validated by INV-021) Mechanical scan for non-standard reference forms.
- (Validated by INV-006) Reference graph construction from standardized references detects orphans.

---

### ADR-012: ADR Confidence Levels

#### Problem

Not all architectural decisions carry the same certainty. Some are firm ("validated by prototype"), others are provisional ("best guess, revisit after spike"). Should DDIS formalize confidence levels on decisions?

#### Options

A) **Formalized confidence field** — add a required Confidence field (high/medium/low) to every ADR, with defined criteria per level and handling guidance.
- Pros: LLMs know which decisions to implement firmly vs. which to design for changeability; spec authors communicate uncertainty explicitly; operational playbook can flag low-confidence ADRs for early spikes.
- Cons: Additional authoring burden per ADR; risk of every ADR being marked "medium" by lazy authors.

B) **Optional annotation** — recommend but don't require a confidence field.
- Pros: Low friction; available when useful.
- Cons: Optional means absent. LLMs assume all decisions are equally certain. Low-confidence decisions don't get flagged for re-evaluation.

C) **No confidence levels** (status quo from DDIS 2.0) — all ADRs are treated equally.
- Pros: Simplest.
- Cons: No signal about decision certainty; provisional decisions are treated as permanent; LLMs implement workarounds as if they were deliberate architecture.

#### Decision

**Option A: Formalized confidence field, required on all ADRs.** The field is lightweight (one line) and high-value. Criteria:

| Confidence | Criteria | LLM Implementation Guidance |
|---|---|---|
| **High** | Validated by spike, prototype, or production experience | Implement directly; optimize freely |
| **Medium** | Informed by analysis but not validated empirically | Implement, but isolate behind an interface for potential replacement |
| **Low** | Best guess; depends on unknowns | Implement minimal version; flag in operational playbook for spike |

Confidence: High — this pattern is well-established in architectural practice (ATAM, risk-driven architecture).

// WHY NOT Option B? Because optional means absent when it matters most. The ADRs where confidence levels are most valuable (low-confidence decisions) are exactly the ones where authors are least likely to volunteer uncertainty.

// WHY NOT Option C? LLMs treat all ADR decisions as equally authoritative. Without confidence signals, an LLM will implement a speculative caching strategy with the same permanence as a proven storage architecture.

#### Consequences

- Every ADR includes a Confidence field
- Low-confidence ADRs are flagged in the operational playbook for early spikes (§6.1.1)
- LLMs can adjust their implementation strategy based on decision certainty

#### Tests

- (Validated by INV-018) Every ADR includes the Confidence field per the template.
- Qualitative: review 3 low-confidence ADRs; verify each has a spike or re-evaluation plan in the operational playbook.

---

## 0.7 Quality Gates

A DDIS-conforming specification is "done" when all quality gates pass. Gates are ordered by priority; a failing Gate 1 makes Gates 2–8 irrelevant.

**Gate 1: Structural Conformance**
All required elements from §0.3 are present, including negative specifications per implementation chapter and verification prompts. Mechanical check.

**Gate 2: Causal Chain Integrity**
Five randomly selected implementation sections trace backward to the formal model without breaks. (Validates INV-001.)

**Gate 3: Decision Coverage**
An adversarial reviewer identifies zero "obvious alternatives" not covered by an ADR. (Validates INV-002.)

**Gate 4: Invariant Falsifiability**
Every invariant has a constructible counterexample, a named test, and a WHY THIS MATTERS annotation. (Validates INV-003.)

**Gate 5: Cross-Reference Web**
The reference graph has no orphan sections and the graph is connected. (Validates INV-006.)

**Gate 6: Implementation Readiness**
A competent implementer (or LLM), given only the spec and public references, can begin implementing without asking clarifying questions about architecture, algorithms, data models, or invariants. Questions about micro-level implementation details (variable names, error message wording) are acceptable.

**Gate 7: LLM Implementation Readiness**
An LLM, given one implementation chapter plus the relevant invariants, ADRs, glossary, and negative specifications, produces an implementation that: (a) preserves all referenced invariants, (b) does not hallucinate requirements not in the spec, (c) does not violate any negative specification, and (d) passes the chapter's verification prompt. Tested on at least 2 representative chapters. (Validates INV-017, INV-018, INV-019, INV-020.)

**Gate 7 measurement procedure**: Select 2 implementation chapters with the highest invariant density. For each: (1) assemble the chapter + referenced invariants + glossary into a prompt. (2) Ask an LLM to implement the subsystem. (3) Review the output for hallucinated features, invariant violations, and negative specification violations. (4) Run the verification prompt against the output. If any chapter fails, Gate 7 fails.

**Gate 8: Automated Consistency**
All mechanical checks pass: (a) reference graph has no orphans and uses only standard syntax (INV-021), (b) all conditional sections have explicit triggers and are included when applicable (INV-022), (c) all restatements match their canonical definitions (INV-020), (d) all elements follow their type template (INV-018), (e) every ADR includes a Confidence field. (See §12.4 for the full automated testing procedure.)

### Modularization Quality Gates [Conditional — modular specs only]

In addition to Gates 1–8, modular specs must pass these gates. A failing Gate M-1 makes Gates M-2 through M-5 irrelevant.

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

DDIS 2.0 is "done" when:
- This document passes Gates 1–8 applied to itself
- Gate 8 automated checks pass with zero FAIL results
- At least one non-trivial specification has been written conforming to DDIS and the author reports that the standard was sufficient (no structural gaps required working around)
- The Glossary (Appendix A) covers all DDIS-specific terminology
- Gate 7 has been demonstrated on at least one conforming spec

## 0.8 Performance Budgets (for Specifications, Not Software)

Specifications have performance characteristics too. A spec that takes 40 hours to read is too long. A spec that takes 2 hours to read probably omits critical details.

### 0.8.1 Specification Size Budgets

| System Complexity | Target Spec Length | Rationale |
|---|---|---|
| Small (single crate, < 5K LOC target) | 500–1,500 lines | Enough for formal model + invariants + key ADRs |
| Medium (multi-crate, 5K–50K LOC target) | 1,500–5,000 lines | Full DDIS treatment |
| Large (multi-service, > 50K LOC target) | 5,000–15,000 lines | May split into sub-specs or modularize per §0.13 |

### 0.8.2 Proportional Weight Guide

Not all PART sections are equal. The following proportions prevent bloat in some areas and starvation in others. These are guidelines — domain-specific specs may adjust by ±20%.

| Section | % of Total | Why |
|---|---|---|
| Preamble + PART 0 | 15–20% | Dense: formal model, invariants, ADRs, quality gates |
| PART I: Foundations | 8–12% | First principles, state machines, complexity analysis |
| PART II: Core Implementation | 35–45% | THE HEART: algorithms, data structures, protocols, examples, negative specs |
| PART III: Interfaces | 8–12% | API schemas, adapters, external contracts |
| PART IV: Operations | 10–15% | Testing, operational playbook, roadmap |
| Appendices + Part X | 10–15% | Reference material, glossary, error taxonomy, master TODO |

### 0.8.3 Authoring Time Budgets

| Element | Expected Authoring Time | Notes |
|---|---|---|
| First-principles model | 2–4 hours | Hardest part; requires deep domain understanding |
| One invariant (high quality) | 15–30 minutes | Including violation scenario, test strategy, WHY THIS MATTERS |
| One ADR (high quality) | 30–60 minutes | Including genuine alternative analysis |
| One implementation chapter | 2–4 hours | Including algorithm, examples, negative specs, verification prompt |
| End-to-end trace | 1–2 hours | Requires all subsystems to be drafted first |
| Glossary | 1–2 hours | Best done last, by extracting terms from the full spec |
| Negative specifications per chapter | 15–30 minutes | Think adversarially: what would an LLM add unprompted? |

### 0.8.4 Measurement Methods

| Metric | How to Measure | Target |
|---|---|---|
| Time to first implementer question | Give spec to implementer, timestamp first clarifying question | > 2 hours of implementation before first question |
| LLM hallucination rate | Count features in LLM output not in spec, divide by total features | < 5% per chapter |
| Cross-reference density | Count edges in reference graph, divide by sections | ≥ 2.0 edges/section |
| Invariant falsifiability | Count invariants with constructible counterexamples, divide by total | 100% |
| Gate passage rate | Run all gates, count passing | 100% for Gates 1–8 |
| Automated check pass rate | Run Gate 8 checks (§12.4), count passing | 100% PASS on all checks |

---

## 0.9 Public API Surface (of DDIS Itself)

DDIS exposes the following "API" to specification authors:

1. **Document Structure Template** (§0.3) — the skeleton to fill in.
2. **Element Specifications** (PART II) — what each structural element must contain.
3. **Quality Criteria** (§0.5 invariants, §0.7 gates) — how to validate conformance.
4. **Voice and Style Guide** (PART III, §8.1) — how to write well within the structure.
5. **Anti-Pattern Catalog** (PART III, §8.3) — what bad specs look like.
6. **LLM Consumption Model** (§0.2.3) — how to structure elements for LLM implementers.
7. **Completeness Checklist** (Part X) — mechanical conformance validation.
8. **Specification Error Taxonomy** (Appendix D) — classification of authoring errors.
9. **Composition Protocol** (§0.14) — how multiple DDIS specs reference each other.

---

## 0.10 Open Questions (for DDIS 3.0)

1. **Multi-document specs (partially addressed)**: §0.14 defines basic composition rules. Remaining open: How do negative specifications compose across spec boundaries? How are cross-spec cascade effects tracked? What is the governance model when specs have different owners?

2. **Formal verification bridge**: Should DDIS define a pathway from falsifiable invariants to machine-checked properties for safety-critical systems?

3. **Automated Gate 7 testing**: Can LLM implementation readiness be automated as a CI check that feeds a spec chapter to an LLM and validates the output?

4. **Spec diffing**: When a spec evolves from version N to N+1, how should changes be communicated to implementers already working from version N? Should DDIS define a changelog format that highlights invariant changes, superseded ADRs, and new negative specifications?

5. **Domain-specific DDIS profiles**: Should DDIS define profiles (e.g., "safety-critical", "API-only", "data pipeline") with pre-configured conditional sections? A safety-critical profile might mandate formal verification (currently Open Question 2) while an API-only profile might skip operational playbooks.

---
