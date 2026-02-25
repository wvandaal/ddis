---
name: feature-discovery
description: Transform nebulous feature ideas into DDIS-conforming specs through structured discovery. Decisions become ADRs, risks become invariants, findings become spec sections — incrementally, as questions converge. Extends the service's existing spec rather than creating isolated feature specs.
argument-hint: <feature-name> or status/explore/answer/decide/probe/map/risks/verify/help
origin: WIP skill from collaborator (Root-Rise/project-oak), adapted for DDIS integration
---

# Feature Discovery → DDIS Spec

Discovery and spec creation are the same process. Every "bedrock" moment directly
produces a DDIS artifact in the spec. By the time you're confident, `ddis validate` passes
and the spec is ready for RALPH tightening or implementation.

**One spec per service.** Each discovery extends the service's existing spec — adding
a module, new invariants, new ADRs, amending the constitution. Independent feature specs
create invisible contradictions. A shared spec makes conflicts explicit.

```
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│    NEBULOUS     │ ──►  │   CONVERGING    │ ──►  │   EXECUTABLE    │
│   (Score 0-5)   │      │   (Score 5-8)   │      │   (Score 8-10)  │
└─────────────────┘      └─────────────────┘      └─────────────────┘
  Questions diverge        ADRs + invariants         ddis validate passes
  Exploring existing       accumulating in spec       Artifact map = changelog
  spec for context         Sections filling in        Ready for task generation
```

## One Spec Per Service

```
First feature for a service   →  Creates the service's monolith DDIS spec
Subsequent features           →  Extend the existing spec (new sections, ADRs, invariants)
Spec grows past ~4000 lines   →  Modularize via DDIS protocol (constitution + modules)
After modularization          →  Each feature adds a new module + amends constitution
```

When `start` runs, it checks for an existing spec:

- **Existing spec found**: Load it, explore what's already there, begin discovery in context
  of existing invariants and ADRs. New artifacts extend the spec. Amendments are explicit.
- **No spec exists**: Ask the user for the spec path. Create the initial skeleton.

This prevents the n+1 problem: Feature B can't silently contradict Feature A because they
share invariants in the same spec. If Feature B needs to change an invariant Feature A
established, it must amend it explicitly — which creates a visible amendment record and
generates tasks to update the existing implementation.

## How Discovery Events Become DDIS Artifacts

As questions hit bedrock (resolve with 0 children), write the appropriate artifact
directly into the spec using exact DDIS parser-compatible formats.

| You discover... | Write this DDIS artifact |
|---|---|
| A decision with rationale + alternatives | **ADR** in `## 0.6` section |
| A risk with a concrete failure scenario | **Invariant** in `## 0.5` with violation scenario |
| A domain term that needed clarification | **Glossary entry** in `Appendix A` |
| A "must NOT do X" conclusion | **Negative spec** in relevant implementation section |
| A constraint with a measurable threshold | **Performance budget** or invariant |
| Implementation ordering between components | Chapters under `## 2` with cross-refs |
| System behavior resolved with evidence | Section prose in `## 1` |
| A change to an existing invariant or ADR | **Amendment** on the existing artifact |

## The Artifact Map Is the Changelog

The artifact map in the discovery state tracks every artifact produced or amended.
This is how task generation knows what changed — no git archaeology needed.

```json
"artifact_map": {
  "q2.1": [
    { "type": "adr", "id": "ADR-007", "title": "Primary Contact Update Strategy", "action": "created" },
    { "type": "invariant", "id": "INV-004", "title": "Primary Contact Uniqueness", "action": "created" },
    { "type": "negative_spec", "section": "§3", "text": "DO NOT silently drop primary changes", "action": "created" }
  ],
  "q5.2": [
    { "type": "invariant", "id": "INV-002", "title": "Sync Idempotency", "action": "amended",
      "amendment": "Relaxed from exactly-once to at-least-once with idempotency key" }
  ]
}
```

Task generation reads the artifact map:

| Artifact | Tasks generated |
|---|---|
| New ADR | Implement the decision |
| New invariant | Implement the constraint + write enforcement test |
| New negative spec | Implement the guard + write regression test |
| Amended invariant | Update existing implementation + update existing tests |
| Amended ADR | Update implementation to match new decision |
| New glossary entry | No task (documentation only) |

