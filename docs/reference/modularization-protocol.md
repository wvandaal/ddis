# DDIS Modularization Protocol — Addendum to DDIS Standard v1.0

> This document extends DDIS with a formal protocol for decomposing specifications
> that exceed a single LLM context window. It adds new invariants (INV-011 through
> INV-016), new ADRs (ADR-006, ADR-007), and a new required section (§0.13) for
> specs that trigger the modularization threshold.
>
> Namespace note: INV-001 through INV-010 and ADR-001 through ADR-005 are DDIS
> meta-standard invariants/ADRs (defined in the base standard). INV-011 through
> INV-016 and ADR-006 through ADR-007 are modularization protocol meta-invariants/ADRs
> (defined here). Application specs using DDIS define their OWN invariant namespace
> (e.g., APP-INV-001) — never reuse the meta-standard's INV-NNN space.

---

## §0.13 Modularization Protocol [Conditional]

This section is REQUIRED when the monolithic specification exceeds 4,000 lines or
when the target context window (model-dependent) cannot hold the full spec plus
a meaningful working budget for LLM reasoning. It is OPTIONAL but recommended for
specs between 2,500–4,000 lines.

### 0.13.1 The Scaling Problem

A DDIS spec's value depends on the implementer holding sufficient context to
produce correct output without guessing. When the spec exceeds the implementer's
context window, two failure modes emerge:

1. **Truncation**: The LLM silently drops content from the beginning of the context,
   losing invariants and formal model — the very elements that prevent hallucination.

2. **Naive splitting**: Arbitrary file splits break cross-references, orphan
   invariants from the sections they constrain, and force the LLM to guess at
   contracts defined in unseen sections.

The modularization protocol prevents both failures by defining a principled decomposition
with formal completeness guarantees.

### 0.13.2 Core Concepts

**Monolith**: A DDIS spec that exists as a single document. All specs start as
monoliths. Most small-to-medium specs remain monoliths.

**Module**: A self-contained unit of the spec covering one major subsystem. Each
module corresponds to one chapter of PART II in the monolithic structure. A module
is never read alone — it is always assembled into a bundle with the appropriate
constitutional context.

**Constitution**: The cross-cutting material that constrains all modules. Contains
the formal model, invariants, ADRs, quality gates, architecture overview, glossary,
and performance budgets. Organized in tiers to manage its own size.

**Domain**: An architectural grouping of related modules that share tighter coupling
with each other than with modules in other domains. Domains correspond to rings,
layers, or crate groups in the architecture overview.

**Bundle**: The assembled document sent to an LLM for implementation. Always
contains: system constitution + domain constitution + cross-domain deep context
+ the module itself. A bundle is the unit of LLM consumption.

**Manifest**: A machine-readable YAML file that declares all modules, their domain
membership, invariant ownership, cross-module interfaces, and assembly rules. The
manifest is the single source of truth for the assembly script.

### 0.13.3 The Tiered Constitution

The constitution is organized in three tiers to prevent it from becoming a
bottleneck itself. Each tier has a hard line budget, a clear scope, and NO
overlapping content between tiers.

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

// WHY THREE TIERS? Two tiers (root + module) works for systems with < 20
// invariants and < 10 ADRs. Beyond that, the root itself exceeds budget.
// Three tiers add one level of indirection -- domain grouping -- which is
// already present in any well-architected system. The domain boundary serves
// double duty: it was already an isolation mechanism in the architecture,
// now it is also a context management mechanism. See ADR-006.

### 0.13.4 Invariant Declarations vs. Definitions

The critical mechanism that makes the tiered constitution work. An invariant
has two representations:

**Declaration** (Tier 1, always present, ~1 line):
```
APP-INV-017: Event log is append-only -- Owner: EventStore -- Domain: Storage
```

