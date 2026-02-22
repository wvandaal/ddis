# Module: Modularization Protocol
<!-- domain: modularization -->
<!-- maintains: INV-011, INV-012, INV-013, INV-014, INV-015, INV-016 -->
<!-- interfaces_with: INV-001, INV-006, INV-008 -->
<!-- adjacent: core-framework -->
<!-- budget: 520 lines -->

## Negative Specifications
- This module MUST NOT redefine core DDIS invariants (INV-001 through INV-010) — it only interfaces with them
- This module MUST NOT contain element-by-element authoring guidance — that belongs to element-specifications
- This module MUST NOT prescribe voice or style conventions — that belongs to guidance-and-practice
- This module MUST NOT override Quality Gates 1–6 — it only extends them with Gates M-1 through M-5

---

## §0.13 Modularization Protocol [Conditional]

REQUIRED when the monolithic specification exceeds 4,000 lines or when the target context window cannot hold the full spec plus reasoning budget. OPTIONAL but recommended for specs between 2,500–4,000 lines.

> Namespace note: INV-001 through INV-016 and ADR-001 through ADR-007 are DDIS meta-standard identifiers. Application specs define their OWN namespace (e.g., APP-INV-001).

### 0.13.1 The Scaling Problem

When a spec exceeds the implementer's context window, two failure modes emerge:

1. **Truncation**: The LLM silently drops content from the beginning, losing invariants and the formal model.
2. **Naive splitting**: Arbitrary splits break cross-references, orphan invariants, and force guessing.

The modularization protocol prevents both by defining principled decomposition with formal completeness guarantees. (Motivated by INV-008.)

### 0.13.2 Core Concepts

- **Monolith**: A single-document DDIS spec. All specs start here.
- **Module**: A self-contained unit covering one major subsystem. Never read alone — always assembled into a bundle.
- **Constitution**: Cross-cutting material constraining all modules. Organized in tiers.
- **Domain**: An architectural grouping of related modules with tighter internal coupling.
- **Bundle**: The assembled document for LLM consumption: system constitution + domain constitution + cross-domain deep context + module.
- **Manifest**: Machine-readable YAML declaring all modules, domains, invariant ownership, and assembly rules.

### 0.13.3 The Tiered Constitution

Three tiers prevent the constitution from becoming a bottleneck. NO overlapping content between tiers. (Locked by ADR-006.)

```
Tier 1: System Constitution (200-400 lines, always)
  Declarations only: ID + 1-line for all invariants and ADRs
  Plus: design goal, non-negotiables, architecture overview, glossary, quality gates

Tier 2: Domain Constitution (200-500 lines, per-domain)
  Full definitions for domain-owned invariants and ADRs
  Domain formal model, interface contracts, performance budgets

Tier 3: Cross-Domain Deep Context (0-600 lines, per-module)
  Full definitions for OTHER-domain invariants this module interfaces with
  Zero overlap with Tier 2. Empty if module has no cross-domain interfaces.

Module (800-3,000 lines)
  Module header + full PART II content for one subsystem

Bundle = Tier 1 + Tier 2 + Tier 3 + Module
Target: 1,200-4,500 lines | Ceiling: 5,000 lines
```

### 0.13.4 Invariant Declarations vs. Definitions

**Declaration** (Tier 1, ~1 line):
```
APP-INV-017: Event log is append-only -- Owner: EventStore -- Domain: Storage
```

**Definition** (Tier 2, ~10-20 lines): Full statement, formal expression, violation scenario, validation method, WHY THIS MATTERS.

**Inclusion rules:**

| Relationship | Tier 1 | Tier 2 | Tier 3 |
|---|---|---|---|
| Module MAINTAINS invariant | Declaration | Full definition | — |
| INTERFACES, same domain | Declaration | Full definition | — |
| INTERFACES, other domain | Declaration | — | Full definition |
| No relationship | Declaration | — | — |

The same pattern applies to ADRs.

### 0.13.5 Module Header (Required per Module)

```yaml
# Module Header: [Module Name]
# Domain: [Domain Name]
# Maintains: APP-INV-017, APP-INV-018, APP-INV-019
# Interfaces: APP-INV-003 (via EventStore), APP-INV-032 (via Scheduler)
# Implements: APP-ADR-003, APP-ADR-011
# Adjacent modules: EventStore, Scheduler
# Assembly: Tier 1 + Domain + cross-domain deep
#
# NEGATIVE SPECIFICATION:
# - Must NOT directly access TUI rendering state
# - Must NOT bypass the reservation system
```

Consumed by: assembly script, LLM implementer, RALPH loop.

### 0.13.6 Cross-Module Reference Rules

**Rule 1**: Cross-module references go through the constitution, never direct. (INV-012, ADR-007.)
```
BAD:  "See section 7.3 in the Scheduler chapter"
GOOD: "See APP-INV-032, maintained by the Scheduler module"
```

**Rule 2**: Shared types are defined in the constitution, not in any module.

**Rule 3**: The end-to-end trace is a special cross-cutting module with `interfaces: all`.

### 0.13.7 Modularization Decision Flowchart

