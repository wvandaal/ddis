## 0.7 Quality Gates

A DDIS-conforming specification is "done" when all quality gates pass. Gates are ordered by priority; a failing Gate 1 makes Gates 2–10 irrelevant.

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

**Test procedure for Gate 8:** Run a cross-reference parser against the spec. Verify: (1) All `[[TARGET|substance]]` references resolve to existing elements. (2) No section is an orphan in the reference graph. (3) Proportional weight deviations are within tolerance or annotated. (4) Every ADR has a Confidence field and lifecycle state. If any automated check fails, Gate 8 fails.

**Gate 9: Conformance Level Compliance**
The specification satisfies ALL requirements for its declared conformance level (§0.2.6). An Essential spec passes Gates 1–4. A Standard spec passes Gates 1–7 plus automated testing. A Complete spec passes Gates 1–10. (Validates declared conformance level; locked by [[ADR-014|graduated conformance]].)

**Test procedure for Gate 9:** Identify the spec's declared conformance level. Check the element checklist for that level (§0.2.6). Verify every required element is present and satisfies the level's quality criteria. A spec that passes Gate 7 but lacks machine-readable cross-references cannot claim Standard conformance.

**Gate 10: Implementation Traceability**
The spec's implementation mapping (§5.9) covers >= 90% of invariants and algorithms. Every mapped artifact references at least one spec element. No invariant is orphaned (unmapped). (Validates [[INV-025|spec-to-code traceability]].)

**Test procedure for Gate 10:** Parse the implementation mapping tables from all implementation chapters. Count the total invariants and algorithms defined in the spec. Count those with at least one mapping entry. Compute coverage = mapped / total. If coverage < 0.9, Gate 10 fails. Also verify: every mapping entry references a valid spec element ID; no mapping entry references a nonexistent invariant or algorithm.

### Modularization Quality Gates [Conditional -- modular specs only]

In addition to Gates 1–10, modular specs must pass these gates.

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

DDIS 5.0 is "done" when:
- This document passes Gates 1–10 applied to itself
- At least one non-trivial specification has been written conforming to DDIS and the author reports structural sufficiency
- The Glossary (Appendix A) covers all DDIS-specific terminology
- The LLM provisions are woven throughout (ADR-011), not isolated
- Cross-references use machine-readable syntax ([[INV-022|parseable cross-refs]])
- Conformance levels are defined and self-consistent ([[ADR-014|graduated conformance]])
- ADR lifecycle states are formalized ([[ADR-015|formal ADR lifecycle]])
- Implementation mapping demonstrated in §5.9 worked example ([[INV-025|spec-to-code traceability]])
- Multi-pass LLM workflow documented (§0.2.7)
- LLM validation protocol operational (§12.5)
- Verification coverage completeness demonstrated ([[INV-026|every invariant in a verification prompt]])

## 0.8 Performance Budgets (for Specifications, Not Software)

### 0.8.1 Specification Size Budgets

| System Complexity | Target Spec Length | Rationale |
|---|---|---|
| Small (single crate, < 5K LOC target) | 500–1,500 lines | Formal model + invariants + key ADRs |
| Medium (multi-crate, 5K–50K LOC target) | 1,500–5,000 lines | Full DDIS treatment |
| Large (multi-service, > 50K LOC target) | 5,000–15,000 lines | May split via §0.13 modularization |

**LLM context window guidance:** If a spec exceeds the target LLM's context window minus a 25% reasoning reserve, modularization (§0.13) is required, not optional. Individual implementation chapters should not exceed 500 lines — if a chapter is longer, consider splitting the subsystem.

**Context budget calculator** (new in 4.0): For a target LLM with context window C tokens:
```
effective_spec_budget = C * 0.75          (25% reasoning reserve)
glossary_overhead     = term_count * 15   (avg tokens per glossary entry)
available_for_content = effective_spec_budget - glossary_overhead
```
If `available_for_content` < estimated spec size in tokens, modularization (§0.13) is required. For multi-pass workflows (understand -> implement -> verify), reduce the effective budget by an additional 20% to allow the LLM to hold both the spec and its own generated output.

