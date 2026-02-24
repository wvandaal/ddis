---
module: drift-management
domain: drift
tier: 2
description: >
  Drift Management module — formalizes spec-implementation divergence
  as a first-class DDIS concept with mechanical detection, measurement,
  and remediation.
ddis_version: "3.0"
tier_mode: two-tier
---

# Drift Management Module

Spec-implementation drift is not a bug to be avoided — it is the natural state of any living system. Code evolves faster than prose. Intent shifts during conversation. New requirements emerge from implementation experience. This module formalizes drift as a first-class DDIS concept: measurable, classifiable, and mechanically reducible to zero.

**Invariants referenced from other modules (INV-018 compliance):**
- INV-003: Every invariant can be violated by a concrete scenario and detected by a named test (maintained by core-standard module)
- INV-006: The specification contains a cross-reference web where no section is an island (maintained by core-standard module)
- INV-007: Every section earns its place by serving at least one other section or preventing a named failure mode (maintained by core-standard module)
- INV-015: Every invariant declaration is a faithful summary of its full definition (maintained by modularization module)
- INV-016: The manifest accurately reflects the current state of all spec files (maintained by modularization module)

---

## §D.1 The Problem

The DDIS CLI started as 13 commands. The specification described 13 commands, 16 invariants, and 11 ADRs. Then implementation happened.

```
Before (spec):  13 commands, 16 invariants, 11 ADRs
After (code):   23 commands, 28 invariants, 15 ADRs
Divergence:     10 commands, 12 invariants, 4 ADRs existed only in code

Detection:  $ ddis validate index.db --json → Check 1: 98 unresolved cross-refs
```

Ten commands — `coverage`, `skeleton`, `checkpoint`, `checklist`, `cascade`, `bundle`, `impl-order`, `progress`, `state`, `exemplar` — existed only in implementation. No spec section described them. No invariant governed them. No ADR justified their design choices. They worked, but their correctness was accidental: an engineer happened to make good decisions, not because the spec constrained bad ones.

This is the common case. Every project that iterates on implementation eventually outpaces its spec. The usual response — "we should keep the spec updated" — is an aspiration, not a mechanism. DDIS requires mechanisms (INV-003).

**The meta-failure.** This very plan to formalize drift exhibited drift. The human's goal was "practical tool for agent collaboration." The AI's first draft became a category theory paper. This IS intent-specification drift — the exact problem DDIS exists to solve. The three-tier structure is the bridge:

```
Human intent → System Constitution (semi-formal) → Module (impl-ready)
Each tier is a translation step. Drift can occur at any step.
```

Drift is not just a diagnostic. It is **the collaboration protocol**: `drift = 0` is the mechanical definition of "done." The spec IS the test suite for human-AI alignment.

---

## §D.2 What Is Drift

### Three Representations, Two Gaps

```
Intent ←— gap₁ —→ Specification ←— gap₂ —→ Implementation
```

**gap₂ (implementation drift)** is fully mechanical. Compare the spec index against declared implementation elements:

```
impl_drift(t) = |unspecified| + |unimplemented| + 2·|contradictions|
```

- `unspecified`: elements in code with no spec section (depth drift)
- `unimplemented`: elements in spec with no implementation (correctness drift)
- `contradictions`: spec says X, code does Y (weighted 2x — both sides are wrong)

**gap₁ (intent drift)** is semi-mechanical. Non-negotiable coverage can be measured; undeclared goals cannot:

```
intent_drift(t) = |uncovered_nonnegotiables| + |purposeless_elements|
```

```
total_drift = intent_drift + impl_drift
```

### Quality-Dimension Decomposition

The scalar drift count tells you *how much* drift exists. The quality decomposition tells you *what kind* and guides remediation:

| Dimension | Measures | Components | Remediation |
|-----------|----------|------------|-------------|
| **Correctness** | Spec says X, code violates X | unimplemented + contradictions | Fix implementation |
| **Depth** | Code does X, spec silent on X | unspecified | Formalize into spec |
| **Coherence** | Spec internally inconsistent | orphan cross-refs, declaration-definition gaps | Repair spec structure |

