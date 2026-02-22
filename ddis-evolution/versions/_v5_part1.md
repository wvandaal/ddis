# DDIS: Decision-Driven Implementation Specification Standard

## Version 5.0 — A Self-Bootstrapping Meta-Specification

> Design goal: **A formal standard for writing implementation specifications that are precise enough for an LLM or junior engineer to implement correctly without guessing, while remaining readable enough that a senior engineer would choose to read them voluntarily.**

> Core promise: A specification conforming to DDIS contains everything needed to implement the described system — architecture, algorithms, invariants, decisions, test strategies, performance budgets, negative constraints, and execution plan — in a single cohesive document where every section earns its place by serving the sections around it, without requiring the implementer to guess at unstated requirements.

> Document note (important):
> This standard is **self-bootstrapping**: it is written in the format it defines.
> Every structural element prescribed by DDIS is demonstrated in this document.
> Where this document says "the spec must include X," this document includes X — about itself.
> Code blocks are design sketches for illustration. The correctness contract lives in the
> invariants, not in any particular syntax.
> **LLM implementers**: treat invariants as ground truth. When a code sketch contradicts an invariant, the invariant wins.

> How to use this standard (practical):
> 1) Read **PART 0** once end-to-end: understand what DDIS requires, why, and how elements connect.
> 2) Lock your spec's **churn-magnets** via ADRs before writing implementation sections.
> 3) Write your spec following the **Document Structure** (§0.3), using PART II as the element-by-element reference.
> 4) For each implementation chapter, include **negative specifications** (what the subsystem must NOT do) and a **verification prompt** (self-check for the implementer). See §3.8 and §5.6.
> 5) Validate against the **Quality Gates** (§0.7) — including Gate 7 (LLM Implementation Readiness) and Gate 8 (Specification Testability) — and the **Completeness Checklist** (Part X) before considering the spec "done."
> 6) Treat the **cross-reference web** as a product requirement, not polish — it is the mechanism that makes the spec cohere. Cross-references must **restate substance**, not just cite IDs (INV-018). Use **machine-readable syntax** (§10.3) to enable automated validation.
> 7) If your spec exceeds **2,500 lines** or your target LLM's context window, read **§0.13 (Modularization Protocol)** and decompose into a manifest-driven module structure.
> 8) If your system depends on another DDIS-specified system, follow the **Composability Protocol** (§0.2.5) for cross-spec references.
> 9) Choose your **conformance level** (§0.2.6): Essential, Standard, or Complete. Start with Essential for first specs; graduate to Standard once comfortable.
> 10) Use the **LLM Validation Protocol** (§12.5) to have an LLM review your spec for DDIS conformance before manual gate reviews.

---

# PART 0: EXECUTIVE BLUEPRINT

## 0.1 Executive Summary

DDIS (Decision-Driven Implementation Specification) is a standard for writing technical specifications that bridge the gap between architectural vision and correct implementation.

Most specifications fail in one of two ways: they are too abstract (the implementer must guess at critical details) or too mechanical (they prescribe code without explaining why, making evolution impossible). DDIS avoids both failure modes by requiring a **causal chain** from first principles through decisions to implementation details, where every element justifies its existence by serving the elements around it.

DDIS 2.0 added a third failure axis: **LLM hallucination**. When a large language model implements from a spec, it fills unspecified gaps with plausible behavior from its training data. DDIS introduced negative specifications, verification prompts, meta-instructions, and structural redundancy to close these gaps.

DDIS 3.0 addresses a fourth axis: **specification testability and composability**. Real systems are built from multiple specs that must reference each other. And specifications themselves need automated validation — not just manual gate reviews. DDIS 3.0 adds machine-readable cross-references (INV-022, ADR-013), a composability protocol (§0.2.5), automated specification testing (§12.3), ADR confidence levels (ADR-012), and incremental authoring support (§11.3).

DDIS 4.0 addresses a fifth axis: **specification maturity and adoption**. Real adoption requires graduated conformance levels (§0.2.6), formal decision lifecycle management (ADR-015), verifiable worked examples (INV-023), and computed health metrics (§12.4). DDIS 4.0 also formalizes conditional section semantics (INV-024) and adds an ambiguity resolution protocol (§0.2.3, Principle L4) — telling LLMs what to do when specs are unclear rather than leaving them to guess.

DDIS 5.0 addresses a sixth axis: **implementation traceability and multi-pass LLM workflows**. Real implementations reveal a gap between specification and code — invariants exist in the spec but there is no structured mapping to the files, functions, and tests that enforce them. DDIS 5.0 adds spec-to-implementation mapping (INV-025, §5.9), a multi-pass LLM consumption workflow (§0.2.7), an LLM validation protocol for spec review (§12.5), verification coverage completeness (INV-026), and strengthened self-bootstrapping with negative specifications for the meta-standard's own element spec chapters.

