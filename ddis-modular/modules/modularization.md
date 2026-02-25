---
module: modularization
domain: modularization
maintains: [INV-011, INV-012, INV-013, INV-014, INV-015, INV-016]
interfaces: [INV-001, INV-006, INV-008, INV-017, INV-018]
implements: [ADR-006, ADR-007]
adjacent: [core-standard]
negative_specs:
  - "Must NOT allow direct cross-module references bypassing the constitution"
  - "Must NOT allow bundles exceeding the hard ceiling"
  - "Must NOT allow invariants to be unowned or multiply-owned"
---

# Modularization Protocol Module

This module contains the DDIS modularization protocol (§0.13) for specs exceeding 2,500 lines or context window limits, its associated invariants, ADRs, and quality gates.

---

## Modularization Invariants

**INV-011: Module Completeness** [Conditional — modular specs only]

*An LLM receiving a properly assembled bundle can implement the module's subsystem without information from any other module's implementation content.*

```
∀ module ∈ modules:
  let bundle = ASSEMBLE(module)
  ∀ implementation_question Q about module's subsystem:
    bundle.answers(Q) ∨ Q.answerable_from(general_competence)
```

Violation scenario: The Scheduler module references EventStore's internal ring buffer layout, but ring buffer details live only in the EventStore module — not in the constitution.

Validation: Give a bundle (not the full spec) to an LLM. Track questions requiring information from another module's implementation. Any such question violates INV-011.

// WHY THIS MATTERS: If module completeness fails, modularization provides no benefit. The value proposition is that bundles are sufficient.

---

**INV-012: Cross-Module Isolation** [Conditional — modular specs only]

*Modules reference each other only through constitutional elements (invariants, ADRs, shared types). No module contains direct references to another module's internal sections, algorithms, or data structures.*

```
∀ module_a, module_b ∈ modules where module_a ≠ module_b:
  ∀ ref ∈ module_a.outbound_references:
    ref.target ∉ module_b.internal_sections ∧
    ref.target ∈ {constitution, shared_types, invariants, ADRs}
```

Violation scenario: The TUI Renderer module says "use the same batching strategy as the EventStore module's flush_batch() function."

Validation: Mechanical (CHECK-7 in §0.13.11). Semantic: review for implicit references that bypass the constitution.

// WHY THIS MATTERS: If modules reference each other's internals, bundles need other modules' implementation — defeating modularization. The constitution is the "header file"; modules are "implementation files" never directly included. (Locked by ADR-007.)

// META-STANDARD CARVE-OUT: For meta-standards using two-tier mode with a declaration-only constitution, the Section Map (§0.3) serves as the constitutional routing table for cross-module section references. The pattern "§X.Y, [module-name] module" is permissible because (a) the Section Map is constitutional content mediating the reference, (b) the module name suffix makes the cross-module nature explicit, and (c) the referenced sections are the meta-standard's public API surface. Application specs should still prefer invariant/ADR references over direct section references.

---

**INV-013: Invariant Ownership Uniqueness** [Conditional — modular specs only]

*Every application invariant is maintained by exactly one module (or explicitly by the system constitution). No invariant is unowned or multiply-owned.*

```
∀ inv ∈ invariant_registry:
  (inv.owner = "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 0)
  ∨ (inv.owner ≠ "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 1)
```

Violation scenario: Both EventStore and SnapshotManager list an invariant in their maintains declarations. Which module's tests are authoritative?

Validation: Mechanical (CHECK-1 in §0.13.11).

// WHY THIS MATTERS: If two modules both claim to maintain an invariant, neither takes full responsibility for its test coverage.

---

**INV-014: Bundle Budget Compliance** [Conditional — modular specs only]

*Every assembled bundle fits within the hard ceiling defined in the manifest's context budget.*

```
∀ module ∈ modules:
  line_count(ASSEMBLE(module)) ≤ context_budget.hard_ceiling_lines
```

Violation scenario: Scheduler module grows to 3,500 lines. With 1,200-line constitutional context, the bundle is 4,700 lines — under the 5,000 hard ceiling but over the 4,000 target (WARN). If the bundle reaches 5,100 lines, INV-014 is violated (ERROR, assembly fails).

Validation: Mechanical (CHECK-5 in §0.13.11). Run the assembly script; it validates budget compliance automatically.

// WHY THIS MATTERS: Budget violations mean modularization added complexity without delivering its benefit.

---

**INV-015: Declaration-Definition Consistency** [Conditional — modular specs only]

*Every invariant declaration in the system constitution is a faithful summary of its full definition in the domain constitution.*

