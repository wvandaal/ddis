# R5.1a: Seed-as-Prompt Optimization Analysis

> **Bead**: R5.1a
> **Traces to**: SEED.md SS5, SS8; spec/06-seed.md (ADR-SEED-003, ADR-SEED-004); spec/12-guidance.md (INV-GUIDANCE-007); spec/13-budget.md
> **Status**: Research complete

---

## 1. Premise

The Seed output is not merely a data transfer mechanism. It is a **prompt** that
bootstraps a fresh LLM session into a functioning agent with project-specific
knowledge, methodology, and direction. The five-part structure (Orientation,
Constraints, State, Warnings, Directive) maps directly to five distinct
**prompt-engineering functions** that activate different cognitive substrates in
the model. Optimizing the seed means optimizing the prompt, subject to the token
budget constraint (INV-SEED-002) and the rate-distortion framework (ADR-BUDGET-003).

This analysis examines each section through the lens of prompt optimization,
identifies current strengths and gaps, and provides concrete recommendations.

---

## 2. Section-by-Section Analysis

### 2.1 Orientation

**Prompt function**: Activating the project-specific reasoning substrate.

Orientation performs **identity priming** -- it tells the model *what it is* and
*where it is in the project lifecycle*. This is analogous to a system prompt that
establishes persona and context. Without it, the model defaults to its pretrained
generic-assistant identity (Basin B in the basin competition model, ADR-GUIDANCE-002).

**Current spec** (ADR-SEED-004):
> "Project identity, current phase, recent session history."

**What makes a good Orientation section**:

1. **Concise identity activation, not verbose project description.** The seed's
   Orientation should be 2-3 sentences, not paragraphs. The key insight from
   prompt optimization research is that identity priming works via pattern
   matching on *genre markers*, not through exhaustive description. "You are
   working on Braid (DDIS datom store). Current phase: Stage 0 implementation.
   Active namespace: STORE." activates the correct substrate. A 500-word project
   overview wastes budget and dilutes the activation.

2. **Phase anchoring is critical.** The current phase determines which Basin A
   patterns the model should activate. "Stage 0 implementation" triggers
   different reasoning than "specification production." The phase should be
   a keyword, not a description.

3. **Active namespace narrows the cognitive aperture.** Including which spec
   namespace is active (e.g., "spec/01-store.md") tells the model to
   pre-activate that domain's concepts. This is a form of **selective attention
   cueing** -- it biases the model toward relevant patterns before any task
   is presented.

4. **Session history should be frontier-oriented.** Not "here is what happened
   in the last 3 sessions" but "your frontier is at tx_47; the store has 312
   datoms; the last harvest drift score was 0.0." Numeric anchors are more
   effective than narrative summaries because they activate the formal-reasoning
   substrate (consistent with ADR-SEED-003: spec-language over instruction-language).

**Recommendations**:

- **Budget**: Orientation should consume at most 80 tokens (consistent with
  INV-GUIDANCE-007's ambient section limit). It is the one section that should
  be nearly identical across sessions for the same project phase.
- **Template**: Use a fixed template with slot-fills, not dynamically generated prose.
  The template itself becomes a pattern the model recognizes across sessions,
  building cross-session consistency.
- **Anti-pattern**: Long orientation sections cause "orientation fatigue" -- the
  model spends attention processing context that does not inform action. Every
  token in Orientation that does not change behavior is deadweight
  (INV-SEED-005 falsification condition).

**Proposed template** (approx. 50 tokens):
```
## Orientation
Project: Braid (DDIS datom store). Phase: {phase}. Namespace: {namespace}.
Store: {datom_count} datoms. Frontier: {frontier_summary}. Last harvest drift: {drift_score}.
Prior session: {one_sentence_summary}.
```

---

### 2.2 Constraints (formerly "Decisions" in guide template)

**Prompt function**: Pre-loading committed choices to prevent relitigating (NEG-002).

The Constraints section performs **decision anchoring** -- it establishes what has
already been decided and is not up for debate. This is the prompt-optimization
equivalent of **few-shot commitment framing**: by presenting settled decisions as
constraints rather than recommendations, the model is more likely to treat them
as load-bearing axioms rather than suggestions to evaluate.

**Current spec** (ADR-SEED-004):
> "Relevant INVs, settled ADRs, negative cases for current task."

**What makes a good Constraints section**:

1. **Commitment weight ordering.** Present constraints in descending commitment
   weight. High-weight ADRs (w >= 8) appear first with an explicit "do not
   relitigate" signal. Low-weight ADRs (w <= 3) appear last with "revisable
   if contradicted." This activates the model's sensitivity to authority
   signals -- it will be more reluctant to violate a w=12 constraint than
   a w=3 one.

2. **"Do not relitigate" as a behavioral nudge.** Including the phrase "do not
   relitigate" after high-weight ADRs is not just documentation -- it is a
   **behavioral constraint** that leverages the model's instruction-following
   tendencies. ADR-SEED-003 says to use spec-language, not instruction-language,
   but this is the one place where a direct imperative is warranted because the
   failure mode it prevents (NEG-002) is the single most common LLM failure in
   extended sessions.

3. **Relevant to the task, not exhaustive.** Including all 97+ invariants would
   overwhelm the budget. The ASSOCIATE step (INV-SEED-003) should select only
   constraints that are graph-adjacent to the current task's entities. A constraint
   that is two hops away from the task entity is noise.

4. **Negative cases are the highest-value constraints.** NEG cases tell the model
   what *not* to do. Prompt optimization research consistently shows that
   explicitly stated negative constraints are more effective than implied ones.
   "Do not generate aspirational stubs (NEG-001)" is worth 3x its token cost
   compared to a positive invariant, because it prevents entire failure modes.

**Recommendations**:

- **Budget**: 100-200 tokens. Scale with task complexity.
- **Ordering**: NEG cases first (highest behavioral leverage per token), then
  high-weight ADRs, then relevant INVs. This ensures that under budget pressure,
  the most actionable constraints survive compression.
- **Format**: Use the invariant ID as the leading element, not a description.
  "INV-STORE-001: append-only" activates formal pattern matching. "The store
  should be append-only" activates generic reasoning.
- **Commitment weight as explicit metadata**: Include `(w=12)` after each
  constraint. This numerical signal is more effective than prose about importance.

**Proposed template** (variable, 100-200 tokens):
```
## Constraints
*Negative cases (do NOT violate):*
- NEG-001: No aspirational stubs — no `unimplemented!()`, no `// TODO`
- NEG-003: No premature optimization — correctness first

*Settled decisions (do not relitigate):*
- ADR-STORE-002: BLAKE3 for content hashing (w=12)
- ADR-STORE-004: HLC for transaction ordering (w=8)
- ADR-STORE-009: redb for persistence (w=3, revisable)

*Active invariants:*
- INV-STORE-001: Append-only immutability
- INV-STORE-003: Content-addressed identity
```

---

### 2.3 State (formerly "Context" in guide template)

**Prompt function**: Providing working memory (recent state for continuation).

The State section performs **recency-weighted memory injection** -- it gives the
model the equivalent of "where were we?" This is the most information-dense
section and the primary target for rate-distortion compression (ADR-BUDGET-003).

**Current spec** (ADR-SEED-004):
> "Relevant datoms, artifacts, frontier, recent changes."

**What makes a good State section**:

1. **Recency-weighted, not chronological.** The most recent transactions should
   appear first with full detail (pi_0). Older transactions should appear as
   summaries (pi_1/pi_2). Ancient context should be omitted. The decay
   function from spec/13-budget.md (attention_decay) should also apply to
   state section ordering: recent items get more tokens because they are
   more likely to be relevant.

2. **Frontier-aware, not just time-aware.** The state should be presented relative
   to the agent's frontier, not as an absolute timeline. "Since your last
   session (tx_42), 5 transactions have occurred: ..." is more actionable
   than "Recent transactions: tx_42, tx_43, tx_44, tx_45, tx_46, tx_47."

3. **Active uncertainties are high-value state.** Uncertainty markers
   (spec/15-uncertainty.md) tell the model where the specification is
   incomplete. These are high-value because they prevent the model from
   treating uncertain claims as settled facts (NEG-007). Including
   uncertainty markers in the State section (rather than buried in
   constraints) makes them visible as part of the current working context.

4. **Diff-oriented, not snapshot-oriented.** The model needs to know what
   *changed* since the last session, not the full store state. A diff
   ("3 new INVs added, 1 ADR revised, drift score decreased from 0.3 to
   0.1") is more actionable per token than a snapshot ("Store contains 312
   datoms across 14 namespaces").

**Recommendations**:

- **Budget**: 100-300 tokens. This section absorbs the most compression
  under budget pressure.
- **Structure**: Lead with the delta (what changed), then the snapshot
  (where things stand). Delta is higher value per token.
- **Compression strategy**: Under budget pressure, compress State before
  Warnings or Directive. State is reconstructible from the store; Warnings
  and Directive are not.
- **Projection pyramid**: Use pi_1 for most state items, pi_0 only for
  items directly related to the task.

**Proposed template** (variable, 100-300 tokens):
```
## State
*Delta since last session (tx_{last} -> tx_{current}):*
{N} new transactions, {M} new datoms. Key changes:
- {most_relevant_change_1}
- {most_relevant_change_2}

*Current snapshot:*
Store: {datom_count} datoms, {entity_count} entities. Frontier: {frontier}.
Active uncertainties: {uncertainty_list_or_"none"}.
```

---

### 2.4 Warnings

**Prompt function**: Activating vigilance and constraint-checking mode.

Warnings perform **threat priming** -- they alert the model to active risks and
failure conditions. This is the most behaviorally potent section per token
because it activates the model's "be careful" substrate, which in turn increases
the likelihood that the model will check constraints before acting.

**Current spec** (ADR-SEED-004):
> "Drift signals, open questions, uncertainties, harvest alerts."

**What makes a good Warnings section**:

1. **Actionable, not informational.** "Drift score is 0.3" is informational.
   "Drift score 0.3 -- 3 uncommitted observations at risk of loss. Run
   `braid harvest` before the end of this session" is actionable. Every
   warning should include: (a) what is wrong, (b) what the consequence of
   inaction is, (c) what to do about it. This maps to the error message
   protocol in guide/00-architecture.md: `{what} -- {why} -- {recovery}`.

2. **Prioritized by consequence, not by recency.** A stale observation
   (low risk) should appear after a critical invariant violation (high risk).
   The consequence dimension of the reconciliation taxonomy
   (consequential divergence) provides the ordering.

3. **Empty warnings section is a positive signal.** "Warnings: None" is
   valuable information -- it tells the model the system is in a healthy
   state and it can proceed with confidence. Do not pad an empty warnings
   section with generic advice.

4. **Harvest alerts are the highest-priority warning.** If Q(t) < 0.15 (from
   INV-HARVEST-005), the harvest alert should dominate the warnings section.
   At Q(t) < 0.05, the warnings section should contain ONLY the harvest
   imperative, consistent with the spec.

5. **Warnings should decay.** A warning that has appeared in 5+ consecutive
   seeds without being resolved should be escalated (larger font / stronger
   language) or resolved by the system. Stale warnings lose behavioral
   effectiveness -- the model learns to ignore them (warning fatigue).

**Recommendations**:

- **Budget**: 50-100 tokens. Warnings should be short and sharp. Long
  warnings dilute their behavioral impact.
- **Format**: Each warning is one line: `[SEVERITY] {what} -- {action}`.
  Severity levels: CRITICAL > HIGH > NOTE.
- **Compression**: Under extreme budget pressure, keep only CRITICAL
  warnings. Remove NOTE-level warnings before removing State items.
- **Empty state**: Explicitly output "Warnings: None." when there are no
  active warnings. This is a positive behavioral signal.

**Proposed template** (variable, 50-100 tokens):
```
## Warnings
- [HIGH] Drift score 0.3 — 3 uncommitted observations. Harvest before session end.
- [NOTE] UNC-SCHEMA-001 unresolved (17 attributes sufficient? confidence=0.85).
```

Or when healthy:
```
## Warnings
None. System state is healthy. Proceed with task.
```

---

### 2.5 Directive (formerly "Task" in guide template)

**Prompt function**: The actual task specification and activation trigger.

The Directive section performs **task activation** -- it tells the model what to
do. This is the section that most directly controls the model's behavior. The
quality of the directive determines whether the session is productive.

**Current spec** (ADR-SEED-004):
> "Next task, acceptance criteria, active guidance corrections."

**What makes a good Directive section**:

1. **Specific first-action.** The directive should include not just the task
   description but a concrete first action. "Implement Store::transact" is a
   task. "Implement Store::transact. First action: write the typestate
   Transaction impl in braid-kernel/src/store.rs" is a directive. The
   first-action cue reduces planning overhead and gets the model into
   productive work faster.

2. **Traceability to SEED.md.** Including the trace ("Traces to: SEED.md SS4
   Axiom 2") is not just bookkeeping -- it is a **cognitive anchor** that
   connects the task to the foundational design rationale. When the model
   encounters ambiguity during implementation, the trace gives it a
   decision-making framework: "which interpretation is more consistent
   with SEED.md SS4?"

3. **Acceptance criteria should be mechanically verifiable.** "Implement
   Store::transact correctly" is subjective. "All proptest properties
   in guide/01-store.md SS1.4 pass, and `cargo test` exits 0" is
   mechanically verifiable. Verifiable criteria activate the model's
   test-writing and verification behaviors.

4. **Active guidance corrections are the anti-drift injection point.**
   If the previous session showed drift patterns (e.g., skipping
   transact calls, not using spec-language), the directive should
   include a targeted correction: "In this session, transact after
   every design decision (INV-STORE-014)." This is the seed's
   contribution to the guidance anti-drift mechanism (GU-007, mechanism 1).

5. **One primary task, not a list.** The directive should specify a single
   primary task with a clear scope. A list of tasks ("implement transact,
   query, and status") causes the model to context-switch, reducing depth
   on each. If multiple tasks are needed, they should be sequenced with
   clear phase boundaries.

**Recommendations**:

- **Budget**: 100-200 tokens. The directive needs enough detail to be
  actionable but not so much that it overwhelms.
- **Structure**: `Task: {what}. Traces to: {SEED.md section}. First action:
  {concrete_step}. Acceptance: {verifiable_criteria}. Correction:
  {anti-drift_nudge_if_applicable}.`
- **Compression**: The directive is the *last* section to compress. Under
  extreme budget pressure, compress State and Constraints before touching
  the Directive.
- **Anti-pattern**: Directives that are too vague ("work on the store")
  cause the model to spend its initial high-quality attention on planning
  rather than execution. Directives that are too specific ("write exactly
  this code") prevent the model from applying judgment. The sweet spot is
  constraint-based: "Implement X satisfying INV-Y, ADR-Z."

**Proposed template** (variable, 100-200 tokens):
```
## Directive
**Task**: Implement `Store::transact()` (INV-STORE-001, INV-STORE-002, INV-STORE-014).
**Traces to**: SEED.md SS4, Axiom 2.
**First action**: Define the `Transaction<Building>` typestate in `braid-kernel/src/store.rs`.
**Acceptance**: All proptest properties in guide/01-store.md SS1.4 pass. `cargo test` exits 0.
**Correction**: Transact after every design decision — do not accumulate unrecorded choices.
```

---

## 3. Cross-Cutting Prompt Optimization Principles

### 3.1 Spec-Language Activates Deeper Reasoning (ADR-SEED-003)

The decision to use spec-language over instruction-language is one of the most
consequential prompt optimization choices in the system. Empirical evidence
(documented in ADR-GUIDANCE-004) shows that spec-language ("INV-STORE-001
requires append-only; current operation would mutate") activates the model's
formal-reasoning substrate, while instruction-language ("Step 1: do not delete
datoms") activates the surface procedural substrate. The formal substrate
produces more robust behavior under context pressure (low k*_eff).

**Implication for seed**: All five sections should use spec-language formatting.
Even the Directive section, which is inherently action-oriented, should frame
actions as constraint satisfaction ("Implement X satisfying INV-Y") rather than
procedure following ("Step 1: write function X. Step 2: add tests.").

### 3.2 The Seed Is the First Guidance (ADR-SEED-001)

The three-concern collapse (awareness + guidance + trajectory = CLAUDE.md) means
the seed is not just context -- it is the first anti-drift intervention. The
model's initial turns are its highest-quality reasoning. If the seed activates
the correct substrate (Basin A) during these turns, the model's own outputs
reinforce Basin A, creating a positive feedback loop. If the seed fails to
activate Basin A, the model's pretrained patterns (Basin B) dominate from the
start, and later guidance interventions must fight uphill.

**Implication for seed**: Front-load the highest-value content. Orientation and
Constraints (first 150-200 tokens) determine which basin captures the
trajectory. These sections should be optimized for activation, not for
completeness.

### 3.3 Budget Compression Should Follow Prompt Value, Not Section Order

Under budget pressure (INV-SEED-002), the system must compress. The current
projection pyramid (pi_0 through pi_3) applies uniformly. A prompt-aware
compression strategy would instead assign per-section compression priorities:

| Section     | Compression priority | Rationale |
|-------------|---------------------|-----------|
| Directive   | Last to compress    | Directly controls behavior |
| Warnings    | Second-last         | Safety-critical, high behavioral leverage |
| Orientation | Third               | Short, identity-activating, mostly fixed |
| Constraints | Fourth              | Reconstructible from store; can be compressed to IDs only |
| State       | First to compress   | Lowest marginal value; reconstructible via queries |

This ordering differs from a naive section-order compression and should be
encoded in the ASSEMBLE pipeline.

### 3.4 Token Allocation by Remaining Budget

The five-part structure should allocate tokens adaptively based on the total
budget available:

| Budget range      | Allocation strategy |
|-------------------|---------------------|
| > 2000 tokens     | Full detail in all sections. pi_0 for State items. |
| 500-2000 tokens   | Compress State to pi_1. Keep Constraints at full IDs. |
| 200-500 tokens    | Orientation only (50 tok) + Directive (100 tok) + top-3 Warnings. No State. |
| <= 200 tokens     | Single-line orientation + single-line directive + harvest warning if applicable. |

This maps directly to the projection pyramid from spec/13-budget.md but applies
it at the seed-section level rather than the entity level.

### 3.5 Demonstration Density in the Seed

INV-GUIDANCE-007 requires at least one demonstration per constraint cluster in
the dynamic CLAUDE.md. The same principle should apply to the seed: where the
Constraints section lists a cluster of related INVs, one worked example showing
what compliance looks like is worth 10x its token cost.

For example, if the seed lists INV-STORE-001 (append-only) and INV-STORE-003
(content-addressed identity), including a single 30-token demonstration:
```
Demonstration: `store.transact(tx)` adds datoms; `store.len()` is non-decreasing.
```
activates the model's pattern-completion substrate far more effectively than
the invariant statements alone.

---

## 4. Gap Analysis: Current Spec vs. Prompt-Optimal Design

| Gap | Current state | Prompt-optimal state | Severity |
|-----|---------------|---------------------|----------|
| Section compression ordering | Not specified | Per-section priority (Directive last, State first) | Medium |
| Commitment weight in Constraints | Mentioned in guide but not in spec template | Explicit `(w=N)` after each constraint | Medium |
| "Do not relitigate" as behavioral nudge | Implicit in ADR-SEED-003 spec-language | Explicit directive after high-weight ADRs | Low |
| First-action cue in Directive | Not in ADR-SEED-004 template | "First action: {concrete_step}" as required field | High |
| Empty Warnings as positive signal | Not specified | "None." output when no warnings active | Low |
| Adaptive token allocation by budget | Projection pyramid exists but is entity-level | Section-level allocation table | Medium |
| Demonstration density in seed | Specified for CLAUDE.md (INV-GUIDANCE-007) but not for seed | At least 1 demonstration per constraint cluster | Medium |
| Warning decay / escalation | Not specified | Warnings appearing 5+ times get escalated or resolved | Low |

---

## 5. Concrete Recommendations Summary

1. **Cap Orientation at 80 tokens.** Use a fixed template with slot-fills. Do not
   generate prose.

2. **Order Constraints by behavioral leverage.** NEG cases first, then
   high-weight ADRs with `(w=N, do not relitigate)`, then relevant INVs.

3. **Compress State first under budget pressure.** State is the lowest-value
   section per token because it is reconstructible.

4. **Make Warnings actionable.** Every warning includes `{what} -- {action}`.
   Empty warnings emit "None." as a positive signal.

5. **Include a first-action cue in every Directive.** "First action: {step}"
   eliminates planning overhead and immediately activates productive work.

6. **Add per-section compression priority to the ASSEMBLE pipeline.** Directive
   and Warnings are last to compress; State is first.

7. **Include one demonstration per constraint cluster.** A 30-token worked
   example is worth 10x its token cost in behavioral activation.

8. **Encode commitment weight as explicit metadata.** `(w=12)` is a stronger
   signal than prose about importance.

---

## 6. Relationship to Existing Spec Elements

| Spec element | How this analysis extends it |
|---|---|
| ADR-SEED-003 (spec-language) | Validated and extended: spec-language should also apply within the Directive |
| ADR-SEED-004 (unified template) | Extended with per-section prompt optimization and compression priorities |
| INV-SEED-002 (budget compliance) | Extended with section-level budget allocation table |
| INV-SEED-005 (relevance) | Extended with deadweight token identification criteria |
| INV-GUIDANCE-007 (dynamic CLAUDE.md) | Extended demonstration density principle to the seed itself |
| ADR-GUIDANCE-002 (basin competition) | Applied: seed is the initial basin selection mechanism |
| ADR-BUDGET-003 (rate-distortion) | Applied at section level, not just entity level |

---

*This analysis treats the seed as a prompt optimization problem constrained by
rate-distortion theory. The five-part structure maps to five distinct prompt
functions, each with its own optimization criteria. The key insight is that
prompt value per token is not uniform across sections -- compression should
follow behavioral leverage, not section order.*