DDIS synthesizes techniques from several traditions — Architecture Decision Records (Nygard), Design by Contract (Meyer), temporal invariants (Lamport), performance budgeting (game engine development), test-driven specification, and LLM-optimized document engineering — into a unified document structure. The synthesis is the contribution: these techniques are well-known individually but rarely composed into a single coherent standard.

### 0.1.1 What DDIS Is

DDIS is a document standard. It specifies:

- What structural elements a specification must contain
- How those elements must relate to each other (the cross-reference web)
- What quality criteria each element must meet
- How to validate that a specification is complete — both manually and automatically
- How to structure elements so that LLM implementers produce correct output on the first pass
- How multiple DDIS specs compose when systems depend on each other
- How to adopt the standard incrementally via graduated conformance levels

DDIS is domain-agnostic. It can describe a terminal rendering kernel, an agent coordination system, a database engine, a compiler, or any system where correctness matters and multiple people (or LLMs) will implement from the spec.

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
  ADRs reference invariants. Invariants reference tests. Tests reference performance budgets. Cross-references restate the substance of what they reference ([[INV-018|substance restated at point of use]]), not just the identifier.

- **The document is self-contained**
  A competent implementer with the spec and the spec alone — no oral tradition, no Slack threads, no "ask the architect" — can build a correct v1. If they cannot, the spec has failed.

- **LLM implementers succeed without hallucinating**
  An LLM reading any implementation chapter (with the glossary and restated invariants) can produce a correct implementation without inventing requirements not in the spec. Negative specifications (§3.8) explicitly close the gap between what is specified and what an LLM might plausibly add. (Justified by §0.2.3; validated by Gate 7.)

- **The specification is automatically testable**
  Cross-reference integrity, proportional weight, invariant completeness, and reference staleness can be validated by tooling, not just human review. (Justified by §0.2.5; validated by Gate 8.)

- **Worked examples are verifiable**
  Every worked example can be checked against the spec's own invariants. An incorrect example is worse than no example — LLMs reproduce examples faithfully, including their errors. (Justified by §0.2.3; validated by [[INV-023|example correctness]]).

- **Spec elements trace to implementation artifacts**
  Every invariant, ADR, and algorithm in the spec maps to at least one implementation artifact (file, function, test) via the implementation mapping (§5.9). A spec with no path from requirements to code is a wish list, not a blueprint. (Justified by §0.2.7; validated by Gate 10.)

### 0.1.3 Non-Goals (Explicit)

DDIS does not attempt:

- **To replace code.** A spec describes what to build, why, and how to verify it — not the literal source code. Design sketches illustrate intent; they are not copy-paste targets.
- **To eliminate judgment.** Implementers will make thousands of micro-decisions. DDIS constrains the macro-decisions so micro-decisions are locally safe.
- **To be a project management framework.** DDIS includes a Master TODO and phased roadmap, but these are execution aids, not a substitute for sprint planning.
- **To prescribe notation.** DDIS requires formal models but does not mandate TLA+, Alloy, Z, or any specific formalism. Any precise formalism is acceptable.
- **To guarantee correctness.** A DDIS-conforming spec dramatically reduces the chance of building the wrong thing. It cannot eliminate it.
- **To prevent all LLM hallucination.** Negative specifications and verification prompts reduce hallucination for the most plausible misinterpretations. They cannot prevent all possible LLM errors — only the structurally predictable ones.
- **To prescribe tooling.** Machine-readable cross-reference syntax (§10.3) enables tooling but DDIS does not mandate any specific validation tool.

## 0.2 First-Principles Derivation

### 0.2.1 What IS an Implementation Specification?

A specification is a function from intent to artifact:

```
Spec: (Problem, Constraints, Knowledge) → Document
where:
  Document enables: Implementer × Document → Correct_System
```

The quality of a specification is measured by one criterion: **does an implementer produce a correct system from it, without requiring information not in the document?**

This definition has consequences:

1. **Completeness over elegance.** A verbose spec that leaves nothing ambiguous is better than a terse spec that leaves critical details to inference. (But see [[INV-007|every section earns its place]]: verbosity without structure is noise.)

2. **Decisions over descriptions.** The hardest part of building a system is not writing code — it is making the hundreds of design decisions that determine whether the code is correct. A spec that describes a system without recording why it is shaped that way is a snapshot, not a blueprint.