```
∀ inv ∈ invariant_registry:
  let decl = system_constitution.declaration(inv)
  let defn = full_definition(inv)
  decl.id = defn.id ∧
  decl.one_line is_faithful_summary_of defn.statement
```

Violation scenario: System constitution declares an invariant as "Event log is append-only" but the Storage domain definition now says "append-only except during compaction." An LLM implementing a different domain codes against the wrong contract.

Validation: Semi-mechanical. Extract declaration/definition pairs; present to reviewer for semantic consistency.

// WHY THIS MATTERS: Divergence between tiers means different modules implement against different understandings of the same invariant. The declaration is the API; the definition is the implementation — they must agree.

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

// WHY THIS MATTERS: A file not in the manifest is invisible to all tooling — assembly, validation, improvement loops, cascade analysis.

---

## Modularization ADRs

### ADR-006: Tiered Constitution over Flat Root [Conditional — modular specs only]

#### Problem

When a DDIS spec is modular for context-window compliance (§0.13), constitutional context must accompany every module bundle. How should this constitutional context be structured?

#### Options

A) **Flat root** — one file containing everything.
- Pros: Simple; one file to maintain; no tier logic.
- Cons: Doesn't scale past ~20 invariants / ~10 ADRs. At scale (25 invariants, 15 ADRs, 4,800 lines), the root alone is ~1,500 lines, leaving only 2,500 for the module.

B) **Two-tier** — system constitution (full definitions) + modules.
- Pros: Simple; works for small modular specs (< 20 invariants, constitution ≤ 400 lines).
- Cons: Constitution grows linearly with invariant count; exceeds budget at medium scale.

C) **Three-tier** — system constitution (declarations only) + domain constitution (full definitions) + cross-domain deep context + module.
- Pros: Scales to large specs; domain grouping already present in well-architected systems (double duty); no duplication between tiers.
- Cons: One additional indirection level; requires domain identification.

#### Decision

**Option C: Three-tier with two-tier simplification.** Option C is the full protocol, with Option B as a blessed simplification for small specs (< 20 invariants, constitution ≤ 400 lines). The `tier_mode` manifest field selects between them — no forced complexity for specs that don't need it, with a clear upgrade path.

// WHY NOT Option A? At scale, the flat root consumes 30–37% of the context budget before the module starts. That's context waste, not management.

#### Consequences

- Authors must identify 2–5 architectural domains when modularizing (usually obvious from architecture overview)
- Two-tier specs migrate to three-tier without restructuring modules (§0.13.13)
- Domain boundaries serve double duty: architectural isolation and context management

#### Tests

- (Validated by INV-014) Bundle budget compliance confirms that the chosen tier mode keeps bundles within ceiling.
- (Validated by INV-011) Module completeness confirms that the constitutional context in each bundle is sufficient.

---

### ADR-007: Cross-Module References Through Constitution Only [Conditional — modular specs only]

#### Problem

When a DDIS spec is modular, how should modules reference content in other modules?

#### Options

A) **Direct references** — "see section 7.3 in the Scheduler module."
- Pros: Natural; mirrors monolithic cross-references.
- Cons: Creates invisible dependencies. Module A's bundle needs Module B — defeating modularization. Violates INV-011.

B) **Through constitution only** — Module A references APP-INV-NNN in the constitution, never Module B's internals.
- Pros: Enforces isolation mechanically; bundles are self-contained.
- Cons: Authors must extract all cross-module contracts into the constitution; feels indirect for tightly coupled subsystems.

#### Decision

**Option B: Through constitution only.** INV-012 enforces this mechanically. Cross-module contracts are expressed as invariants or shared types in the constitution, never as references to another module's internals.

// WHY NOT Option A? It breaks INV-011. Module A's bundle would need Module B's implementation — the very thing modularization avoids.

#### Consequences

- All cross-module contracts must be elevated to the constitution
- Modules become truly self-contained; tight coupling becomes visible in the constitution's interface surface

#### Tests

- (Validated by INV-012) Mechanical check (CHECK-7 in §0.13.11) scans modules for direct cross-module references.
- (Validated by INV-011) LLM bundle sufficiency test confirms modules don't need each other's content.

---

## Modularization Quality Gates [Conditional — modular specs only]

In addition to Gates 1–7, modular specs must pass these gates. A failing Gate M-1 makes Gates M-2 through M-5 irrelevant.

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

---

## 0.13 Modularization Protocol [Conditional]

REQUIRED when the monolithic spec exceeds 4,000 lines or when the target context window cannot hold the full spec plus reasoning budget. OPTIONAL but recommended for 2,500–4,000 line specs.

