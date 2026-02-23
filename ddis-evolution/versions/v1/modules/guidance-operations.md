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

**DO NOT** let the voice shift between sections — inconsistency produces inconsistent LLM implementations. **DO NOT** hedge in invariants, ADRs, or negative specs — hedging causes LLMs to treat requirements as optional. (Validates INV-017.)

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
❌ "It might be worth considering the possibility of potentially using a
single-threaded loop, which could arguably provide some benefits in terms
of determinism, although this would need to be validated."
✅ "The kernel loop is single-threaded. This gives us deterministic replay.
See ADR-003 for the throughput analysis that confirms this is sufficient."
```

**Anti-pattern: The Orphan Section**
A section that references nothing and is referenced by nothing. It may contain good content, but if it's disconnected from the web, it's carrying dead weight. Either connect it or remove it.

**Anti-pattern: The Trivial Invariant**
"INV-042: The system uses UTF-8 encoding." This is either enforced by the language/platform (not worth an invariant) or so fundamental it belongs in Non-Negotiables, not the invariant list.

**Anti-pattern: The Strawman ADR**
```
❌ Options:
  A) Our chosen approach (clearly the best)
  B) A terrible approach nobody would choose
  Decision: A, obviously.
```
Every option in an ADR must have a genuine advocate — a competent engineer who, in a different context, would choose it.

**Anti-pattern: The Missing Verification Prompt**
An implementation chapter with negative specifications and invariant references but no verification prompt block. Without it, the LLM has no structured self-check before moving to the next subsystem. (§5.6 in element-specifications module, INV-020.)

**Anti-pattern: The Percentage-Free Performance Budget**
"The system should respond quickly." Without a number, a design point, and a measurement method, this is a wish, not a budget.

**Anti-pattern: The Spec That Requires Oral Tradition**
If an implementer must ask the architect a question the spec should have answered, the spec has a gap. Track questions during implementation and patch them in (Living state, §1.1 in core-standard module).

**Anti-pattern: The Afterthought LLM Section**
A "Chapter N: LLM Considerations" appendix bolted onto an otherwise LLM-unaware spec. Provisions must be woven throughout, not isolated. (ADR-008.)

**DO NOT** treat anti-patterns as a substitute for subsystem-specific negative specifications (§3.8, element-specifications module). Anti-patterns are document-level guidance; negative specs are subsystem-level constraints. Both required. (Validates INV-017, ADR-009.)

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

**DO NOT** apply domain-spec proportions (§0.8.2) to a meta-standard and flag a violation — meta-standards have fundamentally different weight distributions because PART 0 IS the substance, not a summary. (Validates INV-007, INV-017.)

**DO NOT** treat proportional weight as absolute — the ±20% adjustment range is explicit in §0.8.2. Specs with unusual domain distributions (e.g., heavy formal model, light operations) may deviate if the author documents the reasoning. (Validates INV-007, INV-017.)

**DO NOT** diagnose weight imbalance without checking the spec type first — a meta-standard with 50% PART 0 is healthy; a domain-spec with 50% PART 0 is top-heavy. The diagnostic signals in §9.2 apply to domain specs. (Validates INV-017.)

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

**DO NOT** create bidirectional references for trivially related sections — a reference from §A to §B does not require a reciprocal reference from §B to §A unless §B genuinely depends on §A for comprehension. Gratuitous back-references inflate noise without adding navigability. (Validates INV-007, INV-017.)

**DO NOT** rely on section ordering as a substitute for explicit cross-references — an LLM receiving a modular bundle may not see sections in document order. Even in monolithic specs, readers navigate by reference, not by proximity. (Validates INV-006, INV-017.)

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

**DO NOT** write in document order — use authoring order below; writing implementation before ADRs causes cascading rework. **DO NOT** skip negative specs (step 7) or verification prompts (step 11) — these cannot be retrofitted without re-reading each chapter. (Validates INV-017, INV-019.)

**DO NOT** treat the authoring sequence as inflexible for experienced authors — the dependency chain between steps is real, but experienced authors may batch steps within the same dependency tier (e.g., steps 1–3 can be drafted in one pass). Reordering across tiers (e.g., writing implementation before invariants) causes cascading rework regardless of experience. (Validates INV-019, INV-017.)

1. **GOAL: Design goal + Core promise** — no dependencies; start here
2. **FORMAL: First-principles formal model** — depends on GOAL: the formal model derives from the design goal
3. **NON-NEG: Non-negotiables** — depends on GOAL, FORMAL: commitments derive from goal and model
4. **INV: Invariants** — depends on FORMAL, NON-NEG: invariants formalize the model's properties and non-negotiable commitments
5. **ADR: ADRs** — depends on INV: ADRs reference invariants they protect; writing invariants first reveals which decisions matter
6. **IMPL: Implementation chapters** — depends on INV, ADR: implementation must respect locked invariants and ADR decisions; heaviest subsystems first
7. **NEG-SPEC: Negative specifications per chapter** — depends on IMPL: requires reading each chapter's implementation to identify what an LLM might get wrong
8. **TRACE: End-to-end trace** — depends on IMPL: requires all subsystems to be drafted so the trace can traverse them
9. **PERF: Performance budgets** — depends on IMPL: requires implementation to be specified before anchoring budgets to specific operations
10. **TEST: Test strategies** — depends on INV, IMPL, NEG-SPEC: tests validate invariants against implementation and negative specs
11. **VERIFY: Verification prompts per chapter** — depends on INV, NEG-SPEC, TEST: derived from invariants, negative specs, and test strategies
12. **XREF: Cross-references** — depends on steps 1–11: weaves the web across all existing content
13. **GLOSS: Glossary** — depends on steps 1–12: extract terms from the complete spec; writing it earlier means missing terms
14. **TODO: Master TODO** — depends on IMPL, PERF, PLAYBOOK: converts implementation chapters and budgets into an execution plan
15. **PLAYBOOK: Operational playbook** — depends on ADR, IMPL: requires ADRs and implementation to plan phases and delivery order
16. **META-INST: Meta-instructions** — depends on IMPL, TODO, PLAYBOOK: implementation ordering requires knowing the implementation, execution plan, and delivery order

### 11.2 Common Mistakes in First DDIS Specs

1. **Writing implementation chapters before ADRs.** You'll rewrite them when you discover the ADRs imply different choices.

2. **Writing the glossary first.** You don't know your terminology until you've written the spec. Write it last.

3. **Treating the end-to-end trace as optional.** It's the single most effective quality check. Write it.

4. **Under-investing in WHY NOT annotations.** Every non-obvious choice needs one. The first maintainer will thank you.

5. **Skipping the anti-patterns.** Show what bad output looks like. LLMs especially benefit from negative examples.

6. **Omitting negative specifications.** The most common mistake in LLM-targeted specs. If you don't tell the LLM what NOT to do, it will invent plausible but unauthorized behavior. (See §3.8 in element-specifications module, INV-017.)

7. **Referencing invariants by ID only.** INV-017 means nothing 2,000 lines from its definition. Restate it. (See INV-018.)

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

**DO NOT** declare validation complete after only mechanical checks — Gates 1–5 are mechanical, but Gate 6 (Implementation Readiness) and Gate 7 (LLM Implementation Readiness) require human or LLM judgment. Skipping judgment-based gates produces specs that are structurally conforming but practically unusable. (Validates INV-003, INV-017.)

Before declaring a spec complete, the author should:

1. Pick 5 random implementation sections. Trace each backward to the formal model. Did any chain break? (Gate 2)
2. Read each ADR's "alternatives" section. Would a competent engineer genuinely choose any rejected option? If not, the ADR is a strawman. (Gate 3)
3. For each invariant, spend 60 seconds trying to construct a violation scenario. If you can't, the invariant is either trivially true or too vague. (Gate 4)
4. Build the cross-reference graph (mentally or on paper). Are there orphan sections? (Gate 5)
5. Read the spec as if you were an implementer seeing it for the first time. Where did you have to guess? (Gate 6)
6. For 2+ implementation chapters, imagine giving ONLY that chapter (plus glossary and invariants) to an LLM. Would the LLM have enough information? Would it hallucinate anything? Would it know what NOT to do? (Gate 7)
7. Check proportional weight against §9.2 signals. Is any section starved (many invariants, few lines) or bloated (few invariants, many lines)? Is PART 0 heavier than PART II?

### 12.2 External Validation

Give the spec to an implementer (or LLM) and track:
- Questions the spec should have answered → gaps
- Incorrect implementations not prevented → ambiguities
- Skipped sections → voice/clarity issues
- Added behaviors not in spec → missing negative specifications

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

- **Gaps** are patched into the spec, not oral tradition. The spec remains the single source of architectural truth.
- **ADRs may be superseded.** Mark old ADR as "Superseded by ADR-NNN," update all cross-references. Do not delete — reasoning is historical record.
- **New invariants may be added.** Implementation reveals non-obvious properties. Add with full INV-NNN format.
- **Performance budgets may be revised.** If unachievable, the budget or design must change. Document which and why.
- **Negative specifications may be added.** LLM implementation reveals unanticipated hallucination patterns.

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
| **ADR** | Architecture Decision Record. A structured record of a design choice, including alternatives considered and rationale. (See §3.5, element-specifications module) |
| **ADR supersession** | Replacing an ADR while preserving the original as historical record. Requires cross-reference cascade. (See ADR-011, §13.3) |
| **Assembly script** | Tool that reads the manifest and produces bundles by concatenating tiers with module content. (See §0.13.10, modularization module) |
| **Bundle** | Assembled document for LLM implementation of one module: Tier 1 + Tier 2 + Tier 3 + Module. The unit of LLM consumption. (See §0.13.2, §0.13.10, modularization module) |
| **Cascade protocol** | The procedure for identifying and re-validating modules affected by a change to constitutional content. (See §0.13.12, modularization module) |
| **Causal chain** | The traceable path from a first principle through an invariant and/or ADR to an implementation detail. (See §0.2.3, INV-001) |
| **Blast radius** | The set of modules and invariants affected by a change to constitutional content. Determines the scope of re-validation in the cascade protocol. (See §0.13.12, modularization module) |
| **Churn-magnet** | A decision that, if left open, causes the most downstream rework. ADRs should prioritize locking churn-magnets. (See §3.5, element-specifications module) |
| **Comparison block** | A side-by-side ❌/✅ comparison of a rejected and chosen approach with quantified reasoning. (See §5.5, element-specifications module) |
| **Confidence level** | An optional ADR field indicating decision maturity: Committed (high confidence, default), Provisional (revisit after spike), or Speculative (needs abstraction boundary). (See §3.5, element-specifications module) |
| **Constitution** | Cross-cutting material constraining all modules. Organized in tiers: system (Tier 1), domain (Tier 2), cross-domain deep (Tier 3). (See §0.13.3, modularization module) |
| **Context budget** | The portion of an LLM's context window available for a spec fragment, after reserving space for reasoning. Equals context_window × (1 − reasoning_reserve). (See §0.13.9 in modularization module, §0.2.2) |
| **Cross-cutting module** | A module whose domain is set to "cross-cutting" because it spans multiple architectural domains (e.g., end-to-end trace, cross-domain integration tests). (See §0.13.6, modularization module) |
| **Cross-reference** | An explicit link between two sections of the spec, using §X.Y, INV-NNN, or ADR-NNN identifiers. Forms part of the reference web. (See Chapter 10, INV-006) |
| **DDIS** | Decision-Driven Implementation Specification. This standard. |
| **Decision spike** | A time-boxed experiment that de-risks an unknown and produces an ADR. (See §6.1.1, element-specifications module) |
| **Design sketch** | A code block that illustrates intent and API shape without being compilable or copy-paste-ready. Distinguished from production code by the Document Note (§2.3, element-specifications module). Correctness lives in invariants and tests, not in sketch syntax. (See §2.3, element-specifications module) |
| **Declaration** | A compact (1-line) summary of an invariant or ADR in the system constitution (Tier 1). Contrasts with the full definition in the domain constitution (Tier 2). (See §0.13.4, modularization module) |
| **Deep context** | Tier 3 of the constitution: cross-domain invariant definitions, ADR specs, and interface contracts needed by a specific module. Zero overlap with Tier 2. (See §0.13.3, modularization module) |
| **Definition** | The full specification of an invariant or ADR in the domain constitution (Tier 2), including formal expression, violation scenario, and validation method. (See §0.13.4, modularization module) |
| **Design point** | The specific hardware, workload, and scale scenario against which performance budgets are validated. (See §3.7, element-specifications module) |
| **Domain** | An architectural grouping of related modules sharing tighter coupling with each other than with modules in other domains. Corresponds to rings, layers, or crate groups. (See §0.13.2, modularization module) |
| **Domain constitution** | Tier 2 of the constitution: full invariant definitions and ADR analysis for one architectural domain. (See §0.13.3, modularization module) |
| **End-to-end trace** | A worked scenario that traverses all major subsystems, showing data at each boundary. In modular specs, stored as a special cross-cutting module. (See §5.3 in element-specifications module, §0.13.6 in modularization module) |
| **Exit criterion** | A specific, testable condition that must hold for a phase to be considered complete. (See §6.1.2, element-specifications module) |
| **Falsifiable** | A property of an invariant: it can be violated by a concrete scenario and detected by a concrete test. (See INV-003, ADR-002) |
| **First principles** | The formal model of the problem domain from which the architecture derives. (See §3.3, element-specifications module) |
| **Formal model** | A mathematical or pseudo-mathematical definition of the system as a state machine or function. (See §0.2.1) |
| **Gate** | A quality gate: a stop-ship predicate that must be true before the project can proceed. (See §3.6, element-specifications module) |
| **Hallucination** | An LLM failure mode where the model generates plausible but unauthorized behaviors not specified in the document. Prevented by negative specifications (§3.8, element-specifications module). (See §0.2.2) |
| **Invariant** | A numbered, falsifiable property that must hold at all times during system operation. (See §3.4, element-specifications module) |
| **Invariant registry** | The section of the manifest listing every invariant with its owning module, ensuring INV-013 (Invariant Ownership Uniqueness). (See §0.13.9, modularization module) |
| **Living spec** | A specification in active use, being updated as implementation reveals gaps. (See §13.1) |
| **LLM** | Large Language Model. In DDIS context: the primary implementer consuming a spec to produce a correct implementation, operating under the constraints modeled in §0.2.2 (fixed context window, no random access, hallucination tendency). (See §0.2.2) |
| **LLM consumption model** | The formal model of how an LLM consumes a DDIS spec, including failure modes and structural mitigations. (See §0.2.2) |
| **Manifest** | Machine-readable YAML declaring all modules, domain membership, invariant ownership, and assembly rules. Single source of truth for assembly. (See §0.13.9, modularization module) |
| **Master TODO** | A checkboxable task inventory cross-referenced to subsystems, phases, and ADRs. (See §7.3, element-specifications module) |
| **Meta-instruction** | A directive to the LLM implementer embedded in the spec, providing ordering, sequencing, or process guidance. (See §5.7, element-specifications module) |
| **Monolith** | A DDIS spec that exists as a single document, as opposed to a modular spec. All specs start as monoliths. (See §0.13.2, modularization module) |
| **Negative specification** | Explicit "DO NOT" constraint co-located with the implementation chapter. Primary defense against LLM hallucination. (See §3.8 in element-specifications module, INV-017) |
| **Non-goal** | Something the system explicitly does not attempt. (See §3.2, element-specifications module) |
| **Non-negotiable** | A philosophical commitment stronger than an invariant — defines what the system IS. (See §3.1, element-specifications module) |
| **Operational playbook** | A chapter covering how the spec gets converted into shipped software. (See §6.1, element-specifications module) |
| **Partial regression** | A spec lifecycle transition where the spec returns from a later state (Threaded, Gated, Validated, Living) to Drafted for re-validation of affected sections only — not the entire document. Triggered when a gap is discovered or content changes. (See §1.1 State × Event Table, core-standard module) |
| **Proportional weight** | Line budget guidance preventing bloat in some sections and starvation in others. (See §0.8.2) |
| **Quality criteria** | The specific, testable properties an element must exhibit to be considered well-formed within DDIS. Each element spec section (Chapters 2–7) defines quality criteria for its element type. (See Chapters 2–7, element-specifications module) |
| **Reasoning reserve** | The fraction of an LLM's context window reserved for reasoning (not spec content). Default 0.25 (25%). Declared in the manifest. (See §0.13.9, modularization module) |
| **Ring architecture** | DDIS's own structural organization: Core Standard (sacred, mandatory), Guidance (recommended, may be adapted), and Tooling (optional, convenience). Not to be confused with OS protection rings. (See §0.4) |
| **Self-bootstrapping** | A property of this standard: it is written in the format it defines. (See ADR-004) |
| **Module** | Self-contained spec unit covering one major subsystem. Corresponds to one PART II chapter. Always assembled into a bundle. (See §0.13.2, §0.13.5, modularization module) |
| **Signal-to-noise ratio** | The proportion of a section's content that directly contributes to implementer understanding versus administrative overhead or repetition. Governed by INV-007. (See INV-007, §0.8.2) |
| **Module header** | Structured YAML block at module start declaring domain, maintained invariants, interfaces, and negative specifications. (See §0.13.5, modularization module) |
| **Structural redundancy** | The practice of restating key invariants at their point of use (not just at the point of definition) to prevent context loss in long documents. Required by INV-018. (See §0.2.2) |
| **System constitution** | Tier 1 of the constitution: compact declarations of all invariants and ADRs, plus system-wide orientation (design goal, non-negotiables, glossary summaries). Always included in every bundle. (See §0.13.3, modularization module) |
| **Three-tier mode** | The standard modularization configuration: system constitution (Tier 1) + domain constitution (Tier 2) + cross-domain deep context (Tier 3) + module. (See §0.13.7 in modularization module, ADR-006) |
| **Two-tier mode** | A simplified modularization configuration for small specs (< 20 invariants): system constitution (full definitions) + module. No domain or deep context tiers. (See §0.13.7.1, modularization module) |
| **Verification prompt** | A structured self-check prompt at the end of an implementation chapter, used by implementers (especially LLMs) to verify their output against the spec. (See §5.6 in element-specifications module, ADR-010) |
| **Voice** | The writing style prescribed by DDIS: technically precise but human. (See §8.1) |
| **Verification prompt coverage** | Property (INV-020) that every element spec chapter includes a verification prompt block demonstrating §5.6 (element-specifications module) by self-application. (See INV-020) |
| **WHY NOT annotation** | An inline comment explaining why a non-obvious alternative was rejected. (See §5.4, element-specifications module) |
| **Worked example** | A concrete scenario with specific values showing a subsystem in action. (See §5.2, element-specifications module) |

---

## Appendix B: Risk Register

| # | Risk | Impact | Mitigation | Detection |
|---|---|---|---|---|
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
|---|---|---|---|---|
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
          meta-instructions → cross-refs]  (13 components per §5.1, element-specifications)
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
- [ ] Section Map accuracy validated against current module structure after edits (INV-016)
- [ ] Cross-module references audited: all go through constitutional elements or Section Map (INV-012)
- [ ] Declaration-definition faithfulness verified: each Tier 1 declaration matches its Tier 2 definition (Gate M-4)
- [ ] Context budget values in constitution match manifest.yaml (INV-016)

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
