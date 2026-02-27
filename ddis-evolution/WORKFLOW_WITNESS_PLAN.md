# The Workflow Witness: Process Compliance Through Information Flow

> Design document for the single most accretive addition to the DDIS plan.
> Evolved through 6 `ddis discover` invocations on thread `t-1772030865583`.
> Implementation tracked on thread `t-1772160895968`.
> Grounded in three theoretical frameworks: Alien Artifact Methodology,
> LLM Gestalt Theory, and Skill Field Dynamics.
>
> **ID Mapping** (corrected from original — prior session claimed IDs):
> - APP-INV-042 → **APP-INV-056** (Process Compliance Observability)
> - APP-ADR-031 → **APP-ADR-043** (Observational Process Compliance over Prescriptive Gates)
> - Owner: **auto-prompting** (observation patterns domain, not lifecycle-ops)
> - Check: **18** (next after Check 17: Challenge Freshness)

---

## 1. The Problem: Methodology Failure as Information Architecture Gap

During the implementation of Invariant Witnesses (APP-INV-041, APP-ADR-030),
we violated the bilateral lifecycle:

- **Spec-first violation**: Wrote code (Phases 2-5) before spec (Phase 1)
- **Zero tool intermediation**: Never used `ddis discover`, `ddis context`,
  `ddis refine`, or `ddis absorb` during the entire implementation
- **No baseline measurement**: Skipped Phase 0 entirely
- **Result**: B- grade (78/100) — implementation works, but methodology was
  not followed

This is not a one-off failure. It is a **structural failure mode** that will
recur every time an agent receives a detailed plan and defaults to the
implementation substrate.

---

## 2. Theoretical Grounding (Three Lenses)

### Lens 1: Alien Artifact Methodology — Phase Ordering

The alien artifact methodology defines a formal system `M = (D, F, I, S, C, V)`
with five strict transitions:

```
FORMALIZE: (D,∅,∅,∅,∅,∅) → (D,F,∅,∅,∅,∅)     very high DoF
DERIVE:    (D,F,∅,∅,∅,∅) → (D,F,I,∅,∅,∅)      high DoF
SPECIFY:   (D,F,I,∅,∅,∅) → (D,F,I,S,∅,∅)      low DoF
IMPLEMENT: (D,F,I,S,∅,∅) → (D,F,I,S,C,∅)      very low DoF
VERIFY:    (D,F,I,S,C,∅) → (D,F,I,S,C,V)      low DoF
```

The bilateral lifecycle maps to this pipeline:

- `ddis discover` = FORMALIZE (explore domain, high DoF)
- `ddis refine` = DERIVE + SPECIFY (crystallize invariants, reduce DoF)
- code implementation = IMPLEMENT (very low DoF — mechanize the spec)
- `ddis drift` + `ddis witness` = VERIFY (check spec-code alignment)
- `ddis absorb` = the inverse — code speaks back to spec

The fundamental invariant is **INV-METHODOLOGY-MONOTONE**: no transition
destroys prior-phase information. Our failure was jumping from FORMALIZE
(plan) directly to IMPLEMENT (code), skipping DERIVE and SPECIFY entirely.
This violates the precondition: "IMPLEMENT requires S converged."

The methodology explicitly identifies this as **Pitfall 1: Implementation-first
retro-fitting** — "Writing code then deriving invariants. The invariants become
descriptive (what the code does) rather than prescriptive (what the system
guarantees)." And **Pitfall 3: Spec-as-decoration** — "Writing a spec after the
implementation to satisfy process."

### Lens 2: LLM Gestalt Theory — The Mid-DoF Saddle

The agent received a detailed plan (Phases 0-7). This puts it in the **mid-DoF
saddle zone** — half-specified (plan exists, ordering is written down) but with
freedom to reorder execution. Mid-DoF is the worst of both worlds: "too
constrained for reasoning, too vague for reliable execution. Output becomes
hedged and generic."

The fix prescribed by Gestalt Theory: **separate exploration from execution**.
The plan's Phase 1 (spec changes) is a *different cognitive task* from Phase 2-5
(code implementation). They require different DoF, different reasoning modes,
different substrates. Loading them simultaneously — as a flat list of phases to
execute — creates the mid-DoF saddle where the agent defaults to the substrate
it's most comfortable with (implementation) and defers the one it finds harder
(spec writing).

The overprompting theorem (k*) also applies: as conversation depth increases,
the plan's ordering instructions attenuate. By the time the agent reaches
Phase 3, the k* budget for "remember to do spec before code" has dropped to
near-zero. The agent is deep in implementation substrate and the spec-first
ordering constraint has been overwhelmed by implementation-specific context.

### Lens 3: Skill Field Dynamics — Workflow as Skill Composition