High correctness drift with low coherence drift needs implementation work. High coherence drift with low correctness drift needs spec maintenance. The scalar `impl_drift` conflates these — the decomposition makes the drift report diagnostic.

### Drift as Stopping Criterion

The agent iterates until `drift = 0`, replacing subjective "does this look right?" with mechanical verification. Drift as quality metric: compare agents by drift scores on the same task. Drift as collaboration contract: human writes spec, agent reduces drift, both share a mechanical definition of "done."

**Honest limitation:** Undeclared goals require human input. This is inherent — no tool can detect what was never stated.

---

## §D.3 Failure Taxonomy

Three orthogonal classification dimensions. Each drift instance is classified along all three:

**Direction** — who is ahead?

| Direction | Definition | Example |
|-----------|-----------|---------|
| impl-ahead | Code has elements the spec doesn't describe | 10 undocumented CLI commands |
| spec-ahead | Spec describes elements the code doesn't implement | Spec prescribes distributed mode; code is single-node |
| contradictory | Spec and code disagree on behavior | Spec says append-only; code has DELETE |
| mutual | Both sides have unmatched elements | Some commands undocumented AND some spec features unbuilt |

**Severity** — how bad is it?

| Severity | Definition | Detection |
|----------|-----------|-----------|
| additive | New elements, no conflicts | `unspecified > 0 ∨ unimplemented > 0`, `contradictions = 0` |
| contradictory | Active conflicts between spec and code | `contradictions > 0` |
| structural | Spec's internal structure is broken | Orphan cross-refs, broken causal chains (INV-001, INV-006) |

**Intentionality** — was this planned?

| Intentionality | Definition | Source |
|---------------|-----------|--------|
| planned | Tracked in the planned divergence registry | `planned_divergences > 0` |
| organic | Accumulated through normal development | No registry entry, no accident |
| accidental | Result of a mistake or oversight | Contradiction or regression |

**Confidence note:** Classification is display-only in v1. The quality breakdown (§D.2) drives remediation. If empirical use shows classification changes agent behavior, promote to first-class in v2.

---

## §D.4 Invariants

Three load-bearing invariants, reduced from eight via the constraint removal test (INV-007). Empirically, k* for analytical tasks is 2–5 constraints — the same principle behind DDIS's own INV-007 and Gestalt Theory's overprompting threshold. Each gets the full six-component DDIS definition.

---

**INV-021: Drift Detection Completeness**

*Every form of drift is detectable by at least one mechanical check. Both drift measures are independently observable.*

```
∀ drift_instance ∈ {unspecified, unimplemented, contradiction}:
  ∃ check ∈ validation_suite: check.detects(drift_instance)
Corollary: impl_drift(t) and intent_drift(t) are computable from
Spec(t) and Impl(t) alone (impl_drift fully mechanical;
intent_drift semi-mechanical).
```

Violation scenario: A new command type is added to the code but no validation check looks for undeclared commands — the drift instance goes undetected. The spec reports `drift = 0` while 10 commands exist only in code.

Validation: Enumerate all element types in the DDIS schema (commands, invariants, ADRs, sections). For each, produce a synthetic drift instance. Assert at least one check flags it.

// WHY THIS MATTERS: If drift is invisible, it accumulates silently until the spec is useless. Detection completeness ensures every form of divergence has a mechanical alarm.

Falsifiable: Construct a drift instance for each element type; if any goes undetected, the invariant is violated.

---

**INV-022: Reconciliation Monotonicity and Soundness**

*A reconciliation step can only reduce drift and must preserve existing valid correspondences.*

```
∀ reconciliation_step r:
  impl_drift(after(r)) ≤ impl_drift(before(r))              — monotonicity
  ∧ ∀ valid_correspondence (s,i) before r: (s,i) valid after r  — soundness
```

Violation scenario: (Monotonicity) Adding a spec entry for command X introduces a cross-reference to non-existent INV-Y. Drift decreased by 1 (X is now specified) but increased by 1 (INV-Y is now an unresolved reference). Net change: zero or worse. (Soundness) Editing a module to document command X accidentally changes the statement of an existing invariant. The old valid correspondence between spec and implementation is now broken.