3. **Verifiability over trust.** Every claim in the spec must be testable. "The system is fast" is not verifiable. "Event ingestion completes in < 100us p99 at the design point of 300 agents / 10K tasks, measured by Benchmark B-001" is verifiable.

4. **Exclusion over implication.** For LLM implementers, what a spec does NOT say is as dangerous as what it says incorrectly. An LLM will fill unspecified gaps with plausible behavior. Explicit exclusions (negative specifications) are a safety mechanism, not optional polish. (Justified by §0.2.3.)

5. **Automation over ceremony.** Validation that can be automated should be automated. Manual gate reviews catch semantic issues; automated testing catches structural issues. Both are required. (New in 3.0; justified by §0.2.5.)

### 0.2.2 The Causal Chain (Why DDIS Is Structured This Way)

DDIS prescribes a specific document structure because specifications fail in predictable ways, and each structural element prevents a specific failure mode:

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
| LLM adds features not in the spec | Hallucinated requirements | Negative specifications (§3.8, [[INV-017|negative spec per chapter]]) |
| LLM loses context mid-chapter | Implements against stale invariants | Structural redundancy at point of use ([[INV-018|restate substance]]) |
| LLM implements in wrong order | Integration failures | Meta-instructions with ordering (§5.7, [[INV-020|explicit sequencing]]) |
| LLM doesn't self-check | Subtle invariant violations ship | Verification prompts per chapter (§5.6, [[INV-019|self-check]]) |
| Cross-refs rot silently | Stale restatements mislead | Machine-readable refs + automated testing (§10.3, [[INV-022|parseable refs]]) |
| Dependent specs contradict | Integration failures across systems | Composability protocol (§0.2.5) |
| Spec elements have no path to code | Requirements exist on paper but nobody knows which file enforces them | Implementation mapping (§5.9, [[INV-025|spec-to-code traceability]]) |
| LLM implements in single pass without verification | Subtle invariant violations ship | Multi-pass workflow guidance (§0.2.7, [[INV-026|verification coverage]]) |

```
First Principles (formal model of the problem)
  ↓ justifies
Non-Negotiables + Invariants (what must always be true)
  ↓ constrained by
Architecture Decision Records (choices that could go either way)
  ↓ implemented via
Algorithms + Data Structures + Protocols (pseudocode, state machines)
  ↓ bounded by
Negative Specifications (what must NOT be built)
  ↓ verified by
Test Strategies + Performance Budgets + Verification Prompts
  ↓ validated by
Automated Spec Tests + Quality Gates (structural + semantic checks)
  ↓ shipped via
Master TODO (execution checklist)
  ↓ traced by
Implementation Mapping (spec elements → code artifacts)
```

Every element in DDIS exists because removing it causes a specific, named failure. There are no decorative sections.

### 0.2.3 LLM Consumption Model

DDIS formally recognizes that the primary implementer is often a large language model. This section provides the theoretical foundation for all LLM-specific provisions in the standard.

**The Hallucination Gap.** Given a spec S describing system Σ, define the *specified space* as the set of behaviors S explicitly prescribes or excludes. The *implementation space* is the set of all behaviors an implementer might produce. The *hallucination gap* is:

```
H(S) = implementation_space(S) - specified_space(S)
```

For human implementers, H(S) is filled by judgment, experience, and questions to the architect. For LLMs, H(S) is filled by training data — plausible but unvalidated behavior. DDIS aims to minimize H(S) through negative specifications that explicitly close the most dangerous regions.

**Five Principles of LLM-Optimized Specification:**

**L1: Minimize the hallucination gap.** Every implementation chapter includes negative specifications (§3.8) that explicitly exclude the most plausible misinterpretations. The spec author thinks adversarially: "What would an LLM add that I didn't ask for?" (Validated by [[INV-017|negative spec per chapter]].)

**L2: Context-window self-sufficiency.** Each implementation chapter is self-contained for LLM consumption. Cross-references restate the substance of the referenced constraint, not just its ID. An LLM processing a chapter in isolation — without access to earlier chapters — can still produce correct output. (Validated by [[INV-018|substance restated at point of use]].)

**L3: Active verification over passive specification.** Each implementation chapter ends with a verification prompt (§5.6) the LLM can use to self-check its output. Passive specification says "the system must do X." Active verification says "check: does your implementation do X? Does it avoid Y?" (Validated by [[INV-019|verification prompt per chapter]].)