> Namespace note: INV-001 through INV-020 and ADR-001 through ADR-011 are DDIS meta-standard invariants/ADRs (defined in this standard). Application specs using DDIS define their OWN invariant namespace (e.g., APP-INV-NNN) — never reuse the meta-standard's INV-NNN space. Examples in this section use APP-INV-NNN to demonstrate this convention.

### 0.13.1 The Scaling Problem

When the spec exceeds the LLM's context window, two failure modes emerge:

1. **Truncation**: The LLM silently drops content from the beginning, losing invariants and the formal model.

2. **Naive splitting**: Arbitrary splits break cross-references, orphan invariants, and force guessing at contracts in unseen sections.

The modularization protocol prevents both with principled decomposition and formal completeness guarantees. (Motivated by INV-008, INV-014.)

### 0.13.2 Core Concepts

**Monolith**: A DDIS spec that exists as a single document. All specs start as monoliths. Most small-to-medium specs remain monoliths.

**Module**: A self-contained unit of the spec covering one major subsystem. Each module corresponds to one chapter of PART II in the monolithic structure. A module is never read alone — it is always assembled into a bundle with the appropriate constitutional context.

**Constitution**: The cross-cutting material that constrains all modules. Contains the formal model, invariants, ADRs, quality gates, architecture overview, glossary, and performance budgets. Organized in tiers to manage its own size.

**Domain**: An architectural grouping of related modules that share tighter coupling with each other than with modules in other domains. Domains correspond to rings, layers, or crate groups in the architecture overview.

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
|  - All implementation detail for one major subsystem         |
|  SCOPE: What to build for this subsystem.                    |
+--------------------------------------------------------------+

Assembled bundle: Tier 1 + Tier 2 + Tier 3 + Module
Target budget:    1,200 - 4,500 lines per bundle
Hard ceiling:     5,000 lines (must fit in context with reasoning room)
```

// WHY THREE TIERS? Two tiers work for < 20 invariants / < 10 ADRs. Beyond that, the root exceeds budget. Three tiers add domain grouping — already present in well-architected systems. The domain boundary serves double duty: architectural isolation and context management. See ADR-006.

### 0.13.4 Invariant Declarations vs. Definitions

An invariant has two representations:

**Declaration** (Tier 1, always present, ~1 line):
```
APP-INV-017: Event log is append-only -- Owner: EventStore -- Domain: Storage
```

**Definition** (Tier 2, in the owning domain's constitution, ~10-20 lines):
```
**APP-INV-NNN: Event Log Append-Only**

*Events, once written, are never modified or deleted.*

  ∀ event ∈ EventLog, ∀ t1 < t2:
    event ∈ EventLog(t1) → event ∈ EventLog(t2) ∧ event(t1) = event(t2)

Violation scenario: A compaction routine rewrites old events to save space,
silently changing event payloads. Replay produces different state.

Validation: Write 1000 events, snapshot the log, run any operation, compare
log prefix byte-for-byte.

// WHY THIS MATTERS: Append-only is the foundation of deterministic replay.
// Without it, replay determinism is impossible.
```

**Inclusion rules — which tier provides which level of detail:**

| Module's relationship to invariant     | Tier 1      | Tier 2 (own domain)              | Tier 3 (cross-domain)  |
|---------------------------------------|-------------|----------------------------------|------------------------|
| Module MAINTAINS this invariant        | Declaration | Full definition (already present) | — (same domain rule)  |
| INTERFACES, invariant in SAME domain  | Declaration | Full definition (already present) | —                     |
| INTERFACES, invariant in OTHER domain | Declaration | —                               | Full definition        |
| No relationship                       | Declaration | —                               | —                     |

Key insight: a module's maintained invariants are ALWAYS in its own domain (enforced by CHECK-4). Therefore Tier 2 always covers them; Tier 3 ONLY adds cross-domain content, eliminating duplication. The same pattern applies to ADRs.

### 0.13.5 Module Header (Required per Module)

Every module begins with a structured header that makes the module self-describing. The header uses application-level invariant identifiers (APP-INV-NNN), not the DDIS meta-standard's INV-NNN identifiers.

Two formats are valid:

**YAML frontmatter (preferred)** — machine-parseable, used when tooling consumes the headers. This standard uses this format.

```yaml
---
module: [module-name]
domain: [domain-name]
maintains: [APP-INV-017, APP-INV-018, APP-INV-019]
interfaces: [APP-INV-003, APP-INV-032]
implements: [APP-ADR-003, APP-ADR-011]
adjacent: [EventStore, Scheduler]
negative_specs:
  - "Must NOT directly access TUI rendering state (use event bus)"
  - "Must NOT bypass the reservation system for file writes"
  - "Must NOT assume event ordering beyond the guarantees in APP-INV-017"