**Definition** (Tier 2, in the owning domain's constitution, ~10-20 lines):
```
**APP-INV-017: Event Log Append-Only**

*Events, once written, are never modified or deleted.*

  forall event in EventLog, forall t1 < t2:
    event in EventLog(t1) -> event in EventLog(t2) AND event(t1) = event(t2)

Violation scenario: A compaction routine rewrites old events to save space,
silently changing event payloads. Replay produces different state.

Validation: Write 1000 events, snapshot the log, run any operation, compare
log prefix byte-for-byte.

// WHY THIS MATTERS: Append-only is the foundation of deterministic replay.
// Without it, APP-INV-003 (replay determinism) is impossible.
```

**Inclusion rules -- which tier provides which level of detail:**

| Module's relationship to invariant     | Tier 1      | Tier 2 (own domain)              | Tier 3 (cross-domain)  |
|---------------------------------------|-------------|----------------------------------|------------------------|
| Module MAINTAINS this invariant        | Declaration | Full definition (already present) | -- (same domain rule)  |
| INTERFACES, invariant in SAME domain  | Declaration | Full definition (already present) | --                     |
| INTERFACES, invariant in OTHER domain | Declaration | --                               | Full definition        |
| No relationship                       | Declaration | --                               | --                     |

Key insight: a module's maintained invariants are ALWAYS in its own domain (enforced
by CHECK-4 in section 0.13.11). Therefore Tier 2 always covers them. Tier 3 ONLY
adds cross-domain content. This eliminates all duplication between tiers.

The same pattern applies to ADRs: declarations in Tier 1 always, full analysis
in the domain that decided them, cross-domain inclusion in Tier 3 only when a
module in another domain implements or is affected by the ADR.

### 0.13.5 Module Header (Required per Module)

Every module begins with a structured header that makes the module self-describing.
The header uses application-level invariant identifiers (APP-INV-NNN), not the
DDIS meta-standard's INV-NNN identifiers:

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

The module header is consumed by:
1. **The assembly script** -- to determine what context to include in the bundle
2. **The LLM implementer** -- to understand scope boundaries before reading
3. **The RALPH loop** -- to determine module dependencies for improvement ordering

### 0.13.6 Cross-Module Reference Rules

**Rule 1: Cross-module references go through the constitution, never direct.**

```
BAD:  "See section 7.3 in the Scheduler chapter for the dispatch algorithm"
GOOD: "This subsystem publishes SchedulerReady events (see APP-INV-032,
       maintained by the Scheduler module)"
```

The invariant lives in the constitution. Both modules can reference it without
needing each other's content. The LLM implementing Module A never sees Module B's
internals -- only the contract (invariant) that Module B must satisfy.

**Rule 2: Shared types are defined in the constitution, not in any module.**

If two modules both use `TaskId` or `EventPayload`, the type definition lives in
the domain constitution (Tier 2) or the system constitution (Tier 1), not in
either module. Modules reference the type; they don't define it.

**Rule 3: The end-to-end trace is a special module.**

The end-to-end trace (DDIS section 5.3) is the one element that legitimately
crosses all module boundaries. It is stored as its own module file with a special
header:

```yaml
# Module Header: End-to-End Trace
# Domain: cross-cutting
# Maintains: (none -- this module validates, it doesn't implement)
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
             How many invariants + ADRs total?
             |-- < 20 total AND system constitution fits in <= 400 lines
             |    -> TWO-TIER (see section 0.13.7.1)
             +-- >= 20 total OR system constitution > 400 lines
                  -> Does system have natural domain boundaries?
                     |-- Yes -> THREE-TIER (standard protocol)
                     +-- No  -> Refactor architecture to create domain
                                boundaries, then THREE-TIER
```

#### 0.13.7.1 Two-Tier Simplification

For small modular specs, the domain tier can be skipped. In two-tier mode:

- **Tier 1 (System Constitution)**: Contains BOTH declarations AND full definitions
  for all invariants and ADRs (since there are few enough to fit in <= 400 lines).
- **Tier 2 (Domain Constitution)**: SKIPPED. Does not exist in the file layout.
- **Tier 3 (Cross-Domain Deep)**: SKIPPED. Not needed because all full definitions
  are already in Tier 1.
- **Module**: Unchanged.

Assembly in two-tier mode: `system_constitution + module -> bundle`.