Agent rule: run `ddis drift` after every edit. If drift increased, undo.

Validation: Apply a reconciliation step (e.g., add one spec entry). Measure drift before and after. Assert `drift(after) ≤ drift(before)`. Assert all previously valid correspondences remain valid.

// WHY THIS MATTERS: Without monotonicity, reconciliation can oscillate — fixing one thing breaks another. Without soundness, reconciliation is destructive. Together they guarantee convergence.

Falsifiable: Construct a reconciliation step that increases drift or invalidates an existing correspondence. If either occurs and the invariant doesn't detect it, INV-022 is violated.

---

**INV-023: Brownfield Convergence**

*From total drift (no spec, existing implementation), a constructive procedure monotonically reduces drift to zero.*

```
Given Impl(0) ≠ ∅ and Spec(0) = ∅:
  ∃ constructive procedure P:
    ∀ k: drift(P(k+1)) < drift(P(k)) until drift(P(N)) = 0
```

Realized by: `skeleton` generates a spec scaffold from the implementation → RALPH loop iteratively refines → `drift` measures progress at each step. The procedure is constructive: each step is a concrete CLI command, not an abstract instruction.

Violation scenario: A codebase with 50 commands is given to `skeleton`, which generates a spec scaffold. After 3 RALPH iterations, drift plateaus at 12 — some element types have no detection mechanism, so they can never be resolved. INV-021 (detection completeness) was violated first, causing INV-023 to fail.

Validation: Apply `skeleton` + 3 RALPH iterations to a test codebase. Assert drift strictly decreases at each step.

// WHY THIS MATTERS: Without brownfield convergence, DDIS only works for greenfield projects. Most real systems already have code. INV-023 guarantees DDIS is useful for existing codebases — not just new ones.

Falsifiable: Apply the procedure to a non-trivial codebase. If drift does not decrease at every step, the invariant is violated.

---

**Design Goal (not invariant): Intent Preservation** — demoted from invariant because undeclared goals cannot be mechanically detected (violates INV-003 falsifiability). Intent drift measurement (§D.2) is the best available approximation, but it is honest about its semi-mechanical nature.

---

## §D.5 The drift Command

### Demonstration

**Report mode** — the full summary:

```
$ ddis drift /tmp/cli-spec.db --report

Drift Report
═══════════════════════════════════════════════
  Implementation drift:  10 (10 unspecified, 0 unimplemented, 0 contradictions)
  Intent drift:           1 (1 uncovered non-negotiable)
  Planned divergences:    0
  ─────────────────────────────────────────────
  Effective drift:       11

  Quality breakdown:
    Correctness:  0   (nothing violates the spec)
    Depth:       10   (code outpaced the spec — formalize these)
    Coherence:    1   (1 cross-ref gap)

  Direction:     impl-ahead    Severity: additive    Intentionality: organic

  Top unspecified: coverage, skeleton, checkpoint, checklist, cascade, bundle...
  Recommendation: Depth drift dominant — run `ddis progress` to plan spec formalization.
```

**Default mode** — the autonomous workflow engine:

```
$ ddis drift /tmp/cli-spec.db

Next: Formalize `coverage` command (depth drift, priority 1/10)
═══════════════════════════════════════════════════════════════

Context (from ddis context):
  Depends on: INV-003 (falsifiability), INV-007 (signal-to-noise)
  Feeds into: INV-015 (coverage completeness), ADR-008 (woven provisions)
  Impact radius: 4 elements within 2 hops

Exemplar (from ddis exemplar):
  Best pattern: INV-015 in core-standard.md (quality: 0.87)
  Components present: statement +  semi-formal +  violation +  validation +  why +

Guidance:
  1. Add `coverage` to CLI command registry in element-specifications
  2. Write invariant following INV-015 pattern (all 5 components)
  3. Add cross-refs to INV-003 and INV-007
  4. After writing: ddis parse && ddis drift (expect drift: 11 -> 10)
```