---
```

**Comment-style** — for specs without assembly tooling, embedded directly in the document:

```yaml
# Module Header: [Module Name]
# Domain: [Domain Name]
# Maintains: APP-INV-017, APP-INV-018, APP-INV-019
# Interfaces: APP-INV-003 (via EventStore), APP-INV-032 (via Scheduler)
# Negative specs: Must NOT directly access TUI rendering state (use event bus)
```

**DO NOT** omit the `negative_specs` field in the module header — without it, the assembly script cannot populate subsystem-specific constraints and LLMs implementing the module lack DO NOT guidance. (Validates INV-011, INV-017.)

The module header is consumed by:
1. **The assembly script** — to determine what context to include in the bundle (YAML frontmatter preferred)
2. **The LLM implementer** — to understand scope boundaries before reading
3. **The RALPH loop** — to determine module dependencies for improvement ordering

### 0.13.6 Cross-Module Reference Rules

**Rule 1: Cross-module references go through the constitution, never direct.** (Enforced by INV-012, locked by ADR-007.)

```
BAD:  "See section 7.3 in the Scheduler chapter for the dispatch algorithm"
GOOD: "This subsystem publishes SchedulerReady events (see APP-INV-032,
       maintained by the Scheduler module)"
```

The invariant lives in the constitution. Both modules can reference it without needing each other's content. The LLM implementing Module A never sees Module B's internals — only the contract (invariant) that Module B must satisfy.

> **Meta-standard note:** When the constitution includes a Section Map (§0.3), the pattern "§X.Y, [module-name] module" is a Section Map-mediated reference, not a direct cross-module reference. The Section Map is constitutional content that routes navigation. See the carve-out note on INV-012 above.

**Rule 2: Shared types are defined in the constitution, not in any module.**

If two modules both use `TaskId` or `EventPayload`, the type definition lives in the domain constitution (Tier 2) or the system constitution (Tier 1), not in either module. Modules reference the type; they don't define it.

**Rule 3: The end-to-end trace is a special module.**

The end-to-end trace (§5.3, element-specifications module) is the one element that legitimately crosses all module boundaries. It is stored as its own module file with a special header:

```yaml
# Module Header: End-to-End Trace
# Domain: cross-cutting
# Maintains: (none — this module validates, it doesn't implement)
# Interfaces: ALL application invariants
# Purpose: Integration validation, not implementation
# Assembly: Tier 1 + ALL domain constitutions (no Tier 3 needed)
#
# BUDGET NOTE: With 3 domains at ~400 lines each + ~350 lines Tier 1,
# constitutional overhead is ~1,550 lines. The trace itself must fit in
# ~3,450 lines (5,000 ceiling) or ~2,450 lines (4,000 target).
# Sufficient because the trace has NO implementation detail.
```

### 0.13.7 Modularization Decision Flowchart

```
Is spec > 4,000 lines?
  |-- No  -> Is spec > 2,500 lines AND target context < 8K lines?
  |           |-- No  -> MONOLITH (no modularization needed, stop here)
  |           +-- Yes -> MODULE (recommended)
  +-- Yes -> MODULE (required)
             |
             Does the system constitution (declarations only) + largest module
             fit within the target_lines budget?
             |-- Yes -> TWO-TIER (see §0.13.7.1)
             +-- No  -> Does system have natural domain boundaries?
                        |-- Yes -> THREE-TIER (standard protocol)
                        +-- No  -> Refactor architecture to create domain
                                   boundaries, then THREE-TIER
