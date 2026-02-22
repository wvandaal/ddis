
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

## 0.7 Quality Gates

A DDIS-conforming specification is "done" when all quality gates pass. Gates are ordered by priority; a failing Gate 1 makes Gates 2–8 irrelevant.

**Gate 1: Structural Conformance**
All required elements from §0.3 are present, including negative specifications and verification prompts per implementation chapter. Mechanical check.

**Gate 2: Causal Chain Integrity**
Five randomly selected implementation sections trace backward to the formal model without breaks. (Validates [[INV-001|causal traceability]].)

**Gate 3: Decision Coverage**
An adversarial reviewer identifies zero "obvious alternatives" not covered by an ADR. (Validates [[INV-002|decision completeness]].)

**Gate 4: Invariant Falsifiability**
Every invariant has a constructible counterexample and a named test. (Validates [[INV-003|invariant falsifiability]].)

**Gate 5: Cross-Reference Web**
The reference graph has no orphan sections and the graph is connected. All invariant/ADR references include restated substance. (Validates [[INV-006|cross-reference density]], [[INV-018|substance restated]].)

**Gate 6: Implementation Readiness**
A competent implementer (or LLM), given only the spec and public references, can begin implementing without asking clarifying questions about architecture, algorithms, data models, or invariants.

**Gate 7: LLM Implementation Readiness**
An LLM given one implementation chapter (plus the glossary and restated invariants from that chapter) produces an implementation that: (a) does not hallucinate features not in the spec, (b) respects all negative specifications in the chapter, (c) passes the chapter's verification prompt. Tested on at least 2 representative chapters. (Validates [[INV-017|negative spec coverage]], [[INV-018|substance restated]], [[INV-019|verification prompt coverage]].)

**Test procedure for Gate 7:** Extract one implementation chapter. Include only that chapter, the glossary, and the invariants restated within it. Give to an LLM with the prompt: "Implement this subsystem. Follow all constraints. After implementing, answer the verification prompt at the end." Evaluate: (1) Did the LLM add features not in the spec? (2) Did it violate any negative specification? (3) Did it correctly answer the verification prompt? If any answer is yes/yes/no, Gate 7 fails for that chapter.

**Gate 8: Specification Testability**
Automated validation tooling can parse the spec's cross-references, build a reference graph, check density, and detect potential staleness — without manual intervention. (Validates [[INV-022|parseable cross-refs]], [[INV-021|proportional weight]].)

**Test procedure for Gate 8:** Run a cross-reference parser against the spec. Verify: (1) All `[[TARGET|substance]]` references resolve to existing elements. (2) No section is an orphan in the reference graph. (3) Proportional weight deviations are within tolerance or annotated. (4) Every ADR has a Confidence field. If any automated check fails, Gate 8 fails.

### Modularization Quality Gates [Conditional — modular specs only]

In addition to Gates 1–8, modular specs must pass these gates.

**Gate M-1: Consistency Checks**
All nine mechanical checks (CHECK-1 through CHECK-9 in §0.13.11) pass with zero errors.

**Gate M-2: Bundle Budget Compliance**
Every assembled bundle is under the hard ceiling. Fewer than 20% of bundles exceed the target line count. (Validates [[INV-014|bundle budget compliance]].)

**Gate M-3: LLM Bundle Sufficiency**
An LLM receiving one assembled bundle produces zero questions that require another module's implementation content. (Validates [[INV-011|module completeness]].)

**Gate M-4: Declaration-Definition Faithfulness**
Every Tier 1 invariant declaration is a faithful summary of its Tier 2 full definition. (Validates [[INV-015|declaration-definition consistency]].)

**Gate M-5: Cascade Simulation**
A simulated change to one invariant correctly identifies all affected modules via the cascade protocol. (Validates [[INV-016|manifest-spec synchronization]].)

### Definition of Done (for this standard)

DDIS 3.0 is "done" when:
- This document passes Gates 1–8 applied to itself
- At least one non-trivial specification has been written conforming to DDIS and the author reports structural sufficiency
- The Glossary (Appendix A) covers all DDIS-specific terminology
- The LLM provisions are woven throughout (ADR-011), not isolated
- Cross-references use machine-readable syntax ([[INV-022|parseable cross-refs]])