**Multi-pass budget allocation** (new in 5.0): When using the multi-pass workflow (§0.2.7), allocate the context budget per pass:
```
Pass 1 (comprehension): PART 0 + glossary               ~ 20-25% of spec
Pass 2 (implementation): single chapter + restated invariants ~ 15-25% of spec per chapter
Pass 3 (integration): end-to-end trace + key invariants  ~ 10-15% of spec
Pass 4 (mapping audit): implementation mapping tables     ~ 5-10% of spec
```
If any single pass exceeds the effective context budget, that pass's input must be split or the spec modularized.

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

## 0.10 Open Questions (for DDIS 6.0)

1. ~~**Machine-readable cross-references**~~ — **Resolved in 3.0** by [[ADR-013|wiki-link syntax]] and [[INV-022|parseable cross-refs]].

2. **Multi-document specs**: For very large systems, how should sub-specs reference each other beyond the composability protocol (§0.2.5)? Should there be a "meta-manifest" for multi-spec systems?

3. **Formal verification bridge**: Should DDIS define a pathway from falsifiable invariants to machine-checked properties for safety-critical systems?

4. ~~**Automated Gate 7 testing**~~ — **Partially resolved in 4.0** by spec health metrics (§12.4) which provide quantitative LLM-readiness signals. Full automation remains open — a CI pipeline that feeds spec chapters to an LLM and validates output is feasible but model-dependent.

5. **Spec diffing**: When a spec evolves, how should changes be tracked at the element level (not just text diffs)? Should DDIS define a semantic diff format for invariants, ADRs, and cross-references?

6. ~~**Spec-to-implementation traceability**~~ — **Resolved in 5.0** by [[INV-025|spec-to-code traceability]], [[ADR-016|structured implementation mapping]], and §5.9.

7. **Multi-modal specification**: Should DDIS formalize requirements for non-textual spec elements (sequence diagrams, state machine visualizations, architecture diagrams) beyond the current "ASCII preferred" guidance?

8. **Spec-as-test-suite**: Can DDIS invariants be compiled into executable test harnesses? A formal bridge from semi-formal invariant expressions to property-based test generators would make INV-003 (falsifiability) machine-enforceable.

9. **LLM model-specific optimization**: Different LLM architectures (transformer variants, mixture-of-experts) may benefit from different spec structures. Should DDIS define model-family-specific guidance, or is the current model-agnostic approach sufficient?

10. **Collaborative spec authoring**: When multiple authors (human or LLM) write different chapters of the same spec concurrently, how should conflicts in shared elements (invariants, ADRs, glossary) be resolved? The composability protocol (§0.2.5) addresses cross-spec references but not intra-spec collaboration.

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
  FOR ALL event IN EventLog, FOR ALL t1 < t2:
    event IN EventLog(t1) -> event IN EventLog(t2) AND event(t1) = event(t2)
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
             |-- < 20 total AND system constitution <= 400 lines
             |    -> TWO-TIER
             +-- >= 20 total OR system constitution > 400 lines
                  -> THREE-TIER