Most diagnostic tools stop at measurement. `ddis drift` (no flags) generates a complete remediation package by composing existing commands: `progress` picks the item, `context` gathers neighbors, `exemplar` finds demonstrations. The agent's workflow collapses to a loop:

```
loop:
  report = ddis drift index.db
  if report.drift == 0: done
  apply report.remediation
  ddis parse manifest.yaml -o index.db && ddis drift index.db  # verify monotonicity (INV-022)
```

### State Transition

```
T_drift: Index → DriftReport

Input:  Parsed spec index (SQLite DB from ddis parse)
Output: DriftReport { impl_drift, intent_drift, planned_divergences,
                      effective_drift, quality_breakdown, classification }
```

The `--report` flag returns the full summary. The `--json` flag returns machine-readable output. Without flags, `drift` calls `Remediate` internally to produce the next actionable package.

**Implementation:** Thin orchestration of existing APIs — `progress.Analyze()` picks the item, `search.BuildContext()` gathers neighbors, `exemplar.Analyze()` finds demonstrations. All composable from Go; no new infrastructure needed.

---

## §D.6 Brownfield Entry

Most real systems have code before they have a spec. Brownfield entry handles the case where `Impl(0) ≠ ∅` and `Spec(0) = ∅`:

```
1. Measure:  $ ddis drift index.db --report     → total drift (everything is unspecified)
2. Scaffold: $ ddis skeleton index.db            → generate spec scaffold from code
3. Refine:   $ ddis_ralph_loop.sh                → iteratively reduce drift
4. Verify:   $ ddis drift index.db --report      → drift = 0
```

Each RALPH iteration applies: audit (drift + validate + coverage), apply (exemplar + context + bundle), judge (diff + drift + coverage). Drift must decrease at each iteration (INV-022).

### Planned Divergence Registry

Not all drift is bad. Some divergence is intentional — roadmap items, deferred features, conscious tech debt. The planned divergence registry tracks these:

```yaml
planned_divergences:
  - element: "distributed-mode"
    type: spec-ahead
    reason: "Roadmap item, implementation planned for v2"
    expiry: "2026-06-01"

effective_drift = total_drift - |planned_divergences|
```

Every planned divergence requires a reason and an expiry. Expired entries become unplanned drift — forcing a decision: implement, re-defer with justification, or remove from spec.

**Storage:** Planned divergences are stored in the session state KV store (via `ddis state`), not in the spec itself. This is operational state, not spec content — it must survive `ddis parse` without being lost (ADR-013).

---

## §D.7 Agent Quickstart

The definitive workflow for agents working on DDIS specifications. Six steps, one rule.

```
# 1. SEED: Capture starting point
$ ddis seed index.db

# 2. ORIENT: Understand before writing
$ ddis drift index.db                       # baseline measurement
$ ddis context index.db INV-003             # what touches this?
$ ddis exemplar index.db INV-001            # what does good look like?

# 3. AUTHOR: Formalize intent into spec
#    After EACH edit:
$ ddis parse manifest.yaml -o idx.db && ddis drift idx.db
#    Rule: drift MUST NOT increase (INV-022). If it did, undo.

# 4. VERIFY: Confirm intent captured
$ ddis validate index.db --json
$ ddis checkpoint index.db
$ ddis checklist index.db

# 5. PLAN: Decompose into tasks
$ ddis progress index.db --json             # what's ready?
$ ddis impl-order index.db --json           # in what sequence?
#    → Feed into beads (br) for task management

# 6. TRACK: After implementation
$ ddis drift index.db                       # did code stay aligned?
```

**The one rule: never increase drift.**

Every edit either reduces drift (good), maintains it (acceptable if working toward a multi-step reduction), or increases it (undo immediately). This is INV-022 applied to the developer workflow. The spec writes itself forward, guided by its own incompleteness.

---

## §D.8 CLI Command Mapping

All 23 commands organized by drift management role:

| Role | Commands | Gap Level |
|------|----------|-----------|
| **Detection** | `drift` (NEW), `validate`, `coverage`, `diff`, `checkpoint` | Both gaps |
| **Analysis** | `impact`, `cascade`, `impl-order`, `progress`, `checklist` | Spec-Impl |
| **Reconciliation** | `skeleton`, `exemplar`, `context`, `bundle` | Impl→Spec |
| **Infrastructure** | `parse`, `render`, `query`, `search`, `log`, `tx`, `seed`, `state` | Foundation |

### RALPH Integration

The RALPH loop (§D.6) maps directly to drift roles:

| RALPH Phase | Drift Role | Commands |
|-------------|-----------|----------|
| **Audit** | Detection | `drift` + `validate` + `coverage` |
| **Apply** | Reconciliation | `exemplar` + `context` + `bundle` |
| **Judge** | Detection | `diff` + `drift` + `coverage` |
| **Planning** | Analysis | `impl-order` + `cascade` + `impact` + `progress` |

Every command has a drift management role. No command is decorative (INV-007).

---

## §D.9 Spec-Grounded Conversation

The `--intent` flag measures non-negotiable coverage post-hoc. But intent drift happens in real time during conversation. The real mechanism: agents use CLI tools to triangulate between what the human says and what the spec already contains.

### The Spec as Shared Reference Frame

During conversation, the agent queries the existing spec to ground the human's statements. Three interaction patterns:

**ALIGNMENT** — Human states something the spec already captures:
```
Human: "Validation should reject specs with orphaned sections."
Agent: *runs ddis query INV-007*
       → "Signal-to-Noise Ratio — every section earns its place"
Agent: "INV-007 already covers this. Extend its validation, or new requirement?"
```

**DIVERGENCE** — Human states something absent from the spec:
```
Human: "I want to add distributed mode to DDIS."
Agent: *runs ddis search "distributed"* → zero hits
Agent: "This doesn't appear in the spec or any declared goals.
        New direction, or should it connect to something existing?"
```

**CONTRADICTION** — Human states something that conflicts with the spec:
```
Human: "Let's make the oplog mutable so we can fix entries."
Agent: *runs ddis query ADR-003* → "append-only operation log"
Agent: "ADR-003 decided on append-only. Supersede it, or work within it?"
```

**No new tooling needed.** `search`, `query`, `context` already exist. The missing piece is workflow guidance — teaching agents to query the spec during conversation. This is what the Agent Quickstart (§D.7) and the `ddis-drift-workflow` skill provide.

### What This Narrows

The existing spec + implementation constrain the space of plausible intent. The agent does not just listen — it checks what the human says against the formal record, surfacing alignment, divergence, and contradiction as the conversation happens. This is early detection, not post-hoc measurement.

---

## §D.10 Architecture Decision Records

---

### ADR-012: Drift as First-Class Concept

#### Problem

How should spec-implementation divergence be managed? Every specification that lives long enough diverges from its implementation. Should this be addressed by social convention or formal mechanism?

#### Options

A) **Informal guidelines** — "Keep the spec updated" as a team norm.
- Pros: Zero overhead. No new concepts. Works for small teams with strong discipline.
- Cons: No detection mechanism. No measurement. No stopping criterion. "Keep it updated" is the software equivalent of "be careful" — unfalsifiable, unenforceable. Fails exactly when needed most: under deadline pressure.

B) **Formal drift measure with mechanical detection** — Drift is a number. It is computed mechanically. Every form of divergence maps to a computable quantity. The spec is "done" when `drift = 0`.
- Pros: Measurable. Falsifiable. Automatable. Provides a stopping criterion for both humans and agents.
- Cons: Requires tooling. Adds concepts to the spec vocabulary. Some forms of drift (intent) are only semi-mechanical.

#### Decision

**Option B: Formal drift measure.** A spec that cannot detect its own divergence from implementation is a spec waiting to become shelfware. The overhead of formal drift tracking is paid once; the cost of undetected drift compounds indefinitely.

// WHY NOT Option A? "Keep the spec updated" has been the default approach since specifications existed. It fails at scale, under pressure, and across team boundaries — precisely the conditions where specs are most needed. The 13→23 command divergence (§D.1) is the empirical proof.