The manifest uses `tier_mode: two-tier` to signal this to the assembly script.
If the spec grows beyond the two-tier threshold, migrate to three-tier by
extracting domain constitutions (see Migration Procedure section 0.13.14).

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
# manifest.yaml -- Single source of truth for DDIS module assembly
ddis_version: "1.0"
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

  snapshot_manager:
    file: "modules/snapshot_manager.md"
    domain: storage
    maintains: [APP-INV-019, APP-INV-020]
    interfaces: [APP-INV-003, APP-INV-017]
    implements: [APP-ADR-012]
    adjacent: [event_store]
    deep_context: null
    negative_specs:
      - "Must NOT modify events (append-only per APP-INV-017)"

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
  APP-INV-005: { owner: system, domain: system, description: "Perf verifiability" }
  APP-INV-017: { owner: event_store, domain: storage, description: "Append-only log" }
  APP-INV-018: { owner: event_store, domain: storage, description: "Event ordering" }
  APP-INV-019: { owner: snapshot_manager, domain: storage, description: "Snapshot consistency" }
  APP-INV-020: { owner: snapshot_manager, domain: storage, description: "Snapshot recovery" }
  APP-INV-022: { owner: scheduler, domain: coordination, description: "Fair scheduling" }
  APP-INV-023: { owner: scheduler, domain: coordination, description: "No starvation" }
  APP-INV-024: { owner: scheduler, domain: coordination, description: "Dep-aware dispatch" }
```

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

### 0.13.11 Consistency Validation

Nine mechanical checks. All implemented by the validation script.

**CHECK-1: Invariant ownership completeness**
```
forall inv in invariant_registry:
  (inv.owner = "system" AND count(s in modules : inv in s.maintains) = 0)
  OR (inv.owner != "system" AND count(s in modules : inv in s.maintains) = 1)
```
Remediation: Assign unowned invariant or remove duplicate owner.

**CHECK-2: Interface consistency**
```
forall s in modules, forall inv in s.interfaces (where s.interfaces != "all"):
  (exists other in modules : inv in other.maintains AND other != s)
  OR invariant_registry[inv].owner = "system"
```
Remediation: Add invariant to appropriate maintains list or register as system-owned.

**CHECK-3: Adjacency symmetry**
```
forall a in modules, forall b in a.adjacent
  (where a.adjacent != "all" AND b.adjacent != "all"):
    a.name in manifest.modules[b].adjacent
```
Remediation: Add missing adjacency entry.

**CHECK-4: Domain membership consistency**
```
forall s in modules (where s.domain != "cross-cutting"),
  forall inv in s.maintains:
    invariant_registry[inv].domain = s.domain
    OR invariant_registry[inv].domain = "system"
```
Remediation: Move invariant to module's domain or move module to invariant's domain.

**CHECK-5: Budget compliance**
```
forall s in modules:
  line_count(ASSEMBLE(s)) <= context_budget.hard_ceiling_lines
```
Remediation: Reduce module size, move content to constitution, or split module.

**CHECK-6: No orphan invariants**
```
forall inv in invariant_registry:
  exists s in modules : inv in s.maintains OR inv in s.interfaces
```
Remediation: Add invariant to a module's interfaces or remove from registry.

**CHECK-7: Cross-module reference isolation**
```
forall module_file in module_files:
  NOT contains(module_file, pattern matching direct module-to-module references)
```
Remediation: Replace direct references with constitutional references.

**CHECK-8: Deep context correctness (three-tier only)**
```
forall s in modules (where s.domain != "cross-cutting"):
  let xd = {inv in s.interfaces :
    invariant_registry[inv].domain != s.domain
    AND invariant_registry[inv].domain != "system"}
  (count(xd) > 0 implies s.deep_context != null)
  AND (count(xd) = 0 implies s.deep_context = null)
```
Remediation: Create missing deep context file or remove unnecessary one.

**CHECK-9: File existence**
```
forall path in manifest.all_referenced_paths: file_exists(path)
forall module_file in filesystem("modules/"): module_file in manifest.modules.*.file
```
Remediation: Create missing file or correct manifest path. Second clause catches
module files that exist on disk but are missing from the manifest.

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
   - event_store (maintains APP-INV-017) -> MUST re-validate
   - snapshot_manager (interfaces APP-INV-017) -> SHOULD re-validate
   - scheduler (interfaces APP-INV-017 via deep) -> SHOULD re-validate
4. Script creates/reopens br issues for affected modules
   Label: cascade:APP-INV-017, priority by blast radius
5. bv --robot-plan shows improvement order
6. Re-run assembly, re-validate affected modules
```