## 0.8 Performance Budgets (for Specifications, Not Software)

### 0.8.1 Specification Size Budgets

| System Complexity | Target Spec Length | Rationale |
|---|---|---|
| Small (single crate, < 5K LOC target) | 500–1,500 lines | Formal model + invariants + key ADRs |
| Medium (multi-crate, 5K–50K LOC target) | 1,500–5,000 lines | Full DDIS treatment |
| Large (multi-service, > 50K LOC target) | 5,000–15,000 lines | May split via §0.13 modularization |

**LLM context window guidance:** If a spec exceeds the target LLM's context window minus a 25% reasoning reserve, modularization (§0.13) is required, not optional. Individual implementation chapters should not exceed 500 lines — if a chapter is longer, consider splitting the subsystem.

### 0.8.2 Proportional Weight Guide

| Section | % of Total | Why |
|---|---|---|
| Preamble + PART 0 | 15–20% | Dense: formal model, invariants, ADRs, quality gates |
| PART I: Foundations | 8–12% | First principles, state machines, complexity analysis |
| PART II: Core Implementation | 35–45% | THE HEART: algorithms, data structures, protocols, examples |
| PART III: Interfaces | 8–12% | API schemas, adapters, external contracts |
| PART IV: Operations | 10–15% | Testing, operational playbook, roadmap |
| Appendices + Part X | 10–15% | Reference material, glossary, master TODO |

// DEVIATION NOTE: For meta-standards (specs about specs), PART 0 naturally exceeds 15-20% because the standard's invariants, ADRs, and protocol definitions ARE the core content. The proportional weight invariant ([[INV-021|weight compliance]]) accommodates this via its tolerance band and WHY NOT annotation mechanism.

### 0.8.3 Authoring Time Budgets

| Element | Expected Authoring Time | Notes |
|---|---|---|
| First-principles model | 2–4 hours | Hardest part; requires deep domain understanding |
| One invariant (high quality) | 15–30 minutes | Including violation scenario and test strategy |
| One ADR (high quality) | 30–60 minutes | Including genuine alternative analysis |
| One implementation chapter | 2–4 hours | Including algorithm, examples, test strategy |
| Negative specs per chapter | 10–15 minutes | Think adversarially: what would an LLM get wrong? |
| Verification prompt per chapter | 5–10 minutes | Reference specific invariants and negative specs |
| End-to-end trace | 1–2 hours | Requires all subsystems to be drafted first |
| Glossary | 1–2 hours | Best done last, by extracting terms from the full spec |
| Machine-readable ref conversion | 30–60 minutes | Convert freeform refs to `[[ID\|substance]]` syntax |

---

## 0.9 Public API Surface (of DDIS Itself)

DDIS exposes the following "API" to specification authors:

1. **Document Structure Template** (§0.3) — the skeleton to fill in.
2. **Element Specifications** (PART II) — what each structural element must contain.
3. **Quality Criteria** (§0.5 invariants, §0.7 gates) — how to validate conformance.
4. **Voice and Style Guide** (PART III, §8.1) — how to write well within the structure.
5. **Anti-Pattern Catalog** (PART III, §8.3) — what bad specs look like.
6. **LLM Consumption Model** (§0.2.3) — how to structure elements for LLM implementers.
7. **Composability Protocol** (§0.2.5) — how to reference external DDIS specs.
8. **Machine-Readable Syntax** (§10.3) — parseable cross-reference format.
9. **Completeness Checklist** (Part X) — mechanical conformance validation.
10. **Specification Error Taxonomy** (Appendix D) — classification of authoring errors.
11. **Automated Testing Framework** (§12.3) — programmatic spec validation.

---

## 0.10 Open Questions (for DDIS 4.0)

1. ~~**Machine-readable cross-references**~~ — **Resolved in 3.0** by [[ADR-013|wiki-link syntax]] and [[INV-022|parseable cross-refs]].

2. **Multi-document specs**: For very large systems, how should sub-specs reference each other beyond the composability protocol (§0.2.5)? Should there be a "meta-manifest" for multi-spec systems?