### Negative Specs Produce Two Tasks

Every negative spec generates both an implementation task and a test task:

```
- **DO NOT** silently drop primary changes when contact doesn't exist (Validates INV-004)

  → Task 1: Implement guard — check-and-create before marking primary
  → Task 2: Regression test — set Contact_Name to unknown contact, assert participant created
```

Negative specs are the highest-value tests. They guard against specific anticipated failure
modes. Without the test, the guard is one refactor away from being accidentally removed.

### Invariants Produce Property Tests

Each invariant's validation method defines its test. The violation scenario defines
the negative case. Together they produce:

```
INV-004: Primary Contact Uniqueness
  Validation: DB unique partial index + integration test
  Violation: Concurrent sync events create duplicate primaries

  → Task 1: Add DB constraint (partial unique index)
  → Task 2: Property test — for all deals with contacts, exactly one is primary
  → Task 3: Regression test — fire concurrent primary-change webhooks, assert one primary
```

## Confidence = min(five dimensions) = spec completeness

| Dimension | The Question | DDIS Validation |
|---|---|---|
| **Problem Understanding** | Do we know what we're solving? | Glossary complete (Check 4), §0.1 exists (Check 10) |
| **Solution Clarity** | Are decisions well-reasoned? | ADRs have all 5 subsections |
| **Implementation Visibility** | Can we see the path to done? | Negative spec coverage (Check 9), proportional weight (Check 11) |
| **Risk Awareness** | Do we know what could go wrong? | Invariant falsifiability (Check 2) — all 4 components present |
| **Unknown Unknown Coverage** | How much is unexplored? | Cross-ref integrity (Check 1), no orphan sections (Check 3) |

When all dimensions >= 8, `ddis validate` should pass. If it doesn't, the scores are lying.

## Question Tree

Questions form a tree. Convergence = the tree stops growing.

- **open** → identified, not explored
- **answered** → has answer, may have spawned children
- **resolved** → bedrock, produced a DDIS artifact
- **deferred** → explicitly out of scope

When an answer spawns 0 children → bedrock. Write the artifact.
When an answer spawns 2+ children → new territory. Good early, concerning late.

Categories: **product** (domain, scope), **technical** (data, APIs, architecture),
**integration** (external systems, data flow), **risk** (failure modes, edge cases).

## Probing for Unknown Unknowns

Generate probes specific to the feature context across these surfaces:

- **System boundaries**: What does this touch that we haven't examined?
- **Assumptions**: What are we taking as given without verifying?
- **Temporal edges**: Create/update/delete paths? Failure during sync? Ordering?
- **Counter-perspective**: What would a skeptic say? If this fails in prod, why?
- **Existing spec conflicts**: Does this contradict any existing invariants or ADRs?
- **Domain-specific**: Whatever matters for this particular system.

## Initialization Protocol

On `start <feature-name>`:

1. Find repo root: `git rev-parse --show-toplevel` (or walk up looking for `.ddis/`)
2. Check for an existing DDIS spec:
   - Look for `manifest.yaml` (modular spec)
   - Look for `*.md` files that `ddis parse` has indexed (check for `.ddis.db` files)
   - Ask the user if multiple candidates or none found
3. **Existing spec found**: Load it, read current invariants/ADRs/glossary for context.
   New artifacts continue numbering from where the spec left off.
4. **No spec found**: Ask the user for the spec path. Create the initial skeleton.
5. Create discovery state at `.ddis/discoveries/<feature>.json`

## DDIS Artifact Format Reference

These formats match the exact regex patterns in `ddis-cli/internal/parser/patterns.go`.
The spec MUST use these formats or `ddis parse` will not index the artifacts.

### Invariant

Write into the `## 0.5 Invariants` section. ID format: `INV-NNN` (3-digit, zero-padded).
Continue numbering from the highest existing invariant in the spec.

```markdown
**INV-001: Descriptive Title**

*Plain language statement of what must always hold.*

formal_expression(x) → consequence(x)

Violation scenario: A concrete scenario where this invariant is violated.

Validation: How to test that this invariant holds.

// WHY THIS MATTERS: One sentence on the consequence if violated in production.
```