```

**Self-bootstrapping demonstration** (comparison block per §5.5 format, element-specifications module):
```
// SUBOPTIMAL: Two-tier for large specs (> 20 invariants, multiple domains)
//   - System constitution: ~1,500 lines (all invariant definitions inlined)
//   - Module budget: ~3,500 lines remaining (5,000 ceiling − 1,500)
//   - Every module pays the full constitutional overhead even for unrelated domains
// CHOSEN: Three-tier with domain grouping (ADR-006)
//   - Tier 1 (system): ~350 lines (compact declarations only)
//   - Tier 2 (domain): ~400 lines (only the module's own domain definitions)
//   - Module budget: ~4,250 lines remaining — 21% more implementation capacity
//   - See ADR-006 for full analysis
```

#### 0.13.7.1 Two-Tier Simplification

For modular specs where bundles fit within the target budget, the domain tier can be skipped. Two-tier mode has two variants:

**Variant A: Full-definition constitution.** For specs with few invariants/ADRs (< 20 total):
- **Tier 1 (System Constitution)**: Contains BOTH declarations AND full definitions for all invariants and ADRs (since there are few enough to fit in ≤ 400 lines).

**Variant B: Declaration-only constitution.** For specs with more invariants/ADRs where full definitions live in the owning modules:
- **Tier 1 (System Constitution)**: Contains declarations only (ID + 1-line statement + owner). Full definitions are in the modules that maintain each invariant/ADR. This works when the constitution + largest module fits within `target_lines`.

In both variants:
- **Tier 2 (Domain Constitution)**: SKIPPED. Does not exist in the file layout.
- **Tier 3 (Cross-Domain Deep)**: SKIPPED. Not needed because either Tier 1 has full definitions (Variant A) or modules are self-contained with their own full definitions (Variant B).
- **Module**: Unchanged.

Assembly in two-tier mode: `system_constitution + module → bundle`.

**Self-bootstrapping note**: This standard uses Variant B. It has 20 invariants + 11 ADRs = 31 items (exceeding the Variant A threshold), but all bundles are 1,144–1,442 lines — well within the 4,000-line target. The declaration-only constitution (524 lines) keeps bundles compact. Three-tier would add complexity without benefit. (Validates ADR-006, ADR-004.)

The manifest uses `tier_mode: two-tier` to signal this to the assembly script. If the spec grows beyond the two-tier threshold (constitution + largest module > `target_lines`), migrate to three-tier by extracting domain constitutions (see Migration Procedure §0.13.13).

### 0.13.8 File Layout

```
spec-project/
|-- manifest.yaml                     # Single source of truth for assembly
|-- constitution/
|   |-- system.md                     # Tier 1: always included
|   +-- domains/                      # Tier 2: one per domain (absent in two-tier)
|       |-- storage.md
|       |-- coordination.md
|       +-- presentation.md
|-- deep/                             # Tier 3: one per module (only if cross-domain)
|   |-- scheduler.md                  # Cross-domain context for scheduler module
|   +-- integration_tests.md          # Cross-domain context for integration module
|   # NOTE: modules with no cross-domain interfaces have NO file here.
|   # The assembly script treats missing deep/ file as empty Tier 3.
|-- modules/                           # One per subsystem
|   |-- event_store.md
|   |-- snapshot_manager.md
|   |-- scheduler.md
|   |-- reservation_manager.md
|   |-- tui_renderer.md
|   |-- widget_system.md
|   +-- end_to_end_trace.md           # Special cross-cutting module
|-- bundles/                          # Generated by assembly (gitignored)
|   |-- event_store_bundle.md
|   |-- scheduler_bundle.md
|   +-- ...
+-- .beads/                           # Gap/module tracking (if beads enabled)
    +-- beads.db
```

### 0.13.9 Manifest Schema

```yaml
# manifest.yaml — Single source of truth for DDIS module assembly
ddis_version: "3.0"
spec_name: "Example System"
tier_mode: "three-tier"               # "two-tier" or "three-tier"

context_budget:
  target_lines: 4000                  # Preferred max (WARN if exceeded)
  hard_ceiling_lines: 5000            # Absolute max (ERROR if exceeded)
  reasoning_reserve: 0.25             # Fraction reserved for LLM reasoning

constitution:
  system: "constitution/system.md"    # Tier 1: always required
  domains:                            # Tier 2: absent if tier_mode = "two-tier"
    storage:
      file: "constitution/domains/storage.md"
      description: "Event store, snapshots, persistence layer"
    coordination:
      file: "constitution/domains/coordination.md"
      description: "Scheduling, reservations, task DAG"
    presentation:
      file: "constitution/domains/presentation.md"
      description: "TUI rendering, widgets, layout engine"