3. **Formal verification bridge**: Should DDIS define a pathway from falsifiable invariants to machine-checked properties for safety-critical systems?

4. **Automated Gate 7 testing**: Can LLM implementation readiness be automated as a CI check that feeds a spec chapter to an LLM and validates the output? (Gate 8 automates structural checks; Gate 7 remains semantic.)

5. **Spec diffing**: When a spec evolves, how should changes be tracked at the element level (not just text diffs)? Should DDIS define a semantic diff format for invariants, ADRs, and cross-references?

---

## 0.13 Modularization Protocol [Conditional]

This section is REQUIRED when the monolithic specification exceeds 4,000 lines or when the target context window cannot hold the full spec plus a meaningful working budget for LLM reasoning. It is OPTIONAL but recommended for specs between 2,500–4,000 lines.

> Namespace note: INV-001 through INV-022 and ADR-001 through ADR-013 are DDIS meta-standard invariants/ADRs. Application specs define their OWN namespace (e.g., APP-INV-001) — never reuse the meta-standard's identifiers.

### 0.13.1 The Scaling Problem

A DDIS spec's value depends on the implementer holding sufficient context to produce correct output. When the spec exceeds the implementer's context window, two failure modes emerge:

1. **Truncation**: The LLM silently drops content from the beginning, losing invariants and the formal model.
2. **Naive splitting**: Arbitrary file splits break cross-references and orphan invariants.

The modularization protocol prevents both failures by defining a principled decomposition with formal completeness guarantees. (Motivated by [[INV-008|self-containment]].)

### 0.13.2 Core Concepts

**Monolith**: A DDIS spec as a single document. All specs start as monoliths.

**Module**: A self-contained spec unit covering one major subsystem. Each module corresponds to one PART II chapter. Always assembled into a bundle with constitutional context.

**Constitution**: Cross-cutting material constraining all modules. Contains the formal model, invariants, ADRs, quality gates, architecture overview, glossary, and performance budgets. Organized in tiers.

**Domain**: An architectural grouping of related modules sharing tighter coupling with each other than with modules in other domains.

**Bundle**: The assembled document sent to an LLM: system constitution + domain constitution + cross-domain deep context + module. The unit of LLM consumption.

**Manifest**: A machine-readable YAML file declaring all modules, their domain membership, invariant ownership, cross-module interfaces, and assembly rules.

### 0.13.3 The Tiered Constitution

The constitution is organized in three tiers. Each tier has a hard line budget, a clear scope, and NO overlapping content between tiers. (Locked by [[ADR-006|tiered constitution over flat root]].)

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
+--------------------------------------------------------------+
| TIER 2: Domain Constitution (200-500 lines, per-domain)      |
|  - Domain formal model                                       |
|  - FULL DEFINITIONS for invariants owned by this domain      |
|  - FULL ANALYSIS for ADRs decided within this domain         |
|  - Cross-domain interface contracts                          |
|  - Domain-level performance budgets                          |
+--------------------------------------------------------------+
| TIER 3: Cross-Domain Deep Context (0-600 lines, per-module)  |
|  - Full definitions for OTHER-domain invariants this module  |
|    INTERFACES with                                           |
|  - Full ADR specs from OTHER domains affecting this module   |
|  - Interface contracts with adjacent modules in OTHER domains|
|  - Shared types from OTHER domains used by this module       |
+--------------------------------------------------------------+
| MODULE (800-3,000 lines)                                     |
|  - Module header (ownership, interfaces, negative specs)     |
|  - Full PART II content for this subsystem                   |
|  - Negative specifications, verification prompt,             |
|    meta-instructions                                         |
+--------------------------------------------------------------+

Assembled bundle: Tier 1 + Tier 2 + Tier 3 + Module
Target budget:    1,200 - 4,500 lines per bundle
Hard ceiling:     5,000 lines
```

### 0.13.4 Invariant Declarations vs. Definitions

**Declaration** (Tier 1, ~1 line):
```
APP-INV-017: Event log is append-only -- Owner: EventStore -- Domain: Storage
```

**Definition** (Tier 2, ~10-20 lines):
```
**APP-INV-017: Event Log Append-Only**
*Events, once written, are never modified or deleted.*
  ∀ event ∈ EventLog, ∀ t1 < t2:
    event ∈ EventLog(t1) → event ∈ EventLog(t2) ∧ event(t1) = event(t2)