**Cascade workflow (without beads -- manifest-only fallback):**

```
1-3. Same as above.
4. Script prints affected modules to stdout:
   MUST:   event_store
   SHOULD: snapshot_manager, scheduler
5. Re-run assembly, manually re-validate affected modules
```

Both paths use the same manifest query. Beads adds persistence and ordering;
the manifest provides the data either way.

---

## New Invariants for Modularization

### INV-011: Module Completeness

*An LLM receiving a properly assembled bundle can implement the module's subsystem
without information from any other module's implementation content.*

```
forall module in modules:
  let bundle = ASSEMBLE(module)
  forall implementation_question Q about module's subsystem:
    bundle.answers(Q) OR Q.answerable_from(general_competence)
```

Violation scenario: The Scheduler module references EventStore's internal ring
buffer layout to determine batching strategy, but the ring buffer details live
only in the EventStore module (not in the constitution or shared types).

Validation: Give a bundle (not the full spec) to an LLM. Track questions that
require information from another module's implementation. Any such question
violates INV-011.

// WHY THIS MATTERS: If module completeness fails, the modularization protocol provides
// no benefit. The entire value proposition is that bundles are sufficient.

---

### INV-012: Cross-Module Isolation

*Modules reference each other only through constitutional elements (invariants,
ADRs, shared types). No module contains direct references to another module's
internal sections, algorithms, or data structures.*

```
forall module_a, module_b in modules where module_a != module_b:
  forall ref in module_a.outbound_references:
    ref.target not_in module_b.internal_sections
    ref.target in {constitution, shared_types, invariants, ADRs}
```

Violation scenario: The TUI Renderer module says "use the same batching strategy
as the EventStore module's flush_batch() function."

Validation: Mechanical: CHECK-7. Semantic: review for implicit references.

---

### INV-013: Invariant Ownership Uniqueness

*Every application invariant is maintained by exactly one module (or explicitly
by the system constitution). No invariant is unowned or multiply-owned.*

```
forall inv in invariant_registry:
  (inv.owner = "system" AND count(s in modules : inv in s.maintains) = 0)
  OR (inv.owner != "system" AND count(s in modules : inv in s.maintains) = 1)
```

Violation scenario: Both EventStore and SnapshotManager list APP-INV-017 in
maintains. Which module's tests are authoritative?

Validation: Mechanical: CHECK-1.

---

### INV-014: Bundle Budget Compliance

*Every assembled bundle fits within the hard ceiling.*

```
forall module in modules:
  line_count(ASSEMBLE(module)) <= context_budget.hard_ceiling_lines
```

Violation scenario: Scheduler module grows to 3,500 lines. With 1,200-line
constitution, bundle is 4,700 lines -- under 5,000 ceiling but over 4,000
target. Assembly warns but does not error. If bundle reaches 5,100 lines,
INV-014 is violated and assembly errors.

Validation: Mechanical: CHECK-5.

// WHY THIS MATTERS: The modularization protocol exists to keep bundles within
// context budget. Budget violations mean added complexity without the benefit.

---

### INV-015: Declaration-Definition Consistency

*Every invariant declaration in the system constitution is a faithful summary of
its full definition in the domain constitution.*

```
forall inv in invariant_registry:
  let decl = system_constitution.declaration(inv)
  let defn = full_definition(inv)
  decl.id = defn.id
  AND decl.one_line is_faithful_summary_of defn.statement
```

Violation scenario: System constitution declares "APP-INV-017: Event log is
append-only" but the Storage domain definition now says "append-only except
during compaction windows."

Validation: Semi-mechanical: extract declaration/definition pairs, present to
reviewer for semantic consistency.

// WHY THIS MATTERS: Divergence means different modules are implemented against
// different understandings of the same invariant.

---