modules:
  event_store:
    file: "modules/event_store.md"
    domain: storage
    maintains: [APP-INV-003, APP-INV-017, APP-INV-018]
    interfaces: [APP-INV-001, APP-INV-005]
    implements: [APP-ADR-003, APP-ADR-011]
    adjacent: [snapshot_manager, scheduler]
    deep_context: null                # null = no cross-domain context needed
    negative_specs:
      - "Must NOT directly access TUI rendering state"
      - "Must NOT bypass reservation system for writes"

  scheduler:
    file: "modules/scheduler.md"
    domain: coordination
    maintains: [APP-INV-022, APP-INV-023, APP-INV-024]
    interfaces: [APP-INV-003, APP-INV-017]  # In Storage domain = cross-domain!
    implements: [APP-ADR-005, APP-ADR-008]
    adjacent: [reservation_manager, event_store]
    deep_context: "deep/scheduler.md"       # HAS cross-domain context
    negative_specs:
      - "Must NOT hold hard locks (advisory only per APP-ADR-005)"
      - "Must NOT read TUI state directly"

  end_to_end_trace:
    file: "modules/end_to_end_trace.md"
    domain: cross-cutting
    maintains: []
    interfaces: all
    implements: []
    adjacent: all
    deep_context: null                      # Gets ALL Tier 2 instead
    negative_specs: []

invariant_registry:
  APP-INV-001: { owner: system, domain: system, description: "Causal traceability" }
  APP-INV-003: { owner: event_store, domain: storage, description: "Replay determinism" }
  APP-INV-017: { owner: event_store, domain: storage, description: "Append-only log" }
  APP-INV-022: { owner: scheduler, domain: coordination, description: "Fair scheduling" }
  # ... (abbreviated for illustration — real manifests list all invariants)
```

**DO NOT** define modules without the `maintains` field — an unowned invariant violates INV-013 and leaves nobody responsible for testing it. (Validates INV-013.)

### 0.13.10 Assembly Rules

The assembly script reads the manifest and produces one bundle per module.

**Three-tier assembly (tier_mode: three-tier):**

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
    ERROR("Bundle {module_name}: {total_lines} lines exceeds ceiling "
          "{hard_ceiling_lines}. INV-014 VIOLATED.")
  elif total_lines > manifest.context_budget.target_lines:
    WARN("Bundle {module_name}: {total_lines} lines exceeds target "
         "{target_lines}.")

  write(bundles/{module_name}_bundle.md, join(bundle))
```

**Two-tier assembly (tier_mode: two-tier):**

```
ASSEMBLE(module_name):
  module = manifest.modules[module_name]
  bundle = []

  # Tier 1 contains FULL definitions in two-tier mode
  bundle.append(read(manifest.constitution.system))
  # No Tier 2, no Tier 3
  bundle.append(read(module.file))

  validate_budget(bundle, module_name)
  write(bundles/{module_name}_bundle.md, join(bundle))
```

**Complexity**: Assembly is O(T) file reads per module (T = number of tiers, at most 3). Full rebuild is O(M × T) where M = number of modules. Space is O(max_bundle_size) per assembly. For a typical spec with 8 modules and 3 tiers, full assembly reads ≤ 32 files.

**Worked example**: Assembling the `scheduler` module in three-tier mode:
- Tier 1: `constitution/system.md` — 350 lines
- Tier 2: `constitution/domains/coordination.md` — 400 lines
- Tier 3: `deep/scheduler.md` — 200 lines
- Module: `modules/scheduler.md` — 2,500 lines
- **Total**: 3,450 lines (under 4,000 target, under 5,000 ceiling)

**Edge cases**:
- **Missing file**: If any referenced path (constitution, domain, deep context, or module) does not exist, ABORT with an error naming the missing file and the manifest entry that references it. **DO NOT** silently skip missing files during assembly — a missing file is a manifest-spec synchronization failure (INV-016), not an optional component. (Detected by CHECK-9.)
- **Empty module**: If a module file exists but contains 0 lines, WARN and produce a minimal bundle (constitution only). The author likely forgot to populate the module.
- **Missing deep context**: If `deep_context` is non-null but the file does not exist, ABORT. This is a manifest-spec synchronization failure. (Detected by CHECK-9.)
- **Manifest schema violation**: If a module entry lacks required fields (`file`, `domain`, `maintains`), ABORT before assembly with a validation error listing the missing fields.

### 0.13.11 Consistency Validation

Nine mechanical checks. All implementable by a validation script.

**CHECK-1: Invariant ownership completeness**
```
∀ inv ∈ invariant_registry:
  (inv.owner = "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 0)
  ∨ (inv.owner ≠ "system" ∧ count(s ∈ modules : inv ∈ s.maintains) = 1)
```
Remediation: Assign unowned invariant or remove duplicate owner.

**CHECK-2: Interface consistency**
```
∀ s ∈ modules, ∀ inv ∈ s.interfaces (where s.interfaces ≠ "all"):
  (∃ other ∈ modules : inv ∈ other.maintains ∧ other ≠ s)
  ∨ invariant_registry[inv].owner = "system"
```
Remediation: Add invariant to appropriate maintains list or register as system-owned.