Violation scenario: A compaction routine rewrites old events.
Validation: Write 1000 events, snapshot, run any operation, compare prefix byte-for-byte.
// WHY THIS MATTERS: Append-only is the foundation of deterministic replay.
```

### 0.13.5 Module Header (Required per Module)

```yaml
# Module Header: [Module Name]
# Domain: [Domain Name]
# Maintains: APP-INV-017, APP-INV-018, APP-INV-019
# Interfaces: APP-INV-003 (via EventStore), APP-INV-032 (via Scheduler)
# Implements: APP-ADR-003, APP-ADR-011
# Adjacent modules: EventStore (read types), Scheduler (publish events)
# Assembly: Tier 1 + Storage domain + cross-domain deep (Coordination interfaces)
#
# NEGATIVE SPECIFICATION (what this module must NOT do):
# - Must NOT directly access TUI rendering state (use event bus)
# - Must NOT bypass the reservation system for file writes
# - Must NOT assume event ordering beyond the guarantees in APP-INV-017
```

### 0.13.6 Cross-Module Reference Rules

**Rule 1: Cross-module references go through the constitution, never direct.** ([[INV-012|cross-module isolation]], [[ADR-007|through constitution only]].)

```
BAD:  "See section 7.3 in the Scheduler chapter for the dispatch algorithm"
GOOD: "This subsystem publishes SchedulerReady events (see [[APP-INV-032|
       fair scheduling with no starvation]], maintained by Scheduler module)"
```

**Rule 2: Shared types are defined in the constitution, not in any module.**

**Rule 3: The end-to-end trace is a special cross-cutting module.**

### 0.13.7 Modularization Decision Flowchart

```
Is spec > 4,000 lines?
  |-- No  -> Is spec > 2,500 lines AND target context < 8K lines?
  |           |-- No  -> MONOLITH (stop here)
  |           +-- Yes -> MODULE (recommended)
  +-- Yes -> MODULE (required)
             |
             How many invariants + ADRs total?
             |-- < 20 total AND system constitution ≤ 400 lines
             |    -> TWO-TIER
             +-- >= 20 total OR system constitution > 400 lines
                  -> THREE-TIER
```

#### 0.13.7.1 Two-Tier Simplification

For small modular specs, the domain tier can be skipped. In two-tier mode:
- **Tier 1**: Contains BOTH declarations AND full definitions (fits in ≤ 400 lines).
- **Tier 2 and Tier 3**: SKIPPED.

Assembly: `system_constitution + module → bundle`.

### 0.13.8 File Layout

```
project-spec/
├── manifest.yaml                    # Module manifest (§0.13.9)
├── constitution/
│   ├── system.md                    # Tier 1: declarations, glossary, gates
│   ├── domain_storage.md            # Tier 2: Storage domain definitions
│   ├── domain_coordination.md       # Tier 2: Coordination domain definitions
│   └── domain_presentation.md       # Tier 2: Presentation domain definitions
├── deep/
│   ├── scheduler_cross.md           # Tier 3: cross-domain context for Scheduler
│   └── tui_cross.md                 # Tier 3: cross-domain context for TUI
├── modules/
│   ├── event_store.md               # Module: EventStore subsystem
│   ├── scheduler.md                 # Module: Scheduler subsystem
│   └── tui_renderer.md              # Module: TUI Renderer subsystem
└── bundles/                         # Generated (gitignored)
    ├── event_store_bundle.md        # Assembled: Tier1 + Storage + module
    ├── scheduler_bundle.md          # Assembled: Tier1 + Coordination + deep + module
    └── tui_renderer_bundle.md       # Assembled: Tier1 + Presentation + deep + module
```

### 0.13.9 Manifest Schema

```yaml
# manifest.yaml
version: "1.0"
tier_mode: three-tier  # or "two-tier"

context_budget:
  target_lines: 4000
  hard_ceiling_lines: 5000

domains:
  storage:
    constitution: constitution/domain_storage.md
    description: "Event persistence, snapshots, replay"
  coordination:
    constitution: constitution/domain_coordination.md
    description: "Task scheduling, agent management, conflict resolution"