The DDIS workflow tools (discover, refine, absorb, drift) are **skills that
should be sequenced across cognitive phases**, following the composition protocol
from skill field dynamics:

| Cognitive Phase | DDIS Tool | DoF | Reasoning Mode |
|----------------|-----------|-----|---------------|
| Discovery | `ddis discover` | Very high | Formal/Exploratory |
| Refinement | `ddis refine` | High → Low | Formal → Practical |
| Implementation | code + `ddis scan` | Very low | Practical |
| Verification | `ddis witness` + `ddis validate` | Low | Meta-reflective |
| Reconciliation | `ddis absorb` + `ddis drift` | Low → High | Practical → Meta |

The destructive interference diagnosis from skill field dynamics applies
exactly: loading `discover` (high DoF, "explore the space") and `implement`
(very low DoF, "write the code") simultaneously creates the skill-level
mid-DoF saddle. The agent hedges between exploration and execution.

The composition protocol's fix: sequence skills across phases, shed activation
after absorption, never stack conflicting DoF targets.

### Convergence of All Three Frameworks

All three frameworks converge on the same truth:

> **The methodology failure is not a discipline problem. It is an information
> architecture problem.**

The agent doesn't fail because it lacks willpower to follow the plan. It fails
because:

1. The plan's ordering instructions attenuate with conversation depth (k* decay)
2. The mid-DoF saddle pulls toward the substrate of least resistance (code)
3. No feedback signal fires when the ordering is violated

The solution is not enforcement (blocking, gating, requiring). The solution is
**information flow** — making process compliance visible through the tools the
agent is already using, so the methodology enforces itself through the existing
state monad.

---

## 3. The Core Insight

The existing DDIS state space already records everything needed to detect
methodology violations:

- **OpLog**: append-only record of WHAT commands were run and WHEN
- **Git log**: temporal ordering of which files changed in which commits
- **Witness table**: which invariants have valid proof receipts
- **Validation records**: when `ddis validate` was run and what it found

Nobody analyzes the temporal pattern of these records to detect methodology
violations. The OpLog IS the process witness. We just need to read it.

This means the solution is **not a new tool**. It is a **new analysis over
existing data**, delivered through existing information channels.

---

## 4. Formal State Space Extension

Extend the DDIS state space from 8-tuple to 9-tuple:

```
S = (SpecFiles, Index, SearchState, OpLog, TxState, EventStreams,
     DiscoveryState, Workspace, ProcessState)

where:
  ProcessState = {
    phase_ordering:    Seq(Phase)             -- temporal sequence of phases observed
    spec_mutations:    Seq(Timestamp * Path)  -- when spec files changed (git or oplog)
    code_mutations:    Seq(Timestamp * Path)  -- when code files changed (git or oplog)
    tool_invocations:  Seq(Timestamp * Cmd)   -- which DDIS commands were run (oplog)
    validation_points: Seq(Timestamp * Result) -- when ddis validate was run
    compliance_score:  [0.0, 1.0]            -- composite process compliance
  }
```

---

## 5. The Process Compliance Score

```
PC(feature) = w₁·R_spec + w₂·R_tool + w₃·R_witness + w₄·R_validate

where:
  R_spec = |{inv : spec_commit(inv) < code_commit(inv)}| / |modified_invariants|
    -- fraction of invariants where spec changed BEFORE code
    -- git log analysis: compare commit timestamps of spec files vs code files
    -- for each invariant modified in a feature branch, check ordering
    -- degrades to 0.5 when no git available (neutral, not penalizing)

  R_tool = |{cmd ∈ {discover, refine, context, absorb} : cmd ∈ oplog}| / expected_count
    -- fraction of expected auto-prompting commands actually used
    -- expected_count estimated from change scope (1 per modified module)
    -- degrades to 0.5 when no oplog (neutral)

  R_witness = |{inv : witness(inv).status = 'valid'}| / |modified_invariants|
    -- fraction of modified invariants with valid witnesses
    -- always computable (only needs DB)
    -- degrades to 0.0 when no witnesses (honest: nothing verified)

  R_validate = 1 if ∃ validate_record r in oplog WHERE r.timestamp > spec_change
               AND r.timestamp < code_change, else 0
    -- whether validation was run between spec and code changes
    -- degrades to 0.5 when no oplog (neutral)

  weights: w₁ = 0.35, w₂ = 0.20, w₃ = 0.25, w₄ = 0.20
    -- spec-first ordering is the dominant intervention (+3-4 quality points
       per Alien Artifact Methodology empirical Study 7)
    -- witness coverage is the strongest mechanical signal
    -- tool usage and validation gates provide process evidence
```

### Sub-Score Computation Details

#### R_spec (Spec-First Ordering Ratio)

For each invariant/ADR modified in the current feature scope:

1. Find the git commit that last modified the spec file containing the element
2. Find the git commit that last modified the code file implementing the element
3. If spec commit timestamp < code commit timestamp: score 1 (correct order)
4. If spec commit timestamp > code commit timestamp: score 0 (wrong order)
5. If spec commit timestamp = code commit timestamp (same commit): score 0.5
6. R_spec = mean of all per-element scores

Implementation: `git log --format='%H %aI' --follow -- <file>` for each
relevant file, then compare timestamps per invariant.

When git is unavailable: R_spec defaults to 0.5 (neutral). The system does
not penalize for missing data — it honestly reports that it cannot measure
this dimension.

#### R_tool (Tool Intermediation Score)

For each module modified in the current feature scope, the expected DDIS
command sequence is:

```
discover (1 per feature) + context (1 per module) + refine (optional) +
validate (1 per spec change) + witness (1 per invariant) + drift (1 per feature)
```

Expected count = 1 + |modified_modules| + |modified_invariants| + 2

Actual count = number of auto-prompting commands found in the oplog since
the feature branch diverged (or since the last `ddis seed`, whichever is
more recent).

R_tool = min(1.0, actual / expected)

When oplog is unavailable: R_tool defaults to 0.5 (neutral).

#### R_witness (Witness Coverage Ratio)

For each invariant that was modified or newly created in the current feature:

- If the invariant has a valid witness: score 1
- If the invariant has a stale witness: score 0.25 (at least it was witnessed once)
- If the invariant has no witness: score 0

R_witness = mean of per-invariant scores

Always computable — only requires the SQLite database.

When no witnesses exist at all: R_witness = 0.0. This is the honest signal:
nothing has been mechanically verified.

#### R_validate (Validation Gate Score)

Check the oplog for a `validate` record whose timestamp falls between the
last spec file change and the first code file change:

```
spec_change_time < validate_time < code_change_time
```

If such a record exists: R_validate = 1.0 (validation gate passed)
If no such record: R_validate = 0.0 (validation gate missed)
If spec and code changed in the same commit: R_validate = 0.5 (ambiguous)

When oplog is unavailable: R_validate defaults to 0.5 (neutral).

---

## 6. Graceful Degradation (Following APP-INV-030)

The system must work at every level of data availability, degrading gracefully
when information sources are missing. This follows the pattern established by
APP-INV-030 (Contributor Topology Graceful Degradation).

| Available Data | Computable Signals | Degraded Signals | Effective PC |
|---------------|-------------------|-----------------|-------------|
| Git + OpLog + DB | All 4 (R_spec, R_tool, R_witness, R_validate) | None | Full fidelity |
| OpLog + DB (no git) | R_tool, R_witness, R_validate | R_spec → 0.5 | 3/4 fidelity |
| DB only (no git, no oplog) | R_witness | R_spec, R_tool, R_validate → 0.5 | 1/4 fidelity |
| Fresh DB (nothing) | None | All → 0.5 except R_witness → 0.0 | Baseline |

The degradation strategy:
- **Missing git**: R_spec defaults to 0.5 (neutral, not penalizing)
- **Missing oplog**: R_tool and R_validate default to 0.5 (neutral)
- **Missing witnesses**: R_witness defaults to 0.0 (honest: nothing verified)
- **All missing**: PC ≈ 0.375 (weighted average of neutral + zero)

The system never blocks. It never refuses to operate. It produces the best
score it can from available data and honestly reports what it couldn't measure.

---

## 7. Integration Points (Zero New Commands)

The radical insight: this is NOT a new `ddis audit` command. It is three
additions to existing infrastructure. No new tool to learn, no new command
to remember, no new habit to form. The enforcement is embedded in the
information flow itself.

### 7a. Signal 11 in ContextBundle — Process Compliance

Add to `internal/search/context.go`:

```go
// ProcessInfo summarizes process compliance for a spec element.
type ProcessInfo struct {
    Score           float64  `json:"score"`              // composite PC score 0.0-1.0
    SpecFirstRatio  float64  `json:"spec_first_ratio"`   // R_spec
    ToolUsage       float64  `json:"tool_usage"`         // R_tool
    WitnessCoverage float64  `json:"witness_coverage"`   // R_witness
    ValidationGate  float64  `json:"validation_gate"`    // R_validate
    Degraded        []string `json:"degraded,omitempty"` // which signals degraded
    Recommendation  string   `json:"recommendation"`     // workflow guidance
}
```

Add field to `ContextBundle`:

```go
type ContextBundle struct {
    // ... existing 10 signals ...
    ProcessCompliance *ProcessInfo `json:"process_compliance,omitempty"`
}
```

In `BuildContext()`, after Signal 10 (witness status), add Signal 11:

```go
// Signal 11: Process compliance
bundle.ProcessCompliance = computeProcessCompliance(db, specID, bundle.Target, oplogPath)
```

