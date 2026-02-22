## 0.13 Modularization Protocol [Conditional]

This section is REQUIRED when the monolithic specification exceeds 4,000 lines or when the target context window (model-dependent) cannot hold the full spec plus a meaningful working budget for LLM reasoning. It is OPTIONAL but recommended for specs between 2,500–4,000 lines.

> Namespace note: INV-001 through INV-020 and ADR-001 through ADR-010 are DDIS meta-standard invariants/ADRs (defined in this standard). Application specs using DDIS define their OWN invariant namespace (e.g., APP-INV-001) — never reuse the meta-standard's INV-NNN space.

### 0.13.1 The Scaling Problem

A DDIS spec's value depends on the implementer holding sufficient context to produce correct output without guessing. When the spec exceeds the implementer's context window, two failure modes emerge:

1. **Truncation**: The LLM silently drops content from the beginning of the context, losing invariants and the formal model — the very elements that prevent hallucination.

2. **Naive splitting**: Arbitrary file splits break cross-references, orphan invariants from the sections they constrain, and force the LLM to guess at contracts defined in unseen sections.

The modularization protocol prevents both failures by defining a principled decomposition with formal completeness guarantees. (Motivated by INV-008: Self-Containment, §0.2.3: LLM Consumption Model.)

### 0.13.2 Core Concepts

**Monolith**: A DDIS spec that exists as a single document. All specs start as monoliths. Most small-to-medium specs remain monoliths.

**Module**: A self-contained unit of the spec covering one major subsystem. Each module corresponds to one chapter of PART II in the monolithic structure. A module is never read alone — it is always assembled into a bundle with the appropriate constitutional context.

**Constitution**: The cross-cutting material that constrains all modules. Contains the formal model, invariants, ADRs, quality gates, architecture overview, glossary, and performance budgets. Organized in tiers to manage its own size.

**Domain**: An architectural grouping of related modules that share tighter coupling with each other than with modules in other domains.

**Bundle**: The assembled document sent to an LLM for implementation. Always contains: system constitution + domain constitution + cross-domain deep context + the module itself. A bundle is the unit of LLM consumption.

**Manifest**: A machine-readable YAML file that declares all modules, their domain membership, invariant ownership, cross-module interfaces, and assembly rules. The manifest is the single source of truth for the assembly script.

(All terms defined in Glossary, Appendix A.)

### 0.13.3 The Tiered Constitution