modules:
  event_store:
    path: modules/event_store.md
    domain: storage
    maintains: [APP-INV-017, APP-INV-018, APP-INV-019]
    interfaces: [APP-INV-003]
    deep_context:
      - constitution/domain_coordination.md#APP-INV-032
  scheduler:
    path: modules/scheduler.md
    domain: coordination
    maintains: [APP-INV-032, APP-INV-033]
    interfaces: [APP-INV-017]
    deep_context:
      - constitution/domain_storage.md#APP-INV-017

invariant_registry:
  APP-INV-017:
    owner: event_store
    domain: storage
    one_line: "Event log is append-only"
  APP-INV-032:
    owner: scheduler
    domain: coordination
    one_line: "Fair scheduling with no starvation"
```

### 0.13.10 Assembly Rules

**Three-tier assembly** for module M in domain D:
1. Start with `constitution/system.md` (Tier 1)
2. Append `constitution/domain_D.md` (Tier 2)
3. For each entry in M's `deep_context`: append the referenced sections (Tier 3)
4. Append `modules/M.md` (Module)
5. Validate: total lines ≤ `hard_ceiling_lines`

**Two-tier assembly**: Step 1 (Tier 1 with full definitions) + Step 4 (Module). Skip steps 2-3.

**Budget validation**: If assembled bundle exceeds `target_lines`, emit a warning. If it exceeds `hard_ceiling_lines`, fail the assembly and require the module to be split.

### 0.13.11 Consistency Checks

Nine mechanical checks, each with a formal expression:

| Check | What It Validates | Formal Expression |
|---|---|---|
| CHECK-1 | Ownership uniqueness | `∀ inv: count(modules maintaining inv) ≤ 1` |
| CHECK-2 | Ownership coverage | `∀ inv ∈ registry: ∃ module ∈ manifest: inv ∈ module.maintains` |
| CHECK-3 | Interface symmetry | `∀ M: ∀ inv ∈ M.interfaces: ∃ N ≠ M: inv ∈ N.maintains` |
| CHECK-4 | Domain consistency | `∀ M: M.domain ∈ manifest.domains` |
| CHECK-5 | Bundle budget | `∀ M: line_count(ASSEMBLE(M)) ≤ hard_ceiling` |
| CHECK-6 | Declaration existence | `∀ inv ∈ any module: inv ∈ system_constitution` |
| CHECK-7 | Cross-module isolation | `∀ M: M.refs ∩ other_module_internal_sections = ∅` |
| CHECK-8 | Deep context sufficiency | `∀ M: ∀ inv ∈ M.interfaces: inv definition ∈ ASSEMBLE(M)` |
| CHECK-9 | Manifest-filesystem sync | `∀ path ∈ manifest: file_exists(path) ∧ ∀ file ∈ modules/: file ∈ manifest` |

### 0.13.12 Cascade Protocol

When a constitutional element changes, affected modules must be re-validated:

1. **Identify affected modules**: For a changed invariant INV, find all modules where INV ∈ `maintains` ∪ `interfaces`.
2. **Re-validate bundles**: Re-assemble affected bundles and run CHECK-5 (budget) and CHECK-8 (deep context sufficiency).
3. **Update restatements**: Every module that references the changed invariant must update its substance restatement ([[INV-018|substance restated]]).
4. **Run Gate M-4**: Verify declaration-definition consistency for the changed element.
5. **Document the cascade**: Record which modules were affected and which checks re-run.

### 0.13.13 Migration Procedure (Monolith to Modules)

1. Assess: Is the spec over 4,000 lines or does the target LLM struggle with it?
2. Identify domains: Group related subsystems (2-5 domains typical).
3. Choose tier mode: Use the flowchart (§0.13.7).
4. Extract constitution: Move invariants, ADRs, glossary, gates to `constitution/system.md`.
5. Extract domain constitutions: Move full definitions to domain files.
6. Extract modules: Move each PART II chapter to `modules/`.
7. Write module headers (§0.13.5) for each module.
8. Write manifest (§0.13.9).
9. Run all consistency checks (§0.13.11) and fix violations.