**CHECK-3: Adjacency symmetry**
```
∀ a ∈ modules, ∀ b ∈ a.adjacent
  (where a.adjacent ≠ "all" ∧ b.adjacent ≠ "all"):
    a.name ∈ manifest.modules[b].adjacent
```
Remediation: Add missing adjacency entry.

**CHECK-4: Domain membership consistency**
```
∀ s ∈ modules (where s.domain ≠ "cross-cutting"),
  ∀ inv ∈ s.maintains:
    invariant_registry[inv].domain = s.domain
    ∨ invariant_registry[inv].domain = "system"
```
Remediation: Move invariant to module's domain or move module to invariant's domain.

**CHECK-5: Budget compliance**
```
∀ s ∈ modules:
  line_count(ASSEMBLE(s)) ≤ context_budget.hard_ceiling_lines
```
Remediation: Reduce module size, move content to constitution, or split module. (Validates INV-014.)

**CHECK-6: No orphan invariants**
```
∀ inv ∈ invariant_registry:
  ∃ s ∈ modules : inv ∈ s.maintains ∨ inv ∈ s.interfaces
```
Remediation: Add invariant to a module's interfaces or remove from registry.

**CHECK-7: Cross-module reference isolation**
```
∀ module_file ∈ module_files:
  ¬contains(module_file, pattern matching direct module-to-module references)
```
Remediation: Replace direct references with constitutional references. (Validates INV-012.)

**CHECK-8: Deep context correctness (three-tier only)**
```
∀ s ∈ modules (where s.domain ≠ "cross-cutting"):
  let xd = {inv ∈ s.interfaces :
    invariant_registry[inv].domain ≠ s.domain
    ∧ invariant_registry[inv].domain ≠ "system"}
  (count(xd) > 0 ⟹ s.deep_context ≠ null)
  ∧ (count(xd) = 0 ⟹ s.deep_context = null)
```
Remediation: Create missing deep context file or remove unnecessary one.

**CHECK-9: File existence**
```
∀ path ∈ manifest.all_referenced_paths: file_exists(path)
∀ module_file ∈ filesystem("modules/"): module_file ∈ manifest.modules.*.file
```
Remediation: Create missing file or correct manifest path. Second clause catches module files that exist on disk but are missing from the manifest. (Validates INV-016.)

#### Worked Examples for CHECK-1 through CHECK-9

Minimal manifest: two modules (event_store, scheduler), three invariants (APP-INV-NNN through APP-INV-PPP).

```
CHECK-1/6 (Ownership):
  ✓ APP-INV-001 owner=event_store, event_store.maintains=[APP-INV-001] → one owner, referenced
  ✗ Both event_store and scheduler maintain APP-INV-001 → duplicate owner. Fix: remove from one.
  ✗ APP-INV-003 owner=system, no module lists it → orphan. Fix: add to scheduler.interfaces.

CHECK-2 (Interface consistency):
  ✓ scheduler.interfaces=[APP-INV-001], event_store.maintains=[APP-INV-001]
  ✗ scheduler.interfaces=[APP-INV-099], no module maintains it. Fix: assign owner or register as system.

CHECK-3 (Adjacency symmetry):
  ✓ event_store.adjacent=[scheduler], scheduler.adjacent=[event_store] → bidirectional
  ✗ event_store.adjacent=[scheduler], scheduler.adjacent=[] → asymmetric. Fix: add reciprocal entry.

CHECK-4 (Domain membership):
  ✓ scheduler.domain=orchestration, APP-INV-002.domain=orchestration → match
  ✗ scheduler.domain=orchestration maintains APP-INV-001.domain=storage → mismatch. Fix: reassign.

CHECK-5 (Budget compliance):
  ✓ ASSEMBLE(scheduler) = 3,800 lines, ceiling = 5,000
  ✗ ASSEMBLE(event_store) = 5,200 lines > ceiling. Fix: split module or move content to constitution.

CHECK-7 (Cross-module isolation):
  ✓ scheduler.md references "APP-INV-001" (constitutional)
  ✗ scheduler.md references "event_store §3.2" directly. Fix: extract to constitution.

CHECK-8 (Deep context):
  ✓ scheduler has cross-domain interface (APP-INV-001, storage domain) and deep_context file
  ✗ Same scenario but deep_context=null. Fix: create deep context file.

CHECK-9 (File existence):
  ✓ manifest references "modules/scheduler.md", file exists
  ✗ manifest references "modules/cache.md", file missing. Fix: create or remove from manifest.
  ✗ "modules/legacy.md" on disk but not in manifest. Fix: add to manifest or remove file.
```