The `computeProcessCompliance` function:
1. Queries the oplog for recent commands (tool intermediation)
2. Queries git log if available (spec-first ordering)
3. Queries the witness table (witness coverage)
4. Checks oplog for validation-between-spec-and-code (validation gate)
5. Computes composite score with graceful degradation
6. Generates a recommendation string based on the weakest sub-score

Every `ddis context` call automatically includes process compliance. Any LLM
receiving a context bundle knows the process quality of the target element
without doing anything different.

### 7b. Check 18 in Validator — Process Compliance

Add to `internal/validator/checks.go`:

```go
// Check 18: Process compliance — analyzes oplog + git for methodology
// adherence. Warning-only: never fails validation.
// Governs APP-INV-042.
//
// ddis:maintains APP-INV-042 (process compliance observability)
type checkProcessCompliance struct{}

func (c *checkProcessCompliance) ID() int    { return 18 }
func (c *checkProcessCompliance) Name() string { return "Process compliance" }
func (c *checkProcessCompliance) Applicable(codeRoot string) bool { return true }

func (c *checkProcessCompliance) Run(db *sql.DB, specID int64) CheckResult {
    result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}
    // Passed is ALWAYS true — this check never fails validation.
    // It only emits warnings for methodology deviations.

    // Analyze oplog for tool usage patterns
    // Analyze git log for spec-first ordering
    // Analyze witness coverage
    // Report deviations as SeverityWarning findings

    return result
}
```

This check is **warning-only**. It never causes `ddis validate` to fail. It
observes and reports. This follows APP-INV-026 (Classification Non-Prescriptive)
and APP-ADR-018 (Observation over Prescription) — observation, not prescription.

### 7c. Extension to generateGuidance() — Workflow-Aware Recommendations

In `generateGuidance()` (`context.go`), add process-compliance-aware guidance:

```go
// Process compliance guidance
if bundle.ProcessCompliance != nil {
    pc := bundle.ProcessCompliance
    if pc.SpecFirstRatio < 0.5 {
        guidance = append(guidance,
            "Process: spec changes should precede code changes. "+
            "Run `ddis refine audit` on this module before further implementation.")
    }
    if pc.ToolUsage < 0.3 {
        guidance = append(guidance,
            "Process: auto-prompting tools underused. "+
            "Run `ddis discover --content '...'` to establish context before editing.")
    }
    if pc.WitnessCoverage < 0.5 {
        guidance = append(guidance,
            fmt.Sprintf("Process: %.0f%% of modified invariants lack witnesses. "+
                "Run `ddis witness <INV-ID> --verify --code-root .`",
                (1-pc.WitnessCoverage)*100))
    }
    if pc.ValidationGate < 0.5 {
        guidance = append(guidance,
            "Process: no validation run between spec and code changes. "+
            "Run `ddis validate` after spec edits, before implementation.")
    }
}
```

This is the Gestalt principle applied: **demonstrate correct workflow through
guidance, don't constrain against incorrect workflow**. The guidance shows the
agent what the correct next step would have been, encoded naturally in the
editing guidance signal it's already reading.

### 7d. Extension to Drift Report — Process Drift

In `internal/drift/drift.go`, add process drift as a new dimension:

```go
type DriftReport struct {
    // ... existing fields ...
    ProcessDrift   float64          `json:"process_drift,omitempty"`
    ProcessDetails *ProcessDriftInfo `json:"process_details,omitempty"`
}

type ProcessDriftInfo struct {
    SpecFirstRatio     float64  `json:"spec_first_ratio"`
    ToolIntermediation float64  `json:"tool_intermediation"`
    WitnessCoverage    float64  `json:"witness_coverage"`
    ValidateGating     float64  `json:"validate_gating"`
    Degraded           []string `json:"degraded,omitempty"`
}
```

Process drift feeds into the drift report alongside structural drift.
`ddis drift --report` shows both "how far is the spec from the code?" AND
"how well was the methodology followed?"

The process drift score is simply `1.0 - PC(feature)`. When PC = 1.0
(perfect process), process drift = 0.0. When PC = 0.0 (no process followed),
process drift = 1.0.

### 7e. AGENTS.md Convention — Social Enforcement

Add to the project's AGENTS.md:

```markdown
## DDIS Workflow Protocol

Before implementing any plan phase:
1. Run `ddis discover --spec <db> --content 'starting <phase description>'`
2. Edit spec files first (invariants, ADRs, sections)
3. Run `ddis parse && ddis validate` — confirm spec health
4. Implement code changes
5. Run `ddis witness <modified-INVs> --verify --code-root .`
6. Run `ddis drift --report` — confirm drift did not increase
```