**L4: Ambiguity resolution over silent guessing.** When a spec is ambiguous or incomplete, the implementer's response must be predictable. A human asks the architect. An LLM, by default, picks the most plausible interpretation from training data — silently. DDIS specs should include an explicit ambiguity protocol: either (a) flag the ambiguity in a comment and proceed with the most conservative interpretation, or (b) follow the spec's designated fallback behavior (e.g., 'when unspecified, default to the strictest invariant interpretation'). (No invariant — this is guidance, not a structural requirement.)

**L5: Multi-pass consumption over single-pass implementation.** LLMs produce better implementations when they process a spec in structured passes: (1) comprehend the formal model and invariants, (2) implement subsystem by subsystem following meta-instructions, (3) verify each subsystem against its verification prompt, (4) cross-check the implementation mapping. A single monolithic 'implement everything' prompt produces worse results than structured multi-pass workflows. (Validated by §0.2.7.)

**LLM-Specific Failure Modes and Mitigations:**

| LLM Failure Mode | Mitigation | DDIS Element |
|---|---|---|
| Fills unspecified gaps with training data | Negative specifications | §3.8, [[INV-017|negative spec per chapter]] |
| Loses context in long documents | Structural redundancy at point of use | [[INV-018|substance restated]], ADR-009 |
| Over-indexes on examples | High quality bar for worked examples | §5.2 |
| Cannot follow "see above" | Explicit section refs with substance | [[INV-018|substance restated]], Chapter 10 |
| Implements in arbitrary order | Meta-instructions with ordering | §5.7, [[INV-020|explicit sequencing]] |
| Does not self-check | Verification prompts | §5.6, [[INV-019|self-check per chapter]] |
| Produces hedged, vague prose | Voice guidance counteracts | §8.1 |
| Interprets ambiguous terms randomly | Glossary with disambiguation | [[INV-009|glossary coverage]], §7.1 |
| Encounters ambiguous spec text | Picks random interpretation silently | Ambiguity protocol (§0.2.3, L4) |
| Implements everything in one pass | Misses cross-subsystem invariants | Multi-pass workflow | §0.2.7, L5 |
| No traceability from spec to code | Requirements drift from implementation | Implementation mapping | §5.9, [[INV-025|spec-to-code traceability]] |

### 0.2.4 Fundamental Operations of a Specification

Every specification, regardless of domain, performs these operations:

| Operation | What It Does | DDIS Element |
|---|---|---|
| **Define** | Establish what the system IS, formally | First-principles model, formal types |
| **Constrain** | State what must always hold | Invariants, non-negotiables |
| **Decide** | Lock choices where alternatives exist | ADRs (with confidence level) |
| **Describe** | Specify how components work | Algorithms, state machines, protocols |
| **Exemplify** | Show the system in action | Worked examples, end-to-end traces |
| **Bound** | Set measurable limits | Performance budgets, design point |
| **Verify** | Define how to confirm correctness | Test strategies, quality gates, verification prompts |
| **Exclude** | State what the system is NOT | Non-goals, negative specifications |
| **Sequence** | Order the work | Phased roadmap, meta-instructions |
| **Lexicon** | Define terminology | Glossary |
| **Compose** | Reference external specs | Composability protocol (§0.2.5) |
| **Trace** | Map spec elements to implementation artifacts | Implementation mapping |

### 0.2.5 Composability Model

When System B depends on System A, and both have DDIS specs, those specs must reference each other without creating hidden dependencies or contradictions.

**The Composability Problem.** A DDIS spec is self-contained ([[INV-008|spec answers all implementation questions]]). But when System B's spec says "uses System A's event API," System B's spec now depends on System A's spec. If System A's spec changes, System B's spec may silently break.

**Composability Principles:**

**C1: External specs are referenced by stable contract, not internal section.** System B references System A's published invariants and API surface, never its internal implementation chapters.

```
BAD:  "See System A spec, §7.3 for the event schema"
GOOD: "Consumes events conforming to System A's APP-INV-017
       (append-only event log, schema v2.1)"
```

**C2: Cross-spec invariants are declared explicitly.** When System B depends on System A's invariant, System B's spec declares this as an *external dependency* with the invariant substance restated:

```
External Dependencies:
- System A, APP-INV-017: Event log is append-only (events, once written,
  are never modified or deleted). Schema version: 2.1.
  Impact if violated: System B's replay mechanism fails.
```

**C3: Version pinning.** Cross-spec references pin to a specific spec version. "System A spec v2.3, APP-INV-017" — not just "System A's event invariant."

**C4: Composition validation.** When both specs are available, check: (a) every external dependency references an invariant that exists in the target spec at the pinned version, (b) no external dependency contradicts the target spec's negative specifications.