#### Consequences

- Every DDIS spec gains three invariants (INV-021, INV-022, INV-023)
- Every agent workflow gains a mechanical "done" criterion
- `drift = 0` replaces subjective "looks good" as the convergence test

#### Tests

- (Validated by INV-021) Every form of drift is mechanically detectable.
- (Validated by INV-022) Reconciliation monotonically reduces drift.

---

### ADR-013: Planned Divergence Registry

#### Problem

Should all drift be treated as a problem to fix, or should some divergence be explicitly planned and tracked?

#### Options

A) **Zero tolerance** — All drift must be resolved. No exceptions.
- Pros: Simple rule. No bookkeeping. Forces immediate resolution.
- Cons: Roadmap items, deferred features, and conscious tech debt exist in every real project. Zero tolerance either forces premature implementation or forces removing spec content that describes future intent.

B) **Tracked planned divergence with expiry** — Divergences can be declared planned with a reason and an expiry date. Planned divergences are subtracted from effective drift.
- Pros: Acknowledges reality. Prevents drift score from penalizing intentional decisions. Expiry forces periodic reassessment.
- Cons: Risk of "planned divergence" becoming a dumping ground for undone work. Requires storage and tracking.

#### Decision

**Option B: Tracked planned divergence with expiry.** The `effective_drift` formula (§D.6) subtracts planned divergences. Every entry requires a reason (why this is intentional) and an expiry (when it must be resolved or re-justified). Expired entries become unplanned drift automatically.

// WHY NOT Option A? Zero tolerance sounds rigorous but produces perverse incentives: teams remove future-facing spec content to achieve `drift = 0`, losing the spec's roadmap value. Planned divergence acknowledges that a spec describes intent, not just current state.

#### Consequences

- Divergence registry stored in session state (survives parse cycles)
- Every planned divergence has an expiry date — no permanent exemptions
- Effective drift remains the primary metric; total drift remains observable

#### Tests

- (Validated by INV-023) Planned divergences with expired dates are treated as unplanned drift in the convergence procedure.

---

### ADR-014: Brownfield via Skeleton + RALPH

#### Problem

How should DDIS handle existing codebases that have no spec? Writing a spec from scratch for a large existing system is expensive and error-prone.

#### Options

A) **Manual spec-from-scratch** — Author writes the entire spec by hand, informed by the code.
- Pros: Maximum quality. Author deeply understands the system by writing about it.
- Cons: Prohibitively expensive for large codebases. High barrier to DDIS adoption. Spec-code correspondence must be maintained manually during the entire authoring period.

B) **Automated skeleton bootstrap + iterative RALPH refinement** — `ddis skeleton` generates a spec scaffold from parsed implementation. The RALPH loop iteratively refines it. `ddis drift` measures progress.
- Pros: Low barrier to entry. Mechanical scaffold ensures nothing is missed. Each RALPH iteration is auditable. Drift monotonically decreases (INV-022). Converges to zero (INV-023).
- Cons: Initial scaffold quality is low — it contains structure but not insight. Requires RALPH loop infrastructure.

#### Decision

**Option B: Skeleton + RALPH.** The scaffold gets the structure right (completeness). The RALPH loop adds insight (quality). The drift measure tracks progress (convergence). Combined, they guarantee that any codebase can reach `drift = 0` without requiring the author to hold the entire system in their head at once.

// WHY NOT Option A? The 13→23 divergence (§D.1) happened despite disciplined authors. Manual spec-from-scratch at scale is a myth — what actually happens is partial specs that never cover the full system. Automated bootstrap ensures coverage; iterative refinement ensures quality.

#### Consequences

- Every existing codebase has a path to full DDIS conformance
- `skeleton` + RALPH is the standard adoption procedure for brownfield projects
- Initial spec quality is intentionally low — the RALPH loop handles quality
- Drift must decrease at every iteration (INV-022), providing a mechanical progress measure

#### Tests

- (Validated by INV-023) Apply skeleton + 3 RALPH iterations to a test codebase; assert drift decreases at each step.
- (Validated by INV-022) No RALPH iteration increases drift.