This is the cheapest enforcement layer — social convention. It costs nothing
to implement and works even without the tooling changes. The tooling changes
(Signal 11, Check 18, guidance extensions) transform this from a convention
into a self-monitoring system.

### 7f. Human-Readable Rendering

In `renderHumanContext()`, add a PROCESS COMPLIANCE section:

```go
// Process Compliance
if b.ProcessCompliance != nil {
    pc := b.ProcessCompliance
    s.WriteString("PROCESS COMPLIANCE\n")
    fmt.Fprintf(&s, "  Score:           %.0f%%\n", pc.Score*100)
    fmt.Fprintf(&s, "  Spec-first:      %.0f%%\n", pc.SpecFirstRatio*100)
    fmt.Fprintf(&s, "  Tool usage:      %.0f%%\n", pc.ToolUsage*100)
    fmt.Fprintf(&s, "  Witness coverage: %.0f%%\n", pc.WitnessCoverage*100)
    fmt.Fprintf(&s, "  Validation gate: %.0f%%\n", pc.ValidationGate*100)
    if len(pc.Degraded) > 0 {
        fmt.Fprintf(&s, "  Degraded:        %s\n", strings.Join(pc.Degraded, ", "))
    }
    if pc.Recommendation != "" {
        fmt.Fprintf(&s, "  Recommendation:  %s\n", pc.Recommendation)
    }
    s.WriteString("\n")
}
```

---

## 8. Formal Invariant: APP-INV-042

```
**APP-INV-042: Process Compliance Observability**

*For every feature modification scope, the system computes a process
compliance score PC ∈ [0.0, 1.0] from available data sources (git log,
oplog, witness table, validation records). The score is included in
context bundles (Signal 11) and validation reports (Check 18). Missing
data sources degrade individual sub-scores to neutral (0.5), never block
computation. The check is warning-only — it never fails validation.*

FOR ALL features f modified by agent a:
  EXISTS pc IN ProcessCompliance WHERE
    pc.feature = f AND
    pc.score = w₁·R_spec(f) + w₂·R_tool(f) + w₃·R_witness(f) + w₄·R_validate(f) AND
    pc.score ∈ [0.0, 1.0] AND
    pc.degraded = {s : s could not be computed from available data}

FOR ALL context bundles b targeting element e:
  IF e is related to a recently modified feature f:
    b.ProcessCompliance IS NOT NULL

FOR ALL validation runs v:
  v.results CONTAINS process_compliance_check
  process_compliance_check.passed = true  (always — warning-only)

Violation scenario: An agent implements APP-INV-042 itself but writes
code before spec, never uses ddis discover, and doesn't witness the
invariant. The context bundle for APP-INV-042 shows
ProcessCompliance.Score = 0.15 with SpecFirstRatio = 0.0 and
ToolUsage = 0.0. The editing guidance says: "Process: spec changes
should precede code changes." The irony is mechanical: the tool that
measures process compliance reveals its own process was not followed.

Validation: Implement APP-INV-042 following correct methodology
(spec-first, tool-intermediated). Verify Signal 11 appears in context
bundles. Verify Check 18 reports in validation. Then deliberately
implement a feature code-first and verify PC score reflects the
violation. Then remediate (ddis refine audit, ddis witness --verify)
and verify PC score improves.

// WHY THIS MATTERS: Without process compliance observability, the
bilateral lifecycle has no meta-loop — no mechanism that observes the
methodology itself. Agents silently violate spec-first ordering,
drift from the bilateral lifecycle, and produce retro-fitted specs
that look correct but were derived from code rather than driving it.
The process compliance signal closes this gap by making methodology
adherence visible through the same information channels the agent
already reads.
```

---

## 9. Architecture Decision: APP-ADR-031