```

#### 0.13.7.1 Two-Tier Simplification

For small modular specs, the domain tier can be skipped. In two-tier mode:
- **Tier 1**: Contains BOTH declarations AND full definitions (fits in <= 400 lines).
- **Tier 2 and Tier 3**: SKIPPED.

Assembly: `system_constitution + module -> bundle`.

### 0.13.8 File Layout

```
project-spec/
+-- manifest.yaml                    # Module manifest (§0.13.9)
+-- constitution/
|   +-- system.md                    # Tier 1: declarations, glossary, gates
|   +-- domain_storage.md            # Tier 2: Storage domain definitions
|   +-- domain_coordination.md       # Tier 2: Coordination domain definitions
|   +-- domain_presentation.md       # Tier 2: Presentation domain definitions
+-- deep/
|   +-- scheduler_cross.md           # Tier 3: cross-domain context for Scheduler
|   +-- tui_cross.md                 # Tier 3: cross-domain context for TUI
+-- modules/
|   +-- event_store.md               # Module: EventStore subsystem
|   +-- scheduler.md                 # Module: Scheduler subsystem
|   +-- tui_renderer.md              # Module: TUI Renderer subsystem
+-- bundles/                         # Generated (gitignored)
    +-- event_store_bundle.md        # Assembled: Tier1 + Storage + module
    +-- scheduler_bundle.md          # Assembled: Tier1 + Coordination + deep + module
    +-- tui_renderer_bundle.md       # Assembled: Tier1 + Presentation + deep + module
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
5. Validate: total lines <= `hard_ceiling_lines`

**Two-tier assembly**: Step 1 (Tier 1 with full definitions) + Step 4 (Module). Skip steps 2-3.

**Budget validation**: If assembled bundle exceeds `target_lines`, emit a warning. If it exceeds `hard_ceiling_lines`, fail the assembly and require the module to be split.

### 0.13.11 Consistency Checks

Nine mechanical checks, each with a formal expression:

| Check | What It Validates | Formal Expression |
|---|---|---|
| CHECK-1 | Ownership uniqueness | `FOR ALL inv: count(modules maintaining inv) <= 1` |
| CHECK-2 | Ownership coverage | `FOR ALL inv IN registry: EXISTS module IN manifest: inv IN module.maintains` |
| CHECK-3 | Interface symmetry | `FOR ALL M: FOR ALL inv IN M.interfaces: EXISTS N != M: inv IN N.maintains` |
| CHECK-4 | Domain consistency | `FOR ALL M: M.domain IN manifest.domains` |
| CHECK-5 | Bundle budget | `FOR ALL M: line_count(ASSEMBLE(M)) <= hard_ceiling` |
| CHECK-6 | Declaration existence | `FOR ALL inv IN any module: inv IN system_constitution` |
| CHECK-7 | Cross-module isolation | `FOR ALL M: M.refs INTERSECT other_module_internal_sections = EMPTY` |
| CHECK-8 | Deep context sufficiency | `FOR ALL M: FOR ALL inv IN M.interfaces: inv definition IN ASSEMBLE(M)` |
| CHECK-9 | Manifest-filesystem sync | `FOR ALL path IN manifest: file_exists(path) AND FOR ALL file IN modules/: file IN manifest` |

### 0.13.12 Cascade Protocol

When a constitutional element changes, affected modules must be re-validated:

1. **Identify affected modules**: For a changed invariant INV, find all modules where INV IN `maintains` UNION `interfaces`.
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

---

## 0.15 Conditional Section Protocol

Sections in the document structure (§0.3) marked [Optional] or [Conditional] follow these rules. (Validated by [[INV-024|conditional section coherence]].)

**[Optional]** — The section adds value but its absence does not make the spec non-conforming. Decision criterion: include if the system has the relevant concern. Examples: View-model contracts are optional for systems with no UI; benchmark scenarios are optional for non-performance-critical systems.

**[Conditional]** — The section is REQUIRED when a specific predicate is true, and OMITTED otherwise.

| Section | Predicate (include when true) |
|---|---|
| §0.13 Modularization Protocol | Spec > 4,000 lines OR target context window cannot hold full spec |
| §0.14 External Dependencies | System depends on another DDIS-specified system |
| INV-011–016 (modular invariants) | Spec uses modularization protocol |
| Gates M-1–M-5 | Spec uses modularization protocol |

**Validation**: For each [Conditional] section omitted, the spec should include a one-line annotation: `// [Conditional] omitted: [predicate] is false.` This prevents ambiguity about whether the section was considered and deliberately excluded vs. accidentally missed.

---