---

## §D.11 Formal Foundations

### The Gestalt Theory Connection

A specification is a field configuration over the implementation space, just as a prompt is a field configuration over the model's activation space. Four genuine correspondences drive four design choices:

| Gestalt Concept | DDIS Realization | What It Explains |
|----------------|-----------------|-----------------|
| Structure > content | Cross-reference web (INV-006), causal chain (INV-001), document ordering | Why spec *shape* matters more than volume |
| Constraint removal test (k*) | INV-007 + modularization at 2,500 lines + reasoning_reserve | Why shorter specs with fewer invariants produce better implementations |
| Demonstration > constraint | `ddis exemplar` + worked examples vs negative specs | Why showing one good invariant beats listing ten rules |
| Spec-first framing | DDIS existence: formalize before implementing | Why the three-tier architecture works |

### Where the Analogy Breaks

Honesty about the limits:

- The spec is **observable** (unlike the model's internal field configuration). DDIS partially solves the observability gap that Gestalt Theory identifies.
- The conversation-spec boundary is a **feedback loop** (unlike prompt→output, which is feedforward). The agent can query the spec, update it, and re-query.
- DoF separation applies to **sections within** a spec, not to the spec as a whole. A spec is not a prompt.
- k* is a **consumer limitation** while INV-007 is an **artifact quality property**. Different mechanism, same structural principle.

The correspondences are functional, not mechanistic. DDIS does not validate Gestalt Theory — it applies the same structural insights at document-architecture scale.

### Adjunction Model (Optional Formal Treatment)

For those who find it clarifying:

```
α: Implementation → Specification     (abstraction: code → spec)
γ: Specification → Implementation     (concretization: spec → code)

drift = d(S, α(I))    — distance between the current spec and the spec
                         that would perfectly describe the current code

RALPH convergence: S₀ ⊑ S₁ ⊑ ... ⊑ Sₙ  (ascending chain in spec lattice)
Self-bootstrapping: α(γ(S)) ≈ S         (round-trip property)
```

Intent as informal functor: goals map to spec elements, but goals do not form a lattice — they are partially ordered at best. This is why intent drift is semi-mechanical (§D.2).

---

## §D.12 Negative Specifications

- **Must NOT** treat "keep spec updated" as sufficient drift management — it is an aspiration, not a mechanism (ADR-012)
- **Must NOT** allow planned divergence without expiry or justification — every entry requires both (ADR-013)
- **Must NOT** lead with theory over worked examples — demonstrations before constraints, always (INV-007, §D.5)
- **Must NOT** claim mechanical enforcement where human judgment is required — intent drift is explicitly semi-mechanical (§D.2)
- **Must NOT** claim the CLI captures all intent — it mediates formalization of declared intent; undeclared goals require human input

---

## §D.13 Verification Prompts

After implementing or reviewing drift management functionality, verify:

1. [ ] Every form of drift (unspecified, unimplemented, contradiction) has at least one mechanical detection check (INV-021)
2. [ ] A reconciliation step never increases drift and never invalidates existing valid correspondences (INV-022)
3. [ ] The brownfield procedure (skeleton + RALPH) produces strictly decreasing drift at each iteration (INV-023)
4. [ ] The quality breakdown (correctness, depth, coherence) correctly classifies each drift item and guides remediation (§D.2)
5. [ ] The planned divergence registry requires reason AND expiry for every entry — no permanent exemptions (ADR-013, §D.6)
6. [ ] The drift command demonstration appears BEFORE the schema/state-transition definition (§D.5, INV-007)
7. [ ] The Agent Quickstart (§D.7) is self-contained: an agent can follow it without reading the rest of this module
8. [ ] Negative specifications (§D.12) address the five most likely misuse patterns
9. [ ] *Integration*: INV-021, INV-022, INV-023 are registered in the system constitution (§0.5) and manifest (INV-016)
10. [ ] *Integration*: ADR-012, ADR-013, ADR-014 are registered in the system constitution (§0.6) and manifest (INV-016)