```
### APP-ADR-031: Observational Process Compliance over Prescriptive Gates

#### Problem

Agents violate spec-first methodology by defaulting to implementation-first
when given a detailed plan. The plan's ordering instructions attenuate with
conversation depth (k* decay from LLM Gestalt Theory). The mid-DoF saddle
(Alien Artifact Methodology) pulls toward the implementation substrate.
Pre-commit hooks are coercive, bypassable, and create adversarial dynamics
between tool and user.

#### Options

A) **Prescriptive gates** — Pre-commit hook that blocks commits when spec
files haven't been modified before code files. `ddis validate --strict`
fails when process compliance is below threshold.
- Pros: Hard enforcement. Impossible to bypass without disabling.
- Cons: Coercive. Creates adversarial dynamics. Bypassable via
  `--no-verify`. Blocks legitimate workflows (hotfixes, prototyping).
  Violates APP-INV-026 (Classification Non-Prescriptive) and
  APP-ADR-018 (Observation over Prescription).

B) **Observational compliance** — Process compliance is OBSERVED and
REPORTED through existing information channels (context bundles,
validation, drift), never ENFORCED through gates or blocks.
- Pros: Non-coercive. Works with all workflows. Self-correcting via
  guidance. Follows existing DDIS principles (observe, don't prescribe).
  Gracefully degrades. No new commands to learn.
- Cons: Agent can ignore warnings. No hard enforcement. Requires agent
  to read and act on guidance.

C) **Hybrid** — Observational by default, prescriptive opt-in via
`--strict` flag on validate/drift.
- Pros: Flexibility. CI pipelines can enforce, humans can observe.
- Cons: Complexity. Two modes to maintain. Strict mode still has the
  coercion problems of Option A.

#### Decision

**Option B: Observational compliance.** Process compliance is OBSERVED and
REPORTED through existing information channels (context bundles, validation,
drift), never ENFORCED through gates or blocks.

This follows APP-INV-026 (classification non-prescriptive) and APP-ADR-018
(observation over prescription) applied to the methodology itself. The
Gestalt Theory principle applies: **demonstrate correct ordering through
guidance, don't constrain against incorrect ordering**. Constraints are
parasitic when the agent already knows the correct behavior — it just needs
a reminder at the right time.

// WHY NOT Prescriptive gates (Option A)? Pre-commit hooks that block work
create adversarial dynamics. The agent learns to circumvent the gate rather
than internalize the methodology. This is the Gestalt "constraint as
parasitic attention sink" anti-pattern applied to process enforcement.
Blocking is the wrong substrate for methodology adoption.

// WHY NOT Hybrid (Option C)? The strict mode reintroduces all the problems
of Option A for anyone who enables it. If observational compliance works
(and the Gestalt Theory predicts it will — demonstrations outperform
constraints), the strict mode is unused complexity. If observational
compliance doesn't work, the strict mode is a band-aid that masks the
real problem (the guidance isn't reaching the agent at the right time).

#### Consequences

- Context bundles include process compliance (Signal 11)
- Validation includes process compliance check (Check 18, warning-only)
- Drift reports include process drift dimension
- Editing guidance recommends correct workflow sequencing
- No agent is ever blocked from working
- Methodology deviations are recoverable, not permanent
- Self-bootstrapping: the tool measures its own process compliance

#### Tests

- Context bundle for a spec-first feature shows PC > 0.8
- Context bundle for a code-first feature shows PC < 0.4
- Validation Check 18 emits warnings for code-first, no warnings for spec-first
- Drift report includes process drift when PC < 1.0
- Guidance recommends remediation steps when sub-scores are low
- Graceful degradation: all sub-scores compute when git/oplog missing
```

---

## 10. Why This Is The Smartest Addition

### Accretive

Every existing tool gets better. `ddis context` becomes process-aware.
`ddis validate` becomes process-aware. `ddis drift` becomes process-aware.
`ddis progress` can weight done-set items by process compliance. No tool
loses functionality. No new command needed.

### Zero Friction

No new commands to learn. No new habits to form. The process compliance
signal flows through existing infrastructure. An agent receiving a context
bundle is automatically told whether the process was followed correctly.
The enforcement is embedded in the information flow itself.

### Theoretically Grounded

- **Alien Artifact Methodology**: Detects phase ordering violations
  (FORMALIZE→DERIVE→SPECIFY→IMPLEMENT precondition checking)
- **LLM Gestalt Theory**: Demonstrates correct workflow rather than
  constraining against incorrect workflow (demonstrations > constraints)
- **Skill Field Dynamics**: Treats DDIS commands as skills to be sequenced
  across cognitive phases, not stacked simultaneously

### Self-Bootstrapping

Process compliance can be measured for the process compliance feature itself.
`ddis context APP-INV-042 --json` would show the process compliance score
for the invariant that defines process compliance. The tool watches itself.

### Gracefully Degrades

Follows APP-INV-030 exactly. Works with full git+oplog (highest fidelity),
oplog-only (good), DB-only (baseline), or nothing (neutral defaults). The
system never blocks, never refuses, never penalizes for missing data.

### Category-Theoretic Completion

The bilateral lifecycle has four loops (discover, refine, drift, absorb) but
no **meta-loop** — no mechanism that observes the loops themselves. Process
compliance is the natural monad transformer: it lifts the bilateral lifecycle
into a process-aware bilateral lifecycle where the methodology monitors itself.

In category-theoretic terms: the four loops are endofunctors on the spec
category. Process compliance is a natural transformation from the "actual
workflow" functor to the "prescribed workflow" functor. The compliance score
is the distance between these two functors. When the score is 1.0, the
natural transformation is an isomorphism — the actual workflow IS the
prescribed workflow.

---

## 11. Discovery Thread Evolution

This proposal evolved through 6 `ddis discover` invocations on thread
`t-1772030865583`, demonstrating the bilateral lifecycle applied to its own
design:

1. **Initial question**: "How should the spec enforce spec-first methodology?"
   (Mode: divergent, DoF: very high)

2. **Three-lens analysis**: Applied Alien Artifact (phase ordering), Gestalt
   (mid-DoF saddle), and Skill Dynamics (composition protocol) to diagnose
   the root cause.

3. **First crystallization**: Proposed `ddis audit` — a new command for
   process compliance. (Initial form: a standalone tool)

4. **Key refinement**: "The OpLog already IS the process witness — we just
   need to read it." Realized the data already exists. (Pivotal insight)

5. **Pivot**: From "new command" to "new signal in existing context bundles."
   Zero friction design. No new commands. (Architecture decision)

6. **Final synthesis**: Complete PC score formula with 4 sub-scores, 5
   integration points, graceful degradation table, formal invariant,
   and ADR. (Crystallization complete)

This evolution is itself a demonstration of the methodology working correctly:
the *idea* was refined through discovery (FORMALIZE → DERIVE → SPECIFY),
not implemented directly. The design was explored at high DoF, then
crystallized at low DoF.

---

## 12. Implementation Order (Spec-First, Naturally)

### Phase 0: Spec Changes

1. Add APP-INV-042 to `ddis-cli-spec/modules/lifecycle-ops.md`
2. Add APP-ADR-031 to `ddis-cli-spec/modules/lifecycle-ops.md`
3. Add APP-INV-042 to `ddis-cli-spec/manifest.yaml` invariant registry
4. Add APP-ADR-031 to `ddis-cli-spec/manifest.yaml` lifecycle-ops implements
5. Update `ddis-cli-spec/constitution/system.md` registry entries
6. `ddis parse` + `ddis validate` + `ddis drift` — confirm spec health

### Phase 1: ProcessCompliance Computation

1. New file: `internal/process/compliance.go` — PC score computation
2. Git log analysis (with graceful degradation)
3. OpLog analysis (with graceful degradation)
4. Witness coverage computation
5. Validation gate detection
6. Composite score with configurable weights

### Phase 2: Context Bundle Integration (Signal 11)

1. Add `ProcessCompliance *ProcessInfo` to `ContextBundle` struct
2. Add `computeProcessCompliance()` call in `BuildContext()`
3. Add process-compliance-aware guidance in `generateGuidance()`
4. Add PROCESS COMPLIANCE section in `renderHumanContext()`

### Phase 3: Validator Integration (Check 18)

1. Add `checkProcessCompliance` struct implementing Check interface
2. Warning-only: `Passed` always `true`
3. Register in `AllChecks()` slice

### Phase 4: Drift Integration

1. Add `ProcessDrift` and `ProcessDetails` to `DriftReport`
2. Compute process drift in `Analyze()`
3. Render in drift report output

### Phase 5: Tests

1. `TestComputeProcessCompliance_FullData`
2. `TestComputeProcessCompliance_NoGit`
3. `TestComputeProcessCompliance_NoOplog`
4. `TestComputeProcessCompliance_DBOnly`
5. `TestContextBundle_Signal11`
6. `TestValidator_Check18_WarningOnly`
7. `TestDriftReport_ProcessDrift`
8. `TestGuidance_ProcessAware`

### Phase 6: Self-Bootstrap

1. `ddis witness APP-INV-042 --verify --code-root .`
2. `ddis context APP-INV-042` — verify Signal 11 appears
3. `ddis validate` — verify Check 18 reports
4. `ddis drift --report` — verify process drift appears
5. Measure own process compliance score (should be high if we followed
   this plan correctly)

---

## 13. Critical Files Summary

| File | Action | LOC |
|------|--------|-----|
| `ddis-cli-spec/modules/lifecycle-ops.md` | MODIFY — add APP-INV-042, APP-ADR-031 | ~80 |
| `ddis-cli-spec/manifest.yaml` | MODIFY — add registry entries | ~5 |
| `ddis-cli-spec/constitution/system.md` | MODIFY — add registry entries | ~5 |
| `ddis-cli/internal/process/compliance.go` | **CREATE** — PC score computation | ~250 |
| `ddis-cli/internal/process/compliance_test.go` | **CREATE** — 8 test cases | ~200 |
| `ddis-cli/internal/search/context.go` | MODIFY — add Signal 11, guidance | ~50 |
| `ddis-cli/internal/validator/checks.go` | MODIFY — add Check 18 | ~40 |
| `ddis-cli/internal/validator/validator.go` | MODIFY — register Check 18 | ~1 |
| `ddis-cli/internal/drift/drift.go` | MODIFY — add process drift | ~30 |
| **Total** | | **~661** |

---

## 14. Verification Checklist

