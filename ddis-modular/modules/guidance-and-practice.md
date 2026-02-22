# Module: Guidance and Practice
<!-- domain: practice -->
<!-- maintains: (none) -->
<!-- interfaces_with: INV-001, INV-003, INV-006, INV-007, INV-008, INV-009 -->
<!-- adjacent: element-specifications, core-framework -->
<!-- budget: 450 lines -->

## Negative Specifications
- This module MUST NOT redefine structural requirements or element formats — those belong to element-specifications
- This module MUST NOT define invariants, ADRs, or quality gates — those belong to core-framework
- This module MUST NOT contain modularization procedures — those belong to modularization-protocol
- This module MUST NOT override mandatory DDIS requirements — it provides recommended guidance only

---

## PART III: Guidance (Recommended)

### Chapter 8: Voice and Style

#### 8.1 The DDIS Voice

**Technically precise but human.** The voice of a senior engineer explaining their system to a peer they respect.

**Properties**:
- Uses concrete examples, not abstract descriptions
- Admits uncertainty where it exists
- Is direct about tradeoffs
- Does not hedge every statement
- Uses humor sparingly and only when it clarifies
- Never uses marketing language ("enterprise-grade", "cutting-edge")
- Never uses bureaucratic language ("it is recommended that", "the system shall")

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

#### 8.2 Formatting Conventions

- **Bold** for terms being defined, non-negotiable properties, critical warnings
- `Code` for types, function names, file names
- `// Comments` for inline justifications and WHY NOT annotations
- Tables for structured data
- Blockquotes for preamble elements only
- ASCII diagrams preferred over external images

#### 8.3 Anti-Pattern Catalog

**The Hedge Cascade**:
```
❌ "It might be worth considering the possibility of potentially using..."
✅ "The kernel loop is single-threaded. This gives us deterministic replay. See ADR-003."
```

**The Orphan Section**: References nothing and is referenced by nothing. Either connect it or remove it. (Violates INV-006.)

**The Trivial Invariant**: "INV-042: The system uses UTF-8 encoding." Either enforced by platform (not worth an invariant) or belongs in Non-Negotiables.

**The Strawman ADR**: Every option must have a genuine advocate.

**The Percentage-Free Performance Budget**: "The system should respond quickly." Without a number, design point, and measurement method, this is a wish. (Violates INV-005.)

**The Spec That Requires Oral Tradition**: If an implementer must ask questions the spec should answer, patch the gap back. (Violates INV-008.)

### Chapter 9: Proportional Weight Deep Dive

#### 9.1 Identifying the Heart

Every system has 2–3 subsystems where most complexity and bugs live. These should receive 40–50% of PART II line budget.

**How to identify**: Which subsystems have the most invariants? Most ADRs? Most cross-references? If you cut the spec in half, which would you keep?

#### 9.2 Signals of Imbalanced Weight

- 5 invariants + 50 lines of spec = **starved**
- 1 invariant + 500 lines of spec = **bloated**
- PART 0 longer than PART II = top-heavy
- Appendices longer than PART II = reference displacing substance

### Chapter 10: Cross-Reference Patterns

#### 10.1 Reference Syntax

Recommended conventions (consistent within a spec):
```
(see §3.2)                    — section reference
(validated by INV-004)        — invariant reference
(locked by ADR-003)           — decision reference
(measured by Benchmark B-001) — performance reference
(defined in Glossary: "task") — glossary reference
```

#### 10.2 Reference Density Targets

| Section Type | Minimum Outbound References |
|---|---|
| Implementation chapter | 3 (ADR + invariant + other chapter) |
| ADR | 2 (invariant + implementation chapter) |
| Invariant | 1 (test or validation method) |
| Performance budget | 2 (benchmark + design point) |
| Test strategy | 2 (invariant + implementation chapter) |

## PART IV: Operations — Applying and Evolving DDIS

### Chapter 11: Applying DDIS to a New Project

#### 11.1 The Authoring Sequence

Write in this order (not document order) to minimize rework:

1. Design goal + Core promise
2. First-principles formal model
3. Non-negotiables
4. Invariants
5. ADRs
6. Implementation chapters — heaviest subsystems first
7. End-to-end trace
8. Performance budgets
9. Test strategies
10. Cross-references
11. Glossary (extract from complete spec)
12. Master TODO
13. Operational playbook

#### 11.2 Common Mistakes in First DDIS Specs

1. **Writing implementation chapters before ADRs.** You'll rewrite when ADRs imply different choices.
2. **Writing the glossary first.** You don't know your terminology until the spec is written.
3. **Treating the end-to-end trace as optional.** It's the most effective quality check.
4. **Under-investing in WHY NOT annotations.** Every non-obvious choice needs one.
5. **Skipping the anti-patterns.** LLMs especially benefit from negative examples.

### Chapter 12: Validating a DDIS Specification

#### 12.1 Self-Validation Checklist

1. Trace 5 random implementation sections backward to formal model. Any breaks?
2. For each ADR, would a competent engineer genuinely choose each rejected option?
3. For each invariant, spend 60 seconds constructing a violation scenario.
4. Build the cross-reference graph. Orphan sections?
5. Read as a first-time implementer. Where did you have to guess?

#### 12.2 External Validation

Give the spec to an implementer/LLM and track:
- Questions the spec should have answered (gaps)
- Incorrect implementations the spec didn't prevent (ambiguities)
- Sections skipped due to confusion (voice/clarity issues)

### Chapter 13: Evolving a DDIS Specification

#### 13.1 The Living Spec

Once implementation begins:
- **Gaps** are patched into the spec, not into oral tradition
- **Superseded ADRs** are marked "Superseded by ADR-NNN" (not deleted — historical record)
- **New invariants** may be added with full INV-NNN format
- **Performance budgets** may be revised with documented rationale

#### 13.2 Spec Versioning

`Major.Minor` where:
- **Major**: formal model or non-negotiable changes
- **Minor**: ADRs, invariants, or implementation chapters added/revised

## Quick-Reference Card

```
PREAMBLE: Design goal → Core promise → Document note → How to use
PART 0:   Summary → First principles → Architecture → Layout →
          Invariants → ADRs → Gates → Budgets → API → Non-negotiables → Non-goals
PART I:   Formal model → State machines → Complexity
PART II:  [Per subsystem: types → algorithm → state machine → invariants →
          example → WHY NOT → tests → budget → cross-refs]
          End-to-end trace (crosses all subsystems)
PART III: Protocol schemas → Adapters → UI contracts
PART IV:  Test taxonomy → Error taxonomy → Operational playbook
APPENDICES: Glossary → Risks → Formats → Benchmarks
PART X:   Master TODO (checkboxable, by subsystem)

Every invariant: ID + statement + formal + violation + test + why
Every ADR: problem + options (genuine) + decision + WHY NOT + consequences + tests
Every algorithm: pseudocode + complexity + example + edge cases
Cross-refs: web, not list. No orphan sections.
Voice: senior engineer to respected peer. No hedging. No marketing. No bureaucracy.
```