```
Spec > 4,000 lines? → Yes → MODULE (required)
                    → No  → Spec > 2,500 AND context < 8K? → Yes → MODULE (recommended)
                                                            → No  → MONOLITH

If MODULE:
  < 20 invariants+ADRs AND system constitution ≤ 400 lines → TWO-TIER
  Otherwise → THREE-TIER
```

#### Two-Tier Simplification
Tier 1 contains BOTH declarations AND full definitions. No Tier 2 or Tier 3. Assembly: `system_constitution + module → bundle`. Manifest uses `tier_mode: two-tier`.

### 0.13.8 File Layout

```
spec-project/
├── manifest.yaml
├── constitution/
│   ├── system.md                 # Tier 1
│   └── domains/                  # Tier 2 (absent in two-tier)
├── deep/                         # Tier 3 (only if cross-domain)
├── modules/                      # One per subsystem
└── bundles/                      # Generated (gitignored)
```

### 0.13.9 Manifest Schema

See the manifest YAML specification in the constitution for the full schema. Key fields per module: `file`, `domain`, `maintains`, `interfaces`, `implements`, `adjacent`, `deep_context`, `negative_specs`. The `invariant_registry` maps every invariant to its owner, domain, and description.

### 0.13.10 Assembly Rules

**Three-tier**: Tier 1 + domain Tier 2 + Tier 3 (if exists) + module. Cross-cutting modules get ALL domain constitutions.
**Two-tier**: Tier 1 + module.
Both validate budget compliance (INV-014): ERROR if > ceiling, WARN if > target.

### 0.13.11 Consistency Validation

Nine mechanical checks:

- **CHECK-1**: Invariant ownership — each invariant maintained by exactly one module (INV-013)
- **CHECK-2**: Interface consistency — interfaced invariants are maintained elsewhere or system-owned
- **CHECK-3**: Adjacency symmetry — if A lists B as adjacent, B lists A
- **CHECK-4**: Domain membership — module's maintained invariants are in module's domain
- **CHECK-5**: Budget compliance — assembled bundles within ceiling (INV-014)
- **CHECK-6**: No orphan invariants — every invariant maintained or interfaced by some module
- **CHECK-7**: Cross-module isolation — no direct module-to-module references (INV-012)
- **CHECK-8**: Deep context correctness — cross-domain interfaces have deep context files (three-tier only)
- **CHECK-9**: File existence — all manifest paths exist; all module files are in manifest (INV-016)

### 0.13.12 Cascade Protocol

When constitutional content changes, affected modules must be re-validated.

| Change | Blast Radius |
|---|---|
| Invariant wording changed | Modules maintaining or interfacing |
| ADR superseded | Modules implementing that ADR |
| New invariant added | Module assigned as owner |
| Shared type changed | Same-domain + cross-domain users |
| Non-negotiable changed | ALL modules |
| Glossary term redefined | All modules using that term |

### 0.13.13 Modularization Quality Gates

**Gate M-1**: All nine checks pass with zero errors.
**Gate M-2**: All bundles under ceiling; < 20% exceed target.
**Gate M-3**: LLM bundle sufficiency — zero questions requiring other module's content.
**Gate M-4**: Declaration-definition faithfulness (INV-015).
**Gate M-5**: Cascade simulation — simulated invariant change correctly identifies affected modules.

### 0.13.14 Migration: Monolith to Modular

1. Identify domains (2–5 based on architecture)
2. Extract system constitution (Tier 1)
3. Extract domain constitutions (Tier 2)
4. Extract modules with headers; convert direct cross-refs to constitutional refs
5. Create cross-domain deep context files (Tier 3)
6. Build manifest
7. Validate (all 9 checks)
8. Extract end-to-end trace as cross-cutting module
9. LLM validation on 2+ bundles

## Modularization Invariants

**INV-011: Module Completeness** — An LLM receiving a bundle can implement the module's subsystem without information from any other module.
Violation: Module references another module's internal data structure not in the constitution.

**INV-012: Cross-Module Isolation** — Modules reference each other only through constitutional elements.
Violation: "Use the same batching strategy as EventStore's flush_batch()."

**INV-013: Invariant Ownership Uniqueness** — Every invariant maintained by exactly one module.
Violation: Two modules both list APP-INV-017 in maintains.

**INV-014: Bundle Budget Compliance** — Every assembled bundle fits within hard ceiling.
Violation: Module + constitution exceeds 5,000 lines.

**INV-015: Declaration-Definition Consistency** — Tier 1 declarations faithfully summarize Tier 2 definitions.
Violation: Declaration says "append-only" but definition now allows compaction.

**INV-016: Manifest-Spec Synchronization** — Manifest reflects actual file state.
Violation: New module file exists but isn't in manifest.

## Modularization ADRs

### ADR-006: Tiered Constitution over Flat Root
**Decision**: Three-tier as full protocol, two-tier as blessed simplification for small specs (< 20 invariants, ≤ 400-line system constitution). `tier_mode` field selects.
// WHY NOT flat root? At scale, it consumes 30-37% of context budget before the module starts.

### ADR-007: Cross-Module References Through Constitution Only
**Decision**: Module A references invariants in the constitution, never Module B's internal sections. (Enforced by INV-012.)
// WHY NOT direct references? Breaks INV-011 — Module A's bundle would need Module B's implementation.