### Spec Health
- [ ] `ddis parse` succeeds with new invariant/ADR
- [ ] `ddis validate` passes all checks
- [ ] `ddis drift --report` shows 0 drift
- [ ] `ddis coverage` shows correct invariant count

### Signal 11 (Context Bundle)
- [ ] `ddis context APP-INV-042 --json` includes `process_compliance` field
- [ ] All 4 sub-scores present in JSON output
- [ ] Degraded signals listed when git/oplog missing
- [ ] Recommendation string non-empty when score < 1.0
- [ ] Human-readable rendering includes PROCESS COMPLIANCE section

### Check 18 (Validator)
- [ ] Check 18 appears in `ddis validate` output
- [ ] Check 18 ALWAYS passes (warning-only)
- [ ] Warnings emitted when process deviations detected
- [ ] No warnings when process was followed correctly

### Drift Integration
- [ ] `ddis drift --report` includes process drift dimension
- [ ] Process drift = 0.0 when PC = 1.0
- [ ] Process drift = 1.0 when PC = 0.0
- [ ] Process drift details include all 4 sub-scores

### Graceful Degradation
- [ ] Full data (git + oplog + DB): all 4 sub-scores computed
- [ ] No git: R_spec → 0.5, other 3 computed
- [ ] No oplog: R_tool, R_validate → 0.5, other 2 computed
- [ ] DB only: R_witness computed, others → 0.5
- [ ] System never panics, blocks, or errors on missing data

### Guidance Integration
- [ ] Low R_spec triggers "spec changes should precede code changes" guidance
- [ ] Low R_tool triggers "auto-prompting tools underused" guidance
- [ ] Low R_witness triggers "invariants lack witnesses" guidance
- [ ] Low R_validate triggers "no validation between spec and code" guidance
- [ ] High PC (> 0.8) triggers no process-related guidance

### Self-Bootstrap
- [ ] APP-INV-042 witnessed with `--verify` (code annotations exist)
- [ ] `ddis context APP-INV-042` shows own process compliance
- [ ] Process compliance of process compliance feature is measurable
- [ ] Following this plan correctly produces PC > 0.8

---

## 15. Relationship to Existing Invariants

| Existing Invariant | Relationship to APP-INV-042 |
|-------------------|----------------------------|
| APP-INV-022 (Refinement Drift Monotonicity) | Process compliance detects when refine was skipped entirely |
| APP-INV-026 (Classification Non-Prescriptive) | Process compliance is observational, following this principle |
| APP-INV-028 (Spec-as-Trunk) | Process compliance measures whether spec was trunk (changed first) |
| APP-INV-030 (Graceful Degradation) | Process compliance follows the exact degradation pattern |
| APP-INV-034 (State Monad Universality) | Process compliance flows through the state monad (Signal 11) |
| APP-INV-035 (Guidance Attenuation) | Process guidance attenuates with conversation depth |
| APP-INV-041 (Witness Auto-Invalidation) | Witness coverage is a sub-score of process compliance |
| APP-ADR-018 (Observation over Prescription) | Process compliance observes, never prescribes |
| APP-ADR-024 (Bilateral Specification) | Process compliance is the meta-loop of the bilateral lifecycle |

---

## 16. How This Addresses the User's Questions

### "How can we enforce usage of ddis by the human and the AI agent?"

Three layers, ordered by gentleness:

1. **Information flow** (zero friction): Signal 11 in context bundles.
   Every agent already receiving context bundles automatically receives
   process compliance data. No new habit needed.

2. **Social convention** (low friction): AGENTS.md workflow protocol.
   The cheapest enforcement — a written convention that agents read at
   session start.

3. **Auto-prompting guidance** (medium friction): Workflow-sequenced
   recommendations in editing guidance. When process compliance is low,
   the guidance explicitly recommends the correct next DDIS command.

### "How can we gracefully degrade to adapt to the failure mode?"

Process compliance transforms methodology failure from a **permanent gap**
into a **recoverable deviation**:

1. **During implementation**: Nothing blocks. The agent works naturally.
2. **After implementation**: `ddis context` reveals PC score. `ddis validate`
   reports process warnings. `ddis drift --report` shows process drift.
3. **Remediation**: The guidance recommends specific steps:
   - `ddis refine audit` on spec sections written after code
   - `ddis absorb` to capture implicit decisions
   - `ddis witness --verify` on implemented invariants
4. **Net effect**: The bilateral lifecycle absorbs the deviation and
   converges back to alignment. The process drift dimension in the drift
   report tracks this convergence.

### "What's the single smartest addition to the plan?"

Process compliance through existing information flow. Zero new commands.
Self-monitoring methodology. Gracefully degrading. Theoretically grounded
in three frameworks. Self-bootstrapping. Category-theoretically complete
as the meta-loop of the bilateral lifecycle.