**C5: ADR lifecycle cascades across specs.** When System A supersedes an ADR that System B references, System B's external dependency declaration becomes stale. The composability protocol requires: (a) System A's changelog notes superseded ADRs at the element level, (b) System B's external dependency declarations include the ADR lifecycle state, (c) version pinning (C3) limits the blast radius — System B is affected only when it unpins to a new version.

### 0.2.6 Specification Conformance Levels

Not every specification needs every DDIS element on day one. DDIS defines three conformance levels to support incremental adoption. (Locked by [[ADR-014|conformance levels for graduated adoption]].)

**Essential** — The minimum viable DDIS spec. Required elements:
- Design goal + core promise
- First-principles formal model (§0.2, even if brief)
- ≥ 5 numbered invariants with violation scenarios ([[INV-003|falsifiable]])
- ≥ 3 ADRs with genuine alternatives ([[INV-002|decision completeness]])
- Cross-references with substance ([[INV-018|substance restated]])
- Glossary covering domain terms ([[INV-009|glossary coverage]])
- Quality gates 1–4

**Standard** — Full DDIS treatment for production systems. Essential plus:
- Negative specifications per implementation chapter ([[INV-017|negative spec coverage]])
- Verification prompts per chapter ([[INV-019|self-check per chapter]])
- Meta-instructions where ordering matters ([[INV-020|explicit sequencing]])
- Machine-readable cross-references ([[INV-022|parseable refs]])
- End-to-end trace (§5.3)
- Gates 1–7
- Automated spec testing (§12.3)

**Complete** — For large, multi-team, or safety-critical systems. Standard plus:
- Composability protocol for cross-spec references (§0.2.5)
- Modularization protocol if size warrants (§0.13)
- Gate 8 (Specification Testability)
- Spec health metrics (§12.4)
- All [Optional] sections populated

A spec MUST declare its conformance level in the preamble. A spec at level N must satisfy ALL requirements for that level. Higher levels are strictly additive — a Complete spec satisfies Standard and Essential.

// WHY NOT a single level? Adoption friction is the #1 barrier to spec quality. A team that writes an Essential spec is better off than a team that attempts Complete and abandons the effort. Graduated adoption matches how teams actually work.

### 0.2.7 Multi-Pass LLM Consumption Workflow

The LLM Consumption Model (§0.2.3) defines principles for spec structure. This section defines the recommended workflow for an LLM consuming a DDIS spec — the operational counterpart to the structural principles.

**The Single-Pass Anti-Pattern.** Giving an LLM an entire spec with the instruction "implement this" produces mediocre results. The LLM attempts to hold the full spec in context while generating code, losing early invariants as it progresses through implementation chapters. Multi-pass consumption addresses this by structuring the work into focused phases.

**Recommended Multi-Pass Workflow:**

**Pass 1: Comprehension (read-only)**
- Read PART 0 end-to-end: formal model, invariants, ADRs, quality gates
- Read the glossary (Appendix A) for term disambiguation
- Output: a summary of the system's formal model, key invariants, and locked decisions
- Validation: the summary should reference specific INV-NNN and ADR-NNN identifiers

**Pass 2: Implementation (per-chapter)**
- For each implementation chapter (following meta-instruction ordering):
  - Read the chapter (with restated invariants and negative specs)
  - Implement the subsystem
  - Run the chapter's verification prompt as a self-check
  - Record the implementation mapping: which spec elements map to which code artifacts
- Validation: each chapter's verification prompt passes

**Pass 3: Integration verification**
- Re-read the end-to-end trace (§5.3)
- Verify the implementation handles the traced scenario correctly
- Check cross-subsystem invariants that span multiple chapters
- Validation: the end-to-end trace scenario produces correct output

**Pass 4: Mapping audit**
- Review the implementation mapping (§5.9) for completeness
- Every invariant has at least one enforcing artifact
- Every negative specification has at least one test or assertion
- Validation: no unmapped invariants remain

**Structural support for multi-pass consumption:**
- PART 0 is designed as a comprehension-pass input (formal model, constraints, decisions)
- Each PART II chapter is designed as an implementation-pass input (self-contained with restated invariants)
- The end-to-end trace is designed as an integration-verification input
- The implementation mapping is designed as a mapping-audit input

**For modular specs:** In modular mode (§0.13), each bundle is designed for Passes 1-2 within a single context window. Pass 3 requires either the end-to-end trace module or sequential processing of adjacent modules. Pass 4 aggregates mappings across all modules.

// WHY THIS MATTERS: LLMs that follow structured multi-pass workflows produce implementations with fewer invariant violations than single-pass approaches. The workflow aligns with how the spec is structured — each pass consumes a different structural layer.