### ADR

Write into the `## 0.6 Architecture Decision Records` section. ID format: `ADR-NNN`.

```markdown
### ADR-001: Descriptive Title

#### Problem
Why this decision needed to be made.

#### Options
A) **Option Name**
- Pros: concrete advantages
- Cons: concrete disadvantages

B) **Alternative Name**
- Pros: concrete advantages
- Cons: concrete disadvantages

#### Decision
**Option A: Option Name.** Rationale. References INV-001.

// WHY NOT Option B? Brief explanation.

#### Consequences
- Consequence 1
- Consequence 2

#### Tests
How to verify this decision was implemented correctly.

---
```

### Glossary Entry

Append row to the glossary table in `Appendix A`:

```markdown
| **Term** | Definition text, can reference §1.2 or ADR-001 |
```

### Negative Spec

Write into relevant implementation section (§1, §2, etc.):

```markdown
- **DO NOT** perform action X without checking Y (Validates INV-001)
```

Each negative spec produces two tasks: an implementation guard and a regression test.

### Amendment

Append below the existing artifact being amended:

```markdown
**Amendment (YYYY-MM-DD)**: What changed.
**Why changed**: Evidence or reasoning.
**Impact**: What existing code/tests need updating.
```

### Cross-References

Use inline in prose. Parser extracts automatically:
- Sections: `§0.5`, `§1.2`
- Invariants: `INV-001`
- ADRs: `ADR-003`
- Gates: `Gate 1`

## Commands

| Command | When | What happens |
|---|---|---|
| `start <name>` | Beginning | Find existing spec or create skeleton, create discovery state, load existing invariants/ADRs for context |
| `status` | Any time | Phase, confidence, artifact counts, question tree summary, next recommendation |
| `explore <topic>` | Nebulous/Converging | Deep dive, generate questions, write glossary entries and section drafts |
| `answer` | After user responds | Record answer; if bedrock → write DDIS artifact into spec |
| `decide <topic>` | When choice is clear | Write ADR block into §0.6 with all 5 subsections |
| `risks` | Confidence >= 5 | Write invariant blocks into §0.5, negative specs into impl sections |
| `probe` | Periodically | Run unknown-unknown probes — including checking for conflicts with existing spec |
| `map` | Confidence >= 6 | Write implementation chapters under §2 with phases, files, ordering |
| `verify` | Confidence >= 8 | Run `ddis parse` + `ddis validate` on full spec, reconcile with confidence |
| `help` | Any time | Show commands and current state |

## Verify Command Detail

`verify` validates the **entire spec**, not just the new artifacts. This catches:
- Cross-reference breaks between new and existing artifacts
- Contradictions surfaced by structural checks
- Invariants that lost components during amendment

1. Run: `ddis parse <spec_path> -o /tmp/<feature>.ddis.db`
2. Run: `ddis validate /tmp/<feature>.ddis.db --json`
3. Parse the JSON output
4. Reconcile with confidence dimensions
5. Report artifact map summary showing what this discovery created/amended
6. Preview task generation: list the tasks that would be created from the artifact map

## State Management

Discovery state persists in `.ddis/discoveries/<feature>.json`. Tracks:
- Question tree with parent/child relationships
- Artifact map: which question produced which DDIS artifact, whether created or amended
- Confidence dimensions with evidence and gaps
- Counters for INV/ADR/Gate numbering (continuing from existing spec)
- Last validation results
- Reference to the spec file being extended

Discovery can span multiple sessions. On resume: load state, summarize where we left off,
run `ddis validate` to check current spec health, continue from current phase.

After `verify` passes, the artifact map drives task generation via `/tasks`.

## Amending Existing Artifacts

When a new feature needs to change an existing invariant or ADR:

1. Do NOT delete the original — preserve the reasoning
2. Append an amendment section below the original in the spec
3. Record in artifact map as `"action": "amended"` with description of the change
4. Task generation will create update tasks for existing implementation
5. If the change is fundamental (not a refinement), flag it — existing tests may need
   rewriting, not just updating