The constitution is organized in three tiers to prevent it from becoming a bottleneck itself. Each tier has a hard line budget, a clear scope, and NO overlapping content between tiers. (Locked by ADR-006.)

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
|  SCOPE: System-wide orientation. Knows WHAT exists, not HOW. |
+--------------------------------------------------------------+
| TIER 2: Domain Constitution (200-500 lines, per-domain)      |
|  - Domain formal model (subset of full system model)         |
|  - FULL DEFINITIONS for invariants owned by this domain      |
|  - FULL ANALYSIS for ADRs decided within this domain         |
|  - Cross-domain interface contracts (this domain's surface)  |
|  - Domain-level performance budgets                          |
|  SCOPE: Everything needed to work in this domain.            |
|  NOTE: Content here is NOT duplicated in Tier 3.             |
+--------------------------------------------------------------+
| TIER 3: Cross-Domain Deep Context (0-600 lines, per-module)   |
|  - Full definitions for OTHER-domain invariants this module   |
|    INTERFACES with (not in this module's Tier 2)              |
|  - Full ADR specs from OTHER domains that affect this module  |
|  - Interface contracts with adjacent modules in OTHER domains |
|  - Shared types defined in OTHER domains used by this module  |
|  SCOPE: Cross-domain context ONLY. Zero overlap with Tier 2. |
|  NOTE: If module has no cross-domain interfaces, Tier 3 is    |
|  EMPTY. This is common and correct.                          |
+--------------------------------------------------------------+
| MODULE (800-3,000 lines)                                      |
|  - Module header (ownership, interfaces, negative specs)      |
|  - Full PART II content for this subsystem                   |
|  - Negative specifications for this subsystem                |
|  - Verification prompt for this subsystem                    |
|  SCOPE: What to build for this subsystem.                    |
+--------------------------------------------------------------+

Assembled bundle: Tier 1 + Tier 2 + Tier 3 + Module
Target budget:    1,200 - 4,500 lines per bundle
Hard ceiling:     5,000 lines (must fit in context with reasoning room)
```

### 0.13.4 Invariant Declarations vs. Definitions

The critical mechanism that makes the tiered constitution work. An invariant has two representations:

**Declaration** (Tier 1, always present, ~1 line):
```
APP-INV-017: Event log is append-only -- Owner: EventStore -- Domain: Storage
```

**Definition** (Tier 2, in the owning domain's constitution, ~10-20 lines):
```
**APP-INV-017: Event Log Append-Only**

*Events, once written, are never modified or deleted.*

  ∀ event ∈ EventLog, ∀ t1 < t2:
    event ∈ EventLog(t1) → event ∈ EventLog(t2) ∧ event(t1) = event(t2)

Violation scenario: A compaction routine rewrites old events to save space,
silently changing event payloads. Replay produces different state.

Validation: Write 1000 events, snapshot the log, run any operation, compare
log prefix byte-for-byte.

// WHY THIS MATTERS: Append-only is the foundation of deterministic replay.
// Without it, APP-INV-003 (replay determinism) is impossible.
```

**Inclusion rules:**

| Module's relationship to invariant     | Tier 1      | Tier 2 (own domain)              | Tier 3 (cross-domain)  |
|---------------------------------------|-------------|----------------------------------|------------------------|
| Module MAINTAINS this invariant        | Declaration | Full definition (already present) | — (same domain rule)  |
| INTERFACES, invariant in SAME domain  | Declaration | Full definition (already present) | —                     |
| INTERFACES, invariant in OTHER domain | Declaration | —                               | Full definition        |
| No relationship                       | Declaration | —                               | —                     |

### 0.13.5 Module Header (Required per Module)

Every module begins with a structured header that makes the module self-describing:

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
# - Must NOT implement its own serialization (use shared codec from APP-ADR-011)
```

### 0.13.6 Cross-Module Reference Rules

**Rule 1: Cross-module references go through the constitution, never direct.** (Enforced by INV-012, locked by ADR-007.)

```
BAD:  "See section 7.3 in the Scheduler chapter for the dispatch algorithm"
GOOD: "This subsystem publishes SchedulerReady events (see APP-INV-032,
       maintained by the Scheduler module)"
```

**Rule 2: Shared types are defined in the constitution, not in any module.**

**Rule 3: The end-to-end trace is a special module.**

The end-to-end trace (§5.3) is the one element that legitimately crosses all module boundaries. It is stored as its own module file with a special header:

```yaml
# Module Header: End-to-End Trace
# Domain: cross-cutting
# Maintains: (none — this module validates, it doesn't implement)
# Interfaces: ALL application invariants
# Purpose: Integration validation, not implementation
# Assembly: Tier 1 + ALL domain constitutions (no Tier 3 needed)
```

### 0.13.7 Modularization Decision Flowchart

```
Is spec > 4,000 lines?
  |-- No  -> Is spec > 2,500 lines AND target context < 8K lines?
  |           |-- No  -> MONOLITH (no modularization needed, stop here)
  |           +-- Yes -> MODULE (recommended)
  +-- Yes -> MODULE (required)
             |
             How many invariants + ADRs total?
             |-- < 20 total AND system constitution fits in <= 400 lines
             |    -> TWO-TIER (see §0.13.7.1)
             +-- >= 20 total OR system constitution > 400 lines
                  -> THREE-TIER (standard protocol)
```

#### 0.13.7.1 Two-Tier Simplification

For small modular specs, the domain tier can be skipped. In two-tier mode:

- **Tier 1 (System Constitution)**: Contains BOTH declarations AND full definitions.
- **Tier 2 (Domain Constitution)**: SKIPPED.
- **Tier 3 (Cross-Domain Deep)**: SKIPPED.
- **Module**: Unchanged.

Assembly in two-tier mode: `system_constitution + module → bundle`.

### 0.13.8 File Layout

```
spec-project/
|-- manifest.yaml
|-- constitution/
|   |-- system.md
|   +-- domains/                      # absent in two-tier
|       |-- storage.md
|       |-- coordination.md
|       +-- presentation.md
|-- deep/                             # Tier 3: only if cross-domain
|   |-- scheduler.md
|   +-- integration_tests.md
|-- modules/
|   |-- event_store.md
|   |-- snapshot_manager.md
|   |-- scheduler.md
|   |-- tui_renderer.md
|   +-- end_to_end_trace.md
|-- bundles/                          # Generated (gitignored)
+-- .beads/
```

### 0.13.9 Manifest Schema

```yaml
# manifest.yaml — Single source of truth for DDIS module assembly
ddis_version: "2.0"
spec_name: "Example System"
tier_mode: "three-tier"               # "two-tier" or "three-tier"

context_budget:
  target_lines: 4000
  hard_ceiling_lines: 5000
  reasoning_reserve: 0.25

constitution:
  system: "constitution/system.md"
  domains:
    storage:
      file: "constitution/domains/storage.md"
      description: "Event store, snapshots, persistence layer"
    coordination:
      file: "constitution/domains/coordination.md"
      description: "Scheduling, reservations, task DAG"

modules:
  event_store:
    file: "modules/event_store.md"
    domain: storage
    maintains: [APP-INV-003, APP-INV-017, APP-INV-018]
    interfaces: [APP-INV-001, APP-INV-005]
    implements: [APP-ADR-003, APP-ADR-011]
    adjacent: [snapshot_manager, scheduler]
    deep_context: null
    negative_specs:
      - "Must NOT directly access TUI rendering state"
      - "Must NOT bypass reservation system for writes"

  end_to_end_trace:
    file: "modules/end_to_end_trace.md"
    domain: cross-cutting
    maintains: []
    interfaces: all
    implements: []
    adjacent: all
    deep_context: null
    negative_specs: []

invariant_registry:
  APP-INV-001: { owner: system, domain: system, description: "Causal traceability" }
  APP-INV-003: { owner: event_store, domain: storage, description: "Replay determinism" }
  APP-INV-017: { owner: event_store, domain: storage, description: "Append-only log" }
  # ... (abbreviated — real manifests list all invariants)
```

### 0.13.10 Assembly Rules

The assembly script reads the manifest and produces one bundle per module.

**Three-tier assembly:**
```
ASSEMBLE(module_name):
  module = manifest.modules[module_name]
  bundle = []

  # Tier 1: Always included
  bundle.append(read(manifest.constitution.system))

  # Tier 2: Domain constitution
  if module.domain == "cross-cutting":
    for domain in manifest.constitution.domains:
      bundle.append(read(domain.file))
  else:
    bundle.append(read(manifest.constitution.domains[module.domain].file))

  # Tier 3: Cross-domain deep context (only if file exists)
  if module.deep_context is not null:
    bundle.append(read(module.deep_context))

  # The module itself
  bundle.append(read(module.file))

  # Budget validation (INV-014)
  total_lines = sum(line_count(section) for section in bundle)
  if total_lines > manifest.context_budget.hard_ceiling_lines:
    ERROR("Bundle {module_name}: {total_lines} lines exceeds ceiling. INV-014 VIOLATED.")
  elif total_lines > manifest.context_budget.target_lines:
    WARN("Bundle {module_name}: {total_lines} lines exceeds target.")

  write(bundles/{module_name}_bundle.md, join(bundle))
```

**Two-tier assembly:**
```
ASSEMBLE(module_name):
  bundle = [read(manifest.constitution.system), read(module.file)]
  validate_budget(bundle, module_name)
  write(bundles/{module_name}_bundle.md, join(bundle))
```

### 0.13.11 Consistency Validation

Nine mechanical checks. All implementable by a validation script.

**CHECK-1: Invariant ownership completeness** — Every invariant has exactly one owner. (Validates INV-013.)
**CHECK-2: Interface consistency** — Every interfaced invariant exists and is maintained. (Validates INV-012.)
**CHECK-3: Adjacency symmetry** — If A lists B as adjacent, B lists A. (Structural consistency.)
**CHECK-4: Domain membership consistency** — Module's maintained invariants are in its domain. (Validates INV-013.)
**CHECK-5: Budget compliance** — Every bundle fits within hard ceiling. (Validates INV-014.)
**CHECK-6: No orphan invariants** — Every invariant is maintained or interfaced by some module. (Validates INV-006.)
**CHECK-7: Cross-module reference isolation** — No direct module-to-module references. (Validates INV-012.)
**CHECK-8: Deep context correctness** — Cross-domain interfaces have deep context files. (Validates INV-011.)
**CHECK-9: File existence** — All manifest paths exist; all module files are in manifest. (Validates INV-016.)

### 0.13.12 Cascade Protocol

When constitutional content changes, affected modules must be re-validated.

| Change | Blast Radius |
|----|---|
| Invariant wording changed | Modules maintaining or interfacing |
| ADR superseded | Modules implementing that ADR |
| New invariant added | Module assigned as owner |
| Shared type changed | Same-domain + cross-domain users |
| Non-negotiable changed | ALL modules |
| Glossary term redefined | All modules using that term |

### 0.13.13 Quality Gate Extensions

Modular specs must pass Gates M-1 through M-5 (§0.7) in addition to Gates 1–7.

### 0.13.14 Monolith-to-Module Migration Procedure

1. **Identify domains.** Group PART II chapters into 2–5 domains based on architectural boundaries.
2. **Extract system constitution.** Preamble, PART 0 sections, all invariant DECLARATIONS, all ADR DECLARATIONS, glossary, quality gates.
3. **Extract domain constitutions.** Full invariant definitions, full ADR analysis, cross-domain interface contracts per domain.
4. **Extract modules.** Add module header (§0.13.5), include implementation content, convert direct references to constitutional references (INV-012).
5. **Create cross-domain deep context files.** For modules interfacing with other-domain invariants.
6. **Build manifest.** All module entries, invariant registry, context budget.
7. **Validate.** Run `ddis_validate.sh` — all nine checks must pass.
8. **Extract end-to-end trace.** Create as cross-cutting module. Verify budget.
9. **LLM validation.** Give 2+ bundles to an LLM. Zero questions requiring other module's implementation.

---

## 0.14 Specification Composition Protocol [Conditional]

This section is REQUIRED when the system being specified depends on or is depended upon by another DDIS-specified system. It is OPTIONAL for standalone systems.

### 0.14.1 The Composition Problem

When System A has a DDIS spec and System B has a DDIS spec, and B depends on A, three questions arise:

1. **How do B's invariants reference A's contracts?** B needs to state assumptions about A without reaching into A's implementation details.
2. **How do ADRs in A affect decisions in B?** A's choice of event sourcing constrains B's integration approach.
3. **How are cross-spec integration properties validated?** Neither A's spec nor B's spec alone describes the integration surface.

### 0.14.2 Cross-Spec Reference Rules

**Rule 1: Reference contracts, not internals.** Spec B may reference Spec A's published API surface (§0.9 equivalent in A), A's invariants that constitute public contracts, and A's ADRs that constrain B's design space. Spec B must NOT reference A's internal implementation chapters, internal state machines, or private types.

```
ALLOWED:
  "This subsystem assumes event delivery as guaranteed by (EXT:system-a:INV-017)."
  "Integration approach constrained by (EXT:system-a:ADR-003)."

NOT ALLOWED:
  "Use the same batching strategy as System A's EventStore flush_batch() function."
  "Parse events using the struct layout in System A §7.3.2."
```

**Rule 2: Namespace isolation.** Each spec maintains its own invariant namespace. Cross-spec references use the extended reference form `(EXT:spec-name:INV-NNN)` defined in INV-021 to distinguish from local invariants. No spec may define invariants in another spec's namespace.

**Rule 3: Integration specs.** Cross-system integration properties that cannot be assigned to either spec are documented in a dedicated **integration spec**. The integration spec follows DDIS structure (§0.3) but its PART II chapters describe integration scenarios rather than subsystems. Each integration chapter includes:
- The specs involved and their relevant invariants
- Integration invariants (§0.14.3) that depend on both specs' guarantees
- Negative specifications for the integration surface
- A cross-spec end-to-end trace

### 0.14.3 Integration Invariants

Cross-spec invariants follow the same template as local invariants (§3.4) with one addition: the **spec boundary** field identifies which specs' guarantees the invariant depends on.

```
**INT-001: Event Delivery Guarantee**

*Events published by System A are consumed by System B within the delivery SLA.*

Spec boundary: Depends on (EXT:system-a:INV-017) (append-only log)
               and (EXT:system-b:INV-003) (idempotent processing).

  ∀ event ∈ A.published_events:
    ∃ t: B.consumed(event, t) ∧ t - event.publish_time ≤ delivery_SLA

Violation scenario: System A publishes events correctly (INV-017 holds), but
System B's consumer crashes on malformed event headers that A's spec never
prohibited. The integration invariant fails despite both specs passing
individually.

Validation: Integration test — publish 1000 events from A, verify B consumes
all within SLA, including edge-case event formats.

// WHY THIS MATTERS: Individual spec conformance does not guarantee system
// composition correctness. Integration invariants capture cross-boundary
// properties that neither spec owns alone.
```

### 0.14.4 When to Use Composition vs. Modularization

| Scenario | Approach |
|---|---|
| One system, spec too large for context window | Modularization (§0.13) |
| Multiple systems, each with own spec, shared integration surface | Composition (§0.14) |
| Systems share invariants but have independent development lifecycles | Composition (§0.14) |
| System's subsystems are tightly coupled with shared state | Modularization (§0.13) |
| System A is a library consumed by System B | Composition — A's API surface is the contract |
| Monorepo with multiple services | Composition + per-service modularization as needed |

### 0.14.5 Composition Validation

Cross-spec composition is validated by:

1. **Reference integrity**: Every `(EXT:spec-name:INV-NNN)` reference resolves to an existing invariant in the named spec. Broken references indicate spec version mismatch.
2. **Contract stability**: External invariants referenced by Spec B should be marked as public contracts in Spec A's API surface (§0.9). Changes to public contract invariants trigger cascade notification to dependent specs.
3. **Integration testing**: Each integration invariant (§0.14.3) has a cross-spec test that exercises both systems together.

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

Transitions (with guards):
  Skeleton  →[fill_sections]→     Drafted
    Guard: every required section (§0.3) has non-empty content
    Entry action: none
    Exit action: author self-review of completeness

  Drafted   →[add_cross_refs]→    Threaded
    Guard: every section has ≥ 1 outbound reference (INV-006)
    Entry action: build reference graph
    Exit action: verify graph connectivity

  Threaded  →[run_gates]→         Gated
    Guard: Gates 1–5 and Gate 8 pass; all invariants have violation scenarios (INV-003)
    Entry action: run mechanical checks (Gate 1, Gate 5)
    Exit action: record gate passage timestamps

  Gated     →[external_validate]→ Validated
    Guard: Gates 6–8 pass (human and LLM implementation readiness, automated consistency)
    Entry action: give spec to implementer/LLM
    Exit action: document validation results and any discovered gaps

  Validated →[begin_impl]→        Living
    Guard: at least one implementer has confirmed readiness
    Entry action: mark spec as authoritative
    Exit action: none

  Living    →[discover_gap]→      Drafted
    Guard: gap is documented; regression scoped to affected sections only
    Entry action: log gap in risk register
    Exit action: re-enter threading phase for affected sections
```

**State × Event table** (validates INV-010):

| State \ Event | fill_sections | add_cross_refs | run_gates | external_validate | begin_impl | discover_gap |
|---|---|---|---|---|---|---|
| **Skeleton** | → Drafted | INVALID: sections empty | INVALID: no content | INVALID: no content | INVALID: no content | INVALID: no content |
| **Drafted** | no-op (already drafted) | → Threaded | INVALID: unthreaded | INVALID: unthreaded | INVALID: unvalidated | → Drafted (patch) |
| **Threaded** | → Drafted (regression) | no-op | → Gated | INVALID: gates not run | INVALID: unvalidated | → Drafted (regression) |
| **Gated** | → Drafted (regression) | → Threaded (regression) | no-op (re-run) | → Validated | INVALID: unvalidated | → Drafted (regression) |
| **Validated** | → Drafted (regression) | → Threaded (regression) | → Gated (re-run) | no-op | → Living | → Drafted (regression) |
| **Living** | INVALID: use discover_gap | INVALID: use discover_gap | → Gated (re-validate) | → Validated (re-validate) | no-op | → Drafted (partial) |

Invalid transition policy: Reject and log. A transition that skips phases indicates incomplete specification work.

### 1.2 Completeness Properties

A complete specification satisfies three properties:

**Safety**: The spec never prescribes contradictory behavior.
```
∀ section_a, section_b ∈ spec:
  ¬(section_a.prescribes(behavior_X) ∧ section_b.prescribes(¬behavior_X))
```

**Liveness**: The spec eventually answers every architectural question an implementer will ask.
```
∀ question Q where Q.is_architectural:
  ◇(spec.answers(Q))  // "eventually" means by Validated state
```

**Negative completeness**: The spec explicitly excludes the most plausible misinterpretations.
```
∀ subsystem S, ∀ misinterpretation M where M.is_plausible:
  spec.explicitly_excludes(M) ∨ spec.unambiguously_prevents(M)
```

**Reference completeness**: The spec's cross-references are all resolvable and form a connected graph.
```
∀ reference ∈ spec.references:
  ∃ target: reference.resolves_to(target) ∧ target ∈ spec
```

// WHY NEGATIVE COMPLETENESS: This property is new in DDIS 2.0 and directly motivated by the LLM Consumption Model (§0.2.3). LLMs fill gaps with plausible behavior. The spec must close the most dangerous gaps — those where the plausible behavior violates invariants. (Enforced by INV-017.)

### 1.3 Complexity of Specification Elements

| Element | Authoring Complexity | Reading Complexity | Verification Complexity |
|---|---|---|---|
| Invariant | O(domain_understanding) | O(1) per invariant | O(1) per invariant |
| ADR | O(alternatives × analysis_depth) | O(alternatives) per ADR | O(1) per ADR |
| Algorithm | O(algorithm_complexity × edge_cases) | O(pseudocode_length) | O(worked_examples) |
| Cross-reference | O(1) per reference | O(1) per reference | O(sections²) for full graph |
| End-to-end trace | O(subsystems × interactions) | O(subsystems) | O(1) (follow the trace) |
| Negative specification | O(domain_understanding) | O(1) per constraint | O(1) (check plausibility) |
| Verification prompt | O(invariants_per_chapter) | O(1) per chapter | O(1) (run the prompt) |

### 1.4 End-to-End Trace (of DDIS Itself)

This section demonstrates the end-to-end trace element (§5.3) by tracing a single ADR through the complete DDIS authoring process — from the author's initial recognition of a decision through to its final validated form.

**Scenario**: An author writing a task coordination spec realizes that the event log could be either append-only or mutable with compaction. This is a genuine design decision (two reasonable alternatives). Trace:

```
Step 1: Recognition (Skeleton → Drafted)
  Author encounters the choice while writing the EventStore implementation chapter.
  The chapter cannot proceed without resolving: "append-only vs. mutable?"
  Output: Decision noted as open question.

Step 2: ADR Creation (Drafted)
  Author creates APP-ADR-011 following the template (§3.5):
    Problem: "Should the event log allow compaction/mutation?"
    Options: A) Append-only (immutable) — Pros: deterministic replay, simple
             B) Mutable with compaction — Pros: space efficiency, faster reads
    Decision: Option A — deterministic replay is a non-negotiable (§0.1.2).
    WHY NOT B: Compaction makes replay non-deterministic; violates APP-INV-003.
    Consequences: Space usage grows linearly; need archival strategy.
    Tests: Validated by APP-INV-003 (replay determinism test).

  Cross-references created:
    APP-ADR-011 → APP-INV-003 (invariant it protects)
    APP-ADR-011 → APP-INV-017 (invariant it justifies — append-only)
    APP-ADR-011 ← EventStore chapter (references this ADR)
    APP-ADR-011 → formal model (event log definition in §0.2)