### 0.13.12 Cascade Protocol

When constitutional content changes, affected modules must be re-validated.

**Blast radius by change type:**

| Change                          | Blast Radius                     |
|---------------------------------|----------------------------------|
| Invariant wording changed       | Modules maintaining or interfacing |
| ADR superseded                  | Modules implementing that ADR     |
| New invariant added             | Module assigned as owner          |
| Shared type changed             | Same-domain + cross-domain users |
| Non-negotiable changed          | ALL modules                       |
| Glossary term redefined         | All modules using that term       |

**Cascade workflow (with beads):**

```
1. Author changes APP-INV-017 in constitution/domains/storage.md
2. Run: ddis_validate.sh --check-cascade APP-INV-017
3. Script queries manifest for affected modules:
   - event_store (maintains APP-INV-017) → MUST re-validate
   - snapshot_manager (interfaces APP-INV-017) → SHOULD re-validate
   - scheduler (interfaces APP-INV-017 via deep) → SHOULD re-validate
4. Script creates/reopens br issues for affected modules
   Label: cascade:APP-INV-017, priority by blast radius
5. bv --robot-plan shows improvement order
6. Re-run assembly, re-validate affected modules
```

**Cascade workflow (without beads — manifest-only fallback):**

```
1-3. Same as above.
4. Script prints affected modules to stdout:
   MUST:   event_store
   SHOULD: snapshot_manager, scheduler
5. Re-run assembly, manually re-validate affected modules
```

Both paths use the same manifest query. Beads adds persistence and ordering; the manifest provides the data either way.

**Complexity**: Cascade identification is O(I × M) where I = number of changed invariants/ADRs and M = number of modules (scanning the manifest's invariant registry for each changed item). Re-validation cost is O(A × V) where A = affected modules and V = validation cost per module (dominated by Gate 7 LLM test if applied). Edge case: if a cascaded change to module A triggers further constitutional changes, repeat the cascade — but this should not recurse more than once since modules cannot modify the constitution.

### 0.13.13 Monolith-to-Module Migration Procedure

**Step 1: Identify domains.**
Group PART II chapters into 2–5 domains based on architectural boundaries.

**Step 2: Extract system constitution.**
From monolith to `constitution/system.md`: preamble, PART 0 sections, all invariant DECLARATIONS, all ADR DECLARATIONS, glossary (1-line definitions), quality gates, non-negotiables, non-goals.

**Step 3: Extract domain constitutions.**
For each domain to `constitution/domains/{domain}.md`: domain formal model, full invariant definitions owned by domain, full ADR analysis decided in domain, cross-domain interface contracts, domain performance budgets.

**Step 4: Extract modules.**
For each PART II chapter to `modules/{subsystem}.md`: add module header (§0.13.5), include implementation content, convert cross-module direct references to constitutional references (hardest step — see INV-012).

**DO NOT** defer cross-module reference conversion to after Step 4 — convert them during extraction while the monolith is still available for context. Deferring produces bundles that violate INV-012.

**Step 5: Create cross-domain deep context files.**
For each module interfacing with other-domain invariants: create `deep/{module}.md` with full definitions for cross-domain invariants, interface contracts, shared types.

**Step 6: Build manifest.**
Create `manifest.yaml` with all module entries, invariant registry, context budget.

**Step 7: Validate.**
Run `ddis_validate.sh` — all nine checks must pass.

**Step 8: Extract end-to-end trace.**
Create `modules/end_to_end_trace.md` as cross-cutting module. Verify bundle fits within budget.

**Step 9: LLM validation.**
Give 2+ bundles to an LLM. Zero questions requiring other module's implementation.

### Verification Prompt for Modularization Protocol (§0.13)

After modularizing your spec, verify:

1. [ ] Every module header declares maintained invariants, interfaces, and negative specs (§0.13.5, INV-011)
2. [ ] Cross-module references go through constitutional elements, never direct module internals (INV-012, ADR-007)
3. [ ] Every invariant is maintained by exactly one module (INV-013, CHECK-1)
4. [ ] Every assembled bundle fits within the hard ceiling (INV-014, CHECK-5)
5. [ ] Constitution declarations faithfully summarize module definitions (INV-015, Gate M-4)
6. [ ] Your modularization does NOT have modules referencing each other's internal sections (INV-012)
7. [ ] Your manifest does NOT have orphan invariants or missing file paths (CHECK-6, CHECK-9)
8. [ ] *Integration*: Your cascade protocol correctly identifies affected modules for a simulated invariant change (Gate M-5)