### INV-016: Manifest-Spec Synchronization

*The manifest accurately reflects the current state of all spec files.*

```
forall path in manifest.all_referenced_paths: file_exists(path)
forall inv in manifest.all_referenced_invariants: inv in system_constitution
forall module_file in filesystem("modules/"): module_file in manifest
```

Violation scenario: Author adds modules/new_feature.md but not to manifest.
Assembly never produces a bundle. RALPH never improves it.

Validation: Mechanical: CHECK-9.

---

## New ADRs for Modularization

### ADR-006: Tiered Constitution over Flat Root

**Problem**: When modular, constitutional context must accompany every module.

**Options**:
A) Flat root -- one file with everything. Simple but doesn't scale past ~20 invariants.
B) Two-tier -- system constitution (full definitions) + modules. Works for small specs.
C) Three-tier -- system (declarations) + domain (definitions) + cross-domain deep + modules.

**Decision**: Option C as the full protocol, with Option B as a blessed
simplification for small specs (<20 invariants, system constitution <=400 lines).
The tier_mode field in the manifest selects between them.

// WHY NOT Option A? Against a FrankenTUI-scale spec (25 invariants, 15 ADRs,
// 4,800 lines): flat root ~1,500 lines, leaving only 2,500 for the module.

**Tests**: Validated by INV-014 (budget compliance) and INV-011 (module completeness).

---

### ADR-007: Cross-Module References Through Constitution Only

**Problem**: How should modules reference content in other modules?

**Options**:
A) Direct references ("see section 7.3 in Scheduler"). Natural but creates
   invisible dependencies and breaks INV-011.
B) Through constitution only. Module A references APP-INV-032, which lives in the
   constitution. Never references another module's internals.

**Decision**: Option B. INV-012 enforces mechanically. The constitution is the
"header file"; modules are "implementation files" never directly included.

// WHY NOT Option A? Breaks INV-011. If Module A references Module B's internals,
// Module A's bundle needs Module B -- defeating the purpose of modularization entirely.

**Tests**: Validated by INV-012 (isolation) and INV-011 (completeness).

---

## 0.13.13 Quality Gate Extensions for Modular Specs

In addition to the base DDIS quality gates (section 0.7), modular specs must pass:

**Gate M-1**: All nine checks (CHECK-1 through CHECK-9) pass with zero errors.
**Gate M-2**: Every bundle under hard ceiling. Fewer than 20% exceed target.
**Gate M-3**: LLM receiving one bundle produces zero questions requiring another
module's implementation. Tested on at least 2 representative modules.
**Gate M-4**: Every Tier 1 declaration is faithful summary of Tier 2 definition.
**Gate M-5**: Simulated invariant change correctly identifies all affected modules.

---

## 0.13.14 Monolith-to-Module Migration Procedure

### Step 1: Identify domains
Group PART II chapters into 2-5 domains based on architectural boundaries.

### Step 2: Extract system constitution
From monolith to constitution/system.md: preamble, PART 0 sections, all invariant
DECLARATIONS, all ADR DECLARATIONS, glossary (1-line definitions), quality gates,
non-negotiables, non-goals.

### Step 3: Extract domain constitutions
For each domain to constitution/domains/{domain}.md: domain formal model, full
invariant definitions owned by domain, full ADR analysis decided in domain,
cross-domain interface contracts, domain performance budgets.

### Step 4: Extract modules
For each PART II chapter to modules/{subsystem}.md: add module header, include
implementation content, convert cross-module direct references to constitutional
references (hardest step -- see INV-012).

### Step 5: Create cross-domain deep context files
For each module interfacing with other-domain invariants: create deep/{module}.md
with full definitions for cross-domain invariants, interface contracts, shared types.

### Step 6: Build manifest
Create manifest.yaml with all module entries, invariant registry, context budget.

### Step 7: Validate
Run ddis_validate.sh -- all nine checks must pass.

### Step 8: Extract end-to-end trace
Create modules/end_to_end_trace.md as cross-cutting module. Verify bundle fits.

### Step 9: LLM validation
Give 2+ bundles to an LLM. Zero questions requiring other module's implementation.