Step 3: Negative Specification (Drafted)
  The ADR's "WHY NOT" clause generates a negative specification for the
  EventStore chapter:
    "Do NOT implement log compaction, event rewriting, or mutation of
     persisted events. See APP-ADR-011."
  This prevents an LLM implementer from "helpfully" adding compaction.

Step 4: Threading (Drafted → Threaded)
  Cross-references verified: APP-ADR-011 has ≥ 1 inbound and ≥ 1 outbound
  reference. The EventStore chapter references APP-ADR-011. The invariant
  section lists APP-INV-017 with "locked by APP-ADR-011."
  Reference graph: no orphan sections (INV-006 satisfied).

Step 5: Gating (Threaded → Gated)
  Gate 3 (Decision Coverage): reviewer confirms no unstated alternatives.
  Gate 4 (Invariant Falsifiability): APP-INV-017 has violation scenario
    (compaction routine that rewrites events).
  Gate 7 (LLM Readiness): LLM given EventStore chapter + APP-INV-003 +
    APP-INV-017 + negative specs. LLM does NOT add compaction. Gate passes.

Step 6: Validation (Gated → Validated)
  External implementer reads EventStore chapter. No questions about
  mutability. The negative specification preempted the most likely question.

Step 7: Living
  Six months later, storage costs prompt reconsideration. Author creates
  APP-ADR-015 superseding APP-ADR-011, with a new approach: append-only
  with external archival. APP-ADR-011 marked "Superseded by APP-ADR-015."
  Cross-references updated. Cascade protocol (§0.13.12) identifies
  affected modules.
```

This trace exercises: ADR creation (§3.5), cross-referencing (Chapter 10, INV-006), negative specifications (§3.8, INV-017), quality gates (§0.7, Gates 3, 4, 7), spec evolution (§13.1), and the cascade protocol (§0.13.12).

---