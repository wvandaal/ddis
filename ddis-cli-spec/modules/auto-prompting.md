---
module: auto-prompting
domain: autoprompt
maintains: [APP-INV-022, APP-INV-023, APP-INV-024, APP-INV-025, APP-INV-026, APP-INV-027, APP-INV-028, APP-INV-029, APP-INV-030, APP-INV-031, APP-INV-032, APP-INV-033, APP-INV-034, APP-INV-035, APP-INV-036, APP-INV-042, APP-INV-045, APP-INV-046]
interfaces: [APP-INV-001, APP-INV-002, APP-INV-003, APP-INV-005, APP-INV-008, APP-INV-009, APP-INV-010, APP-INV-015, APP-INV-016, APP-INV-017, APP-INV-018, APP-INV-020]
implements: [APP-ADR-016, APP-ADR-017, APP-ADR-018, APP-ADR-019, APP-ADR-020, APP-ADR-021, APP-ADR-022, APP-ADR-023, APP-ADR-024, APP-ADR-025, APP-ADR-031, APP-ADR-033]
adjacent: [code-bridge, search-intelligence, query-validation, lifecycle-ops, workspace-ops]
negative_specs:
  - "Must NOT generate prompts that exceed LLM context budget"
  - "Must NOT hide intermediate state from the user"
  - "Must NOT substitute LLM judgment for mechanical verification"
  - "Must NOT prescribe cognitive mode transitions — only observe and classify"
  - "Must NOT treat absorbed spec artifacts as authoritative without human review"
---

# Auto-Prompting Module

The auto-prompting module implements the bilateral specification lifecycle: four self-reinforcing loops that transform DDIS from a static document standard into a living discourse between human intent and machine behavior. `ddis discover` translates ideas into spec. `ddis refine` improves spec quality. `ddis drift` measures spec-implementation correspondence. `ddis absorb` translates implementation back into spec. Together they form a closed cycle where specification is not a one-way decree but a bilateral discourse.

---

## Background: The Bilateral Specification Lifecycle

### The State Monad

The CLI is a state monad. Each command takes state (SQLite + JSONL) and returns `(output, state, guidance)`. The LLM is the interpreter that reads guidance, interacts with the human, and produces the next CLI invocation. The human is the input stream whose natural-language thinking gets translated by the LLM into spec artifacts. The human never needs to learn the spec format --- the LLM authors the spec, the human reviews.

```go
type CommandResult struct {
    Output   string         // what to show the user
    State    StateSnapshot  // current state summary for the LLM
    Guidance Guidance       // light hints for the LLM's next move
}

type StateSnapshot struct {
    ActiveThread     string     // thread ID or "" if no active thread
    Confidence       [5]int     // [coverage, depth, coherence, completeness, formality], each 0-10
    LimitingFactor   string     // what is holding quality back
    OpenQuestions    int        // unresolved questions in active thread
    ArtifactsWritten int        // crystallized artifacts in current workflow
    SpecDrift        float64    // current drift score (0.0 = aligned)
    Iteration        int        // current iteration number in refine/discover loop
    ModeObserved     string     // cognitive mode classification or ""
}

type Guidance struct {
    ObservedMode    string   // from cognition model: {divergent, convergent, dialectical, abductive, metacognitive, incubation, crystallization}
    DoFHint         string   // from gestalt mapping: {very_low, low, mid, high, very_high}
    SuggestedNext   []string // ordered list of 1-3 natural-language suggestions
    RelevantContext []string // spec element IDs relevant to current work: ["INV-004", "ADR-003"]
    TranslationHint string   // one sentence describing what the user seems to be doing
    Attenuation     float64  // 0.0-1.0, how much to shrink guidance relative to first invocation
}
```

### Category-Theoretic Structure

The bilateral specification lifecycle is a category **Spec** whose objects are specification states and whose morphisms are the four loop operations. Each forward-inverse pair forms an adjunction:

```
discover ⊣ absorb          (idea ↔ impl)
parse    ⊣ render           (markdown ↔ index)
tasks    ⊣ traceability     (spec → issues ↔ issues → spec)
refine   ⊣ drift            (improve spec ↔ measure divergence)
witness  ⊣ challenge        (attest ↔ verify)
```

The unit of each adjunction measures round-trip divergence from the identity morphism:

```
η_discover : Id_Spec → absorb ∘ discover      (discovering an idea and absorbing it back should yield the same spec)
η_parse    : Id_Spec → render ∘ parse          (APP-INV-001: byte-identical round-trip)
η_refine   : Id_Spec → drift ∘ refine          (refining then measuring should show improvement)

drift(spec) = ||η(spec) - Id||                  (how far the round-trip diverges from identity)
```

**The persistent manifold metaphor.** The specification is the base manifold --- the accumulated, validated, crystallized state. Each session or thread is a tangent vector at the current spec state, representing a line of exploration that has not yet been projected back. Discovery sessions explore the tangent space; crystallization projects tangent vectors back onto the manifold. Drift measures how far the implementation has moved from the manifold surface. The refine loop contracts the manifold toward the implementation; the absorb loop extends the manifold to cover uncharted implementation territory.

```
ManifoldState = (spec_index, event_streams, thread_topology)

tangent_vector(thread) = thread.events PROJECTED_ONTO spec_elements
crystallize(thread)    = update_spec(spec, tangent_vector(thread))
drift(impl, spec)      = ||impl - project(impl, ManifoldState)||
```

### The Inverse Principle

Every forward operation has an inverse. `discover` (idea->spec) <-> `absorb` (impl->spec). `render` (index->markdown) <-> `parse` (markdown->index). `tasks` (spec->issues) <-> traceability (issues->spec). The system is complete when every arrow has a dual. Drift is the unit measuring how far the round-trip diverges from identity.

### The Four Self-Reinforcing Loops

```
ddis discover:  context → converse → classify → record → (loop until confidence >= 8)
ddis refine:    audit → plan → apply → judge → (loop until drift converges)
ddis drift:     measure → classify → remediate → measure → (loop until drift = 0)
ddis absorb:    scan → prompt → draft → validate → refine → (loop until validate passes)
```

**Invariants interfaced from other modules (INV-018 compliance):**

- APP-INV-001: Round-Trip Fidelity (maintained by parse-pipeline). *The refine apply phase edits spec files surgically; round-trip fidelity ensures these edits don't corrupt surrounding content.*
- APP-INV-002: Validation Determinism (maintained by query-validation). *The refine judge phase compares validation results across iterations; non-deterministic validation makes convergence unmeasurable.*
- APP-INV-003: Cross-Reference Integrity (maintained by query-validation). *Discovery context bundles include cross-reference graphs; broken references produce incomplete context.*
- APP-INV-005: Context Self-Containment (maintained by search-intelligence). *Discovery context bundles extend the existing 9-signal bundle; the self-containment guarantee must hold for the extended bundle.*
- APP-INV-008: RRF Fusion Correctness (maintained by search-intelligence). *Thread matching uses the same search infrastructure; incorrect scores lead to wrong thread selection.*
- APP-INV-009: Monolith-Modular Equivalence (maintained by parse-pipeline). *Absorbed specs may be authored against either form; equivalence guarantees the result is the same regardless of input format.*
- APP-INV-010: Oplog Append-Only (maintained by lifecycle-ops). *Discovery events are recorded in an append-only JSONL stream; the oplog pattern is reused for discovery event sourcing.*
- APP-INV-015: Deterministic Hashing (maintained by parse-pipeline). *Content hashes in discovery event streams enable temporal comparison; non-deterministic hashing makes provenance tracking meaningless.*
- APP-INV-016: Implementation Traceability (maintained by lifecycle-ops). *Absorbed artifacts carry Implementation Trace annotations; Check 13 mechanically verifies them against the source tree.*
- APP-INV-017: Annotation Portability (maintained by code-bridge). *The absorb engine reuses the annotation scanner to extract what code declares before prompting the LLM.*
- APP-INV-018: Scan-Spec Correspondence (maintained by code-bridge). *Absorb's `--against` mode depends on accurate scan-spec correspondence for reconciliation.*
- APP-INV-020: Event Stream Append-Only (maintained by code-bridge). *Discovery events are recorded in Stream 1 (discovery JSONL); the append-only and monotonicity guarantees are inherited.*

---

## CommandResult JSON Schema

Every auto-prompting command returns a `CommandResult` serialized as JSON. The three fields --- `output`, `state`, `guidance` --- provide the state monad triple that the LLM interpreter consumes.

### StateSnapshot Fields

| Field | Type | Description |
|---|---|---|
| `active_thread` | string | Thread ID of the currently active inquiry thread, or `""` if no thread is active |
| `confidence` | `[5]int` | 5-element array `[coverage, depth, coherence, completeness, formality]`, each 0-10. Coverage = how many spec areas are addressed. Depth = how thoroughly each area is explored. Coherence = internal consistency. Completeness = structural completeness (all components present). Formality = semi-formal predicate quality. |
| `limiting_factor` | string | Human-readable description of what is currently holding quality back (e.g., "3 invariants lack violation scenarios") |
| `open_questions` | int | Count of unresolved questions in the active thread's event stream |
| `artifacts_written` | int | Count of crystallized artifacts (invariants, ADRs, negative specs) in the current workflow |
| `spec_drift` | float | Current drift score computed by `ddis drift`. 0.0 means fully aligned. |
| `iteration` | int | Current iteration number (0-indexed) in the refine or discover loop |
| `mode_observed` | string | Cognitive mode classification from the most recent classification event, or `""` if no classification has occurred |

### Guidance Fields

| Field | Type | Description |
|---|---|---|
| `observed_mode` | string | One of `{divergent, convergent, dialectical, abductive, metacognitive, incubation, crystallization}` from the cognition model. Empty string if no classification. |
| `dof_hint` | string | Degrees-of-freedom hint from Gestalt mapping: `{very_low, low, mid, high, very_high}`. Calibrates how constrained the LLM's next response should be. |
| `suggested_next` | `[]string` | Ordered list of 1-3 natural-language suggestions for the LLM's next action. First suggestion is highest priority. |
| `relevant_context` | `[]string` | List of spec element IDs relevant to the current work (e.g., `["APP-INV-022", "APP-ADR-016"]`). Used by the LLM to focus retrieval. |
| `translation_hint` | string | One sentence describing what the user seems to be doing, for LLM orientation (e.g., "user is stress-testing cache invalidation edge cases"). |
| `attenuation` | float | 0.0 to 1.0. How much to shrink guidance relative to the first invocation. 0.0 = full guidance (first call). 1.0 = zero guidance (fully attenuated). Computed from the k\* budget function. |

### Example: `ddis refine audit` Output

```json
{
  "output": "## Audit Report (Iteration 3)\n\n### Drift Summary\n- impl_drift: 4.0 (2 unspecified + 1 unimplemented + 0.5 contradictions)\n- Quality: coverage=7, depth=6, coherence=8, completeness=5, formality=7\n\n### Findings\n1. APP-INV-025 lacks violation scenario (completeness -1)\n2. APP-ADR-019 missing Tests subsection (completeness -1)\n3. Section §3.2 has 0 cross-references (coherence gap)\n\n### Recommended Focus\ncompleteness (limiting factor: 2 invariants missing components)",
  "state": {
    "active_thread": "",
    "confidence": [7, 6, 8, 5, 7],
    "limiting_factor": "2 invariants missing violation scenarios, 1 ADR missing Tests",
    "open_questions": 0,
    "artifacts_written": 0,
    "spec_drift": 4.0,
    "iteration": 3,
    "mode_observed": ""
  },
  "guidance": {
    "observed_mode": "",
    "dof_hint": "low",
    "suggested_next": [
      "Run 'ddis refine plan' to generate a focused improvement plan for completeness",
      "Review APP-INV-025 and add a concrete violation scenario",
      "Check if §3.2 should cross-reference adjacent invariants"
    ],
    "relevant_context": ["APP-INV-025", "APP-ADR-019", "APP-INV-022"],
    "translation_hint": "audit complete — next step is planning focused on the completeness dimension",
    "attenuation": 0.35
  }
}
```

### Example: `ddis discover` Output

```json
{
  "output": "## Discovery Context\n\nResuming thread t-cache-invalidation (confidence: [6,4,7,3,5])\n\nLast activity: 2 days ago — explored TTL strategies, parked question about write-through vs write-behind.\n\n### Active Questions\n1. How should cache warming interact with the TTL policy?\n2. Should invalidation events propagate to subscriber caches?\n\n### Relevant Spec Elements\n- INV-007: Signal-to-Noise (related to cache overhead)\n- ADR-003: Event-Driven Architecture (prior decision)\n\n### Thread Topology\n- t-cache-invalidation (active, 12 events, 2 questions open)\n- t-auth-flow (parked, 8 events, 0 questions)\n- t-rate-limiting (merged, 15 events, 3 artifacts written)",
  "state": {
    "active_thread": "t-cache-invalidation",
    "confidence": [6, 4, 7, 3, 5],
    "limiting_factor": "depth: TTL strategy not fully explored",
    "open_questions": 2,
    "artifacts_written": 0,
    "spec_drift": 2.5,
    "iteration": 0,
    "mode_observed": "divergent"
  },
  "guidance": {
    "observed_mode": "divergent",
    "dof_hint": "very_high",
    "suggested_next": [
      "Explore the interaction between cache warming and TTL expiry",
      "Consider whether subscriber caches need their own invalidation policy",
      "Map failure modes: what happens when the cache warming job crashes mid-warm?"
    ],
    "relevant_context": ["INV-007", "ADR-003", "APP-INV-029"],
    "translation_hint": "user is exploring cache invalidation design space — divergent phase, do not constrain",
    "attenuation": 0.0
  }
}
```

---

## Invariants Maintained by This Module

---

### Refinement Loop Invariants

**APP-INV-022: Refinement Drift Monotonicity**

*Each iteration of the `ddis refine` loop must produce a measurable drift reduction in spec-internal quality metrics. Drift monotonically decreases across iterations within a single refine session. If an iteration increases drift, the system must halt and surface the regression to the user before continuing. Extends INV-022 (Reconciliation Monotonicity) from the parent spec. This invariant applies to spec-internal drift only; code-spec drift (measured by `ddis absorb --against`) is governed separately.*

```
FOR ALL iterations i, i+1 IN refine_loop:
  spec_internal_drift(i+1) <= spec_internal_drift(i)
  IF spec_internal_drift(i+1) > spec_internal_drift(i):
    loop.halted = true AND user_notified = true

WHERE:
  spec_internal_drift = unresolved_xrefs + missing_components + coherence_gaps
  // This is DISTINCT from code_spec_drift = |unspecified| + |unimplemented| + 2*|contradictions|
  // absorb may increase spec_internal_drift temporarily; refine must decrease it
```

Violation scenario: The refine apply phase adds a new invariant to improve depth coverage. The invariant introduces a cross-reference to a non-existent ADR. Drift increases because the new cross-ref is unresolved. The loop continues to the next iteration without noticing, compounding the error.

Validation: Run `ddis refine` for 5 iterations on a spec with known drift. Verify drift is strictly non-increasing. Inject a drift-increasing edit in iteration 3; verify the loop halts with a user-facing regression report.

// WHY THIS MATTERS: An improvement loop that can make things worse is worse than no loop at all. Monotonicity is the contract that justifies automation. The spec-internal vs code-spec distinction prevents absorb from being blamed for refine's contract violation.

---

**APP-INV-023: Prompt Self-Containment**

*Every prompt generated by the refine or discover engines must contain all context needed for the LLM to act. No prompt may depend on implicit context (prior conversation turns, environment variables, or state not explicitly included in the prompt). Prompt size is bounded by the k\* budget function.*

```
FOR ALL prompts p IN generated_prompts:
  p.explicit_context CONTAINS all_referenced_spec_elements
  p.explicit_context CONTAINS all_referenced_drift_data
  p.explicit_context CONTAINS all_relevant_exemplars
  token_count(p) <= k_star_token_target(p.depth)

WHERE:
  k_star_token_target(depth) = {
    depth=0:  ~2000 tokens (k*=12, full framework)
    depth=20: ~1200 tokens (k*=8, mode + context + suggestions)
    depth=45: ~300 tokens  (k*=3, mode only)
  }
```

Violation scenario: The refine audit prompt references "the invariant from the previous iteration" without including the invariant text. The LLM hallucinates an invariant that doesn't exist. The apply phase writes this hallucinated invariant into the spec, introducing a phantom element that has no provenance.

Validation: Extract all prompts from a 5-iteration refine run. For each prompt, verify that every spec element referenced by ID is included verbatim in the prompt context. No prompt may contain dangling references. Verify that no prompt exceeds its k\* token budget.

// WHY THIS MATTERS: Prompts are the interface between the CLI and the LLM interpreter. A prompt with implicit dependencies is a function with hidden parameters --- the output is unpredictable. The k\* budget prevents the opposite failure: prompts so large they dilute the LLM's attention.

---

**APP-INV-024: Ambiguity Surfacing**

*When the refine loop detects unresolved design decisions (missing ADRs, ambiguous invariant specifications, contradictory requirements), these ambiguities must be surfaced to the user. The loop must not resolve ambiguities autonomously.*

```
FOR ALL ambiguities a IN detected_ambiguities:
  a.surfaced_to_user = true
  a.autonomously_resolved = false
```

Violation scenario: The refine audit detects that INV-007 (signal-to-noise) and INV-018 (structural redundancy) could conflict: redundancy adds content but signal-to-noise demands minimalism. The system silently resolves the tension by prioritizing INV-007 and removing restated invariants. This violates INV-018 without the user's knowledge.

Validation: Create a spec with 2 intentional ambiguities (contradictory invariants without an ADR resolving the tension). Run `ddis refine audit`. Verify both ambiguities appear in the output prompt as explicit questions for the user to resolve.

// WHY THIS MATTERS: Ambiguity resolution is a design decision. Design decisions belong to the human, not the tool. The tool surfaces options; the human chooses.

---

### Discovery Loop Invariants

**APP-INV-025: Discovery Provenance Chain**

*Every crystallized artifact (invariant, ADR, glossary entry, negative spec) written by the discovery process has a complete provenance chain in the event stream: from the finding or question that motivated it, through the discussion that refined it, to the crystallization event that committed it.*

```
FOR ALL artifacts a IN discovery.artifact_map:
  chain_connected(a) = true

WHERE chain_connected(a) = true IFF:
  EXISTS ordered sequence [e_1, e_2, ..., e_n] in event_stream:
    e_1.type IN {question_opened, finding_recorded}
    e_n.type = "decision_crystallized" AND e_n.artifact_id = a.id
    FOR ALL consecutive pairs (e_i, e_{i+1}):
      e_i.thread_id = e_{i+1}.thread_id
      e_i.timestamp <= e_{i+1}.timestamp
      e_{i+1}.sequence = e_i.sequence + 1   // no gaps
```

Violation scenario: The LLM writes an ADR during a discovery session but the event recording misses the `decision_crystallized` event. The ADR exists in the spec but has no provenance in the discovery JSONL. When `ddis tasks` generates implementation tasks from the artifact map, this ADR is invisible --- no task is created for it.

Validation: Run a complete discovery session producing 3 artifacts. For each artifact, verify a complete chain exists from a root event to the crystallization event. Delete one intermediate event from the JSONL; verify the provenance chain check reports the gap.

// WHY THIS MATTERS: Provenance is the bridge between organic thinking and formal specification. Without it, artifacts appear to materialize from nowhere --- their rationale is lost, their motivation is untraceable, and their evolution is invisible.

---

**APP-INV-026: Classification Non-Prescriptive**

*The cognitive mode classification layer observes and tags events --- it never prescribes, directs, or constrains the user's cognitive mode. Classification labels are metadata, not directives. The system never tells the user "you should switch to convergent mode" or "this exploration has gone on too long."*

```
FOR ALL classification_events c IN event_stream:
  c.type = "mode_observed"  (not "mode_directed" or "mode_required")
  c.label IN {divergent, convergent, dialectical, abductive,
              metacognitive, incubation, crystallization}
  c.prescription = null     (no directive component)

FOR ALL user_facing_output u IN command_results:
  u.output DOES NOT CONTAIN any mode name string
  // Mode names appear only in state.mode_observed and guidance.observed_mode
  // Never in the human-readable output field
```

Violation scenario: The classification layer detects 5 consecutive divergent events and generates a prompt saying "You've been exploring for a while. Consider narrowing your focus." This seems helpful but violates observation-over-prescription: the user may be in a productive divergent phase where convergence would be premature. The system's suggestion anchors the user toward convergence, potentially cutting off a valuable line of inquiry.

Validation: Run a discovery session with 10 consecutive divergent events. Verify the classification layer tags each event correctly but generates zero prescriptive messages. The user-facing output should contain no mode-transition suggestions. Verify that `CommandResult.Output` never contains the strings "divergent", "convergent", "dialectical", "abductive", "metacognitive", "incubation", or "crystallization".

// WHY THIS MATTERS: The cognition model specifies what modes exist, not when to use them. Prescription destroys the naturalism of discovery --- the moment the system tells you what to think, it stops being a thinking partner and becomes a constraint.

---

**APP-INV-027: Thread Topology Primacy**

*Inquiry threads are the primary organizational unit for discovery events, not sessions. A single session may touch multiple threads; a single thread may span multiple sessions, multiple LLMs, and multiple humans. Sessions are substrate metadata; threads carry the cognitive coherence.*

```
FOR ALL events e IN discovery_stream:
  e.thread_id IS NOT NULL
  e.session_id IS metadata_only

FOR ALL threads t IN thread_topology:
  t.events = {e IN discovery_stream WHERE e.thread_id = t.id}
  t.coherent = true  (events within a thread form a coherent narrative)
```

Violation scenario: A developer starts exploring caching in Session A, switches to authentication mid-session, then returns to caching in Session B. If events are session-scoped, the caching exploration is split across two sessions with authentication events interleaved. Replaying Session A gives an incoherent narrative. Thread-scoping keeps the caching events together in thread t-caching regardless of session boundaries.

Validation: Create a test JSONL with events from 2 sessions, each touching 2 threads. Replay by thread: each thread's events form a coherent narrative. Replay by session: verify session metadata is available but thread grouping is primary.

// WHY THIS MATTERS: The unit of cognitive coherence is the line of inquiry, not the LLM session. Sessions are accidents of tooling; threads are the structure of thought.

---

**APP-INV-028: Spec-as-Trunk**

*Every discovery thread branches from the specification and crystallizes back into it. The specification is the main trunk --- the accumulated, validated representation of all resolved thinking. No orphan threads that bypass spec integration.*

```
FOR ALL threads t IN thread_topology WHERE t.status = "merged":
  EXISTS artifact a IN spec: a.provenance.thread = t.id

FOR ALL threads t IN thread_topology WHERE t.status != "parked":
  t.spec_attachment IS NOT NULL  (thread relates to identifiable spec elements)
```

Violation scenario: A developer creates a thread exploring "API rate limiting" and records 15 events including 3 decisions. But the thread is marked "merged" without any artifacts being written to the spec. The decisions exist only in the JSONL stream --- invisible to validation, invisible to drift measurement, invisible to implementation task generation.

Validation: Create a thread with 3 crystallization events. Mark thread as merged. Verify all 3 artifacts exist in the spec. Create a thread with 0 crystallization events. Attempt to mark as merged; verify the system warns about uncrystallized findings.

// WHY THIS MATTERS: The spec is the single source of truth. Findings that live only in discovery JSONL are invisible to every downstream tool --- validate, drift, tasks, coverage. The spec must absorb everything or the lifecycle has a leak.

---

**APP-INV-029: Convergent Thread Selection**

*The system infers thread attachment from conversation content, never forces the user to declare it. When a user starts discussing a topic, the system matches against existing thread summaries using the search infrastructure (LSI/BM25). User override via `--thread` is always available but never required.*

```
FOR ALL session_starts s:
  IF user provides --thread flag:
    selected_thread = user_specified_thread
  ELSE:
    candidates = ConvergeThread(s.content, active_threads)
    IF candidates.best_score >= 0.4:
      selected_thread = candidates.best_thread  (natural resumption)
    ELSE:
      selected_thread = new_thread(branch_from=spec_trunk)

WHERE:
  ConvergeThread uses:
    combined_score = 0.6 * LSI_similarity + 0.4 * BM25_score
    recency_boost = +0.1 if thread.last_event < 24h ago
    threshold = 0.4 (from IntentCoverageScore "uncovered" boundary)
```

Violation scenario: A developer starts typing about "cache invalidation" but the thread matcher uses exact keyword matching. An existing thread about "TTL-based expiration strategies" --- which is the same topic in different terms --- is not matched. The system creates a duplicate thread, fragmenting the developer's exploration into two disconnected threads.

Validation: Create 3 threads with distinct topics. Start a new session with content related to thread 2, using synonyms (not identical keywords). Verify the system selects thread 2 via LSI semantic similarity. Start a session with genuinely novel content; verify a new thread is created.

// WHY THIS MATTERS: Thread management must be invisible. The moment a user has to remember thread IDs or declare thread context, the system has failed its naturalism promise. The spec + search infrastructure should be sufficient to infer context.

---

**APP-INV-030: Contributor Topology Graceful Degradation**

*When git authorship data is available, discovery context includes contributor topology --- who authored which sections, where mental models overlap, where they silently conflict. When unavailable, discovery proceeds identically minus contributor-specific signals. The degradation path: multi-author (full topology) -> single-author (temporal self-disagreement) -> no-git (skip). No core discovery feature depends on git blame data.*

```
FOR ALL discovery_sessions d:
  IF git_available AND multiple_authors:
    d.context INCLUDES contributor_topology(full)
  ELSE IF git_available AND single_author:
    d.context INCLUDES contributor_topology(temporal_self)
  ELSE:
    d.context EXCLUDES contributor_topology
  d.core_features_functional = true  (regardless of git availability)

WHERE:
  git_available   = os.Stat(".git").err == nil AND exec("git rev-parse --git-dir").exit == 0
  multiple_authors = exec("git log --format='%ae' | sort -u | wc -l").stdout > 1
  single_author   = exec("git log --format='%ae' | sort -u | wc -l").stdout == 1
  no_git          = NOT git_available
```

Violation scenario: A developer runs `ddis discover` on a project with no git history. The contributor topology module crashes with "fatal: not a git repository" and the entire discovery session fails. The developer cannot use discovery without git --- violating the self-containment principle.

Validation: Run `ddis discover` in three environments: (1) multi-author git repo --- verify full contributor topology in context; (2) single-author git repo --- verify temporal self-disagreement analysis; (3) non-git directory --- verify discovery completes successfully with zero errors and zero contributor-related warnings.

// WHY THIS MATTERS: Contributor topology enriches discovery but must never gate it. The self-containment principle demands that every core feature works without external dependencies.

---

### Absorption Loop Invariants

**APP-INV-031: Absorbed Artifacts Validate**

*Every artifact produced by `ddis absorb` must pass `ddis validate`. No syntactically invalid spec output --- the absorption engine produces spec content that conforms to the DDIS parser's expectations.*

```
FOR ALL drafts d IN absorb_output:
  ddis_parse(d).success = true
  ddis_validate(ddis_parse(d)).errors = 0 AT Level 1
```

Violation scenario: The absorption engine asks the LLM to write an invariant from a code pattern. The LLM produces an invariant missing the `Violation scenario:` component. `ddis validate` Check 2 (invariant falsifiability) fails. The absorbed draft is unusable without manual correction --- defeating the purpose of automated absorption.

Validation: Run `ddis absorb` on a Go project with 5 identifiable invariant patterns. Parse and validate the output. Verify Level 1 validation passes (overview exists, invariants defined, ADRs defined).

// WHY THIS MATTERS: Absorption that produces invalid specs creates more work than it saves. The self-correcting loop (validate -> re-prompt -> validate) ensures output quality.

---

**APP-INV-032: Symmetric Reconciliation**

*When `ddis absorb --against <db>` is used, reconciliation reports gaps in both directions: undocumented behavior (code does things the spec doesn't mention) AND unimplemented specification (spec claims things the code doesn't do). Neither direction is privileged.*

```
FOR ALL reconciliation_reports r:
  r.undocumented_behavior = {impl_patterns NOT IN spec_elements}
  r.unimplemented_spec = {spec_elements NOT IN impl_patterns}
  r.behavioral_divergence = {(impl, spec) WHERE impl.behavior != spec.claim}
  |r.undocumented_behavior| + |r.unimplemented_spec| + |r.behavioral_divergence| >= 0
```

Violation scenario: `ddis absorb src/ --against cli.db` only reports what the code does that the spec doesn't mention (undocumented behavior). It never checks whether the spec claims things the code doesn't actually do (unimplemented spec). A developer trusts that absorption captured everything, but 5 spec invariants have no corresponding code --- they're aspirational claims, not implemented reality.

Validation: Create a test spec with 5 invariants and code implementing 3 of them, plus code with 2 additional patterns not in the spec. Run `ddis absorb --against`. Verify: 2 undocumented behaviors reported, 2 unimplemented specs reported, 3 correspondences confirmed.

// WHY THIS MATTERS: Bilateral specification means the code has voice. A reconciliation that only listens to one side is not reconciliation --- it's a monologue.

---

**APP-INV-033: Absorption Format Parity**

*Absorbed specs must be structurally indistinguishable from hand-written specs. The only difference is provenance metadata (which discovery session produced the artifact). Format quality, invariant structure, ADR completeness, and cross-reference density must match the standards of hand-written specs.*

```
FOR ALL absorbed_specs s:
  ddis_validate(s, level=current_spec_level).pass = true
  format_quality(s) >= format_quality(hand_written_reference) * 0.9
```

Violation scenario: `ddis absorb` produces invariants with only a statement and no violation scenario. The absorbed spec passes Level 1 validation but fails Level 2. A developer comparing the absorbed spec to the hand-written CLI spec immediately sees the quality gap. The absorbed spec is treated as a "lesser" artifact, undermining trust in the absorption process.

Validation: Run `ddis absorb` on the ddis-cli source code (~22K LOC). Compare the absorbed draft against the hand-written ddis-cli-spec using `ddis diff`. Verify the absorbed spec achieves Level 2 validation. Verify `exemplar.WeakScore` on absorbed invariants is within 10% of the hand-written invariants' scores.

// WHY THIS MATTERS: If absorbed specs look and feel different from hand-written specs, they'll be treated differently --- reviewed less carefully, trusted less, maintained less. Format parity ensures absorption is a first-class authoring path.

---

### State Monad Invariants

**APP-INV-034: State Monad Universality**

*Every CLI command in an auto-prompting workflow returns `(output, state, guidance)` --- the `CommandResult` type. No command produces output without also producing guidance for the LLM interpreter. The `--prompt-only` flag emits the guidance without executing side effects.*

```
FOR ALL commands c IN {discover, refine audit, refine plan, refine apply,
                       refine judge, absorb, discover status, discover threads}:
  c.return_type = CommandResult
  c.output IS NOT NULL
  c.state IS NOT NULL
  c.guidance IS NOT NULL
  c.prompt_only_mode_available = true

FOR ALL commands c WITH --prompt-only flag:
  c.side_effects = {}  (no database writes, no file writes, no event appends)
  c.guidance IS NOT NULL  (guidance is still emitted)
```

Violation scenario: `ddis refine audit` generates an audit report (output) and updates the state DB (state) but returns no guidance. The LLM interpreter receives the audit report but has no hint about what to do next. The LLM either hallucinates a next step or asks the user --- defeating the purpose of auto-prompting.

Validation: Run each auto-prompting command. Verify the return value includes all three components: non-empty output, valid StateSnapshot, and valid Guidance with at least one SuggestedNext entry. Run each command with `--prompt-only`; verify no database modifications occurred.

// WHY THIS MATTERS: The state monad is the interface contract between the CLI and the LLM. Missing guidance breaks the loop --- the interpreter loses its instructions.

---

**APP-INV-035: Guidance Attenuation**

*The first invocation in a workflow returns heavy guidance (full translation framework, cognitive model summary, domain context). Subsequent invocations return light deltas (mode shift, confidence change, suggested actions). Attention budget decreases over conversation depth --- the k\* guard prevents overprompting.*

```
FOR ALL workflows w:
  guidance_size(w.invocation[0]) > guidance_size(w.invocation[n]) FOR n > 0
  guidance_size(w.invocation[n]) <= k_star_token_target(n)

WHERE:
  k_star_eff(depth) = max(3, 12 - floor(depth / 5))
  k_star_token_target(depth) = {
    k*=12: ~2000 tokens,  k*=8: ~1200 tokens,
    k*=5:  ~600 tokens,   k*=3: ~300 tokens
  }
  attenuation(depth) = 1.0 - (k_star_eff(depth) / 12)
```

Violation scenario: Every `ddis discover` invocation dumps the full translation framework (cognition model summary, Gestalt principles, complete thread topology) regardless of conversation depth. By invocation 10, the LLM's context window is 40% guidance and 60% actual content. The LLM starts ignoring the guidance due to attention dilution (the k\* overprompting threshold has been exceeded).

Validation: Run a 10-invocation discover session. Measure guidance token count at each invocation. Verify invocation 1 guidance is at least 3x larger than invocation 10 guidance. Verify no invocation's guidance exceeds the k\* budget for its conversation depth.

// WHY THIS MATTERS: Overprompting is empirically worse than underprompting (LLM Gestalt Theory Study 6). The guidance must fade as the LLM builds internal context --- heavy initial framing, light subsequent nudges.

---

**APP-INV-036: Human Format Transparency**

*The human never needs to learn the spec format to use discovery. The LLM authors all spec artifacts; the human confirms crystallization. The discovery experience feels like a conversation, not a formatting exercise.*

```
FOR ALL discovery_sessions d:
  d.user_writes_spec_format = false
  d.user_confirms_crystallization = true
  d.artifacts_authored_by = "LLM"
  d.artifacts_reviewed_by = "human"
```

Violation scenario: The discovery session reaches a crystallization point. The system prompts: "Please write the invariant in the following format: `**INV-NNN: Title** ...`" The user must learn the 4-component invariant format, the violation scenario convention, the validation method structure. This is a formatting exercise, not a thinking exercise. The user either struggles with the format or gives up.

Validation: Conduct a discovery session with a test user who has never seen the DDIS spec format. Verify the user produces at least 1 invariant and 1 ADR through natural conversation, with the LLM handling all formatting. The user should not type any DDIS-specific markup (e.g., `**INV-`, `#### Problem`, `Violation scenario:`).

// WHY THIS MATTERS: The spec format is the API contract between the LLM author and the mechanical validator. Humans should interact through the discovery conversation, not through the format. User-friendly means better conversation, not simpler format.

---

## Formal Algorithm Specifications

This section specifies the four core algorithms that power the auto-prompting module. Each algorithm is fully specified with typed inputs, typed outputs, numbered steps, and worked examples with concrete data. These algorithms are the mechanical core --- everything else in this module is orchestration around them.

---

### Algorithm: k\* Attention Budget

The k\* budget controls how much guidance the system emits at each invocation. Too much guidance dilutes the LLM's attention (overprompting); too little leaves the LLM without orientation. The budget decreases monotonically with conversation depth, implementing the empirical finding from Gestalt Study 6 that overprompting is worse than underprompting.

```
Algorithm: KStarBudget
Input:
  conversation_depth: int     -- number of CLI invocations in current workflow (0-indexed)
  base_budget: int = 12       -- ceiling from Gestalt Study 1 (quality peaks at 2-5, no collapse at 12)
  step: int = 5               -- invocations per budget decrement
  floor: int = 3              -- minimum viable guidance (mode + suggested_next + one context element)
Output:
  k_star_eff: int             -- effective attention budget (3-12)
  max_guidance_tokens: int    -- token cap for guidance payload
  attenuation: float64        -- 0.0 (full guidance) to 0.75 (three-quarters attenuated)

Steps:
1. Compute effective budget:
   k_star_eff = max(floor, base_budget - (conversation_depth / step))
   // Integer division: floor(depth / step)

2. Compute token target via linear interpolation:
   max_guidance_tokens = 300 + (k_star_eff - floor) * (2000 - 300) / (base_budget - floor)
   // k*=12 -> 2000 tokens, k*=3 -> 300 tokens

3. Compute attenuation:
   attenuation = 1.0 - (k_star_eff / base_budget)

4. Return (k_star_eff, max_guidance_tokens, attenuation)
```

**Example progression:**

| depth | k\* | tokens | attenuation | Guidance content |
|---|---|---|---|---|
| 0 | 12 | 2000 | 0.00 | Full framework: translation model, mode taxonomy, domain context, exemplars, thread topology |
| 5 | 11 | 1811 | 0.08 | Framework minus taxonomy details |
| 10 | 10 | 1622 | 0.17 | Mode + context + suggestions + relevant exemplars |
| 25 | 7 | 1056 | 0.42 | Mode + context + suggestions |
| 45 | 3 | 300 | 0.75 | Mode observation only + one suggested next action |
| 100 | 3 | 300 | 0.75 | Floor --- never zero (minimum viable guidance always emitted) |

**Token budget calibration per prompt type:**

| Prompt type | tokens\_per\_unit | k\*=12 total | k\*=3 total | Rationale |
|---|---|---|---|---|
| refine audit | ~200 | ~2400 | ~600 | Audit prompts carry drift data + exemplars |
| discover open | ~150 | ~1800 | ~450 | Opening prompts carry thread topology |
| absorb prompt | ~100 | ~1200 | ~300 | Absorption prompts carry code patterns |

// WHY THIS MATTERS: The k\* budget is the only defense against the overprompting failure mode. Without it, every invocation dumps the full framework, and by invocation 10 the LLM's context is mostly guidance noise. The floor at 3 ensures the LLM always receives minimal orientation even in deep conversations.

---

### Algorithm: Cognitive Mode Classification

The classification layer observes cognitive mode from recent events without prescribing transitions (APP-INV-026). This algorithm is purely diagnostic --- its output feeds DoF calibration and is never surfaced as a directive.

```
Algorithm: ClassifyMode
Input:
  event_text: string                 -- content of the most recent event
  recent_events: []Event             -- last 5-10 events in active thread (window)
Output:
  mode: string                       -- one of 7 mode labels
  confidence: float64                -- 0.0-1.0 classification confidence
  evidence: []string                 -- human-readable justification strings
  dof_hint: string                   -- mapped DoF from Gestalt Theory

Steps:
1. EXTRACT lexical signals from event_text and recent_events:
   question_density   = count("?", event_text) / word_count(event_text)
   assertion_density  = count(declarative_markers, event_text) / word_count(event_text)
   hedging_density    = count(hedge_words, event_text) / word_count(event_text)
   enumeration_signal = has_numbered_list(event_text) OR has_bullet_list(event_text)

2. COUNT structural signals across recent_events:
   question_count   = count(e.type = "question_opened" for e in recent_events)
   decision_count   = count(e.type = "decision_crystallized" for e in recent_events)
   challenge_count  = count(contains("but"|"however"|"what about"|"problem with", e.content) for e in recent_events)
   analogy_count    = count(contains("reminds me"|"similar to"|"like in"|"pattern from", e.content) for e in recent_events)
   meta_count       = count(contains("are we"|"should we"|"step back"|"right question", e.content) for e in recent_events)
   artifact_count   = count(e.type = "artifact_written" for e in recent_events)
   gap_duration     = max_time_gap_between_consecutive_events(recent_events)

3. SCORE each mode:
   divergent_score      = question_density * 2 + hedging_density + novelty(event_text, recent_events)
   convergent_score     = assertion_density * 2 + enumeration_signal + specificity(event_text)
   dialectical_score    = challenge_count + count("however"|"but"|"although"|"alternatively", event_text)
   abductive_score      = analogy_count + count("if"|"suppose"|"what_if"|"perhaps", event_text)
   metacognitive_score  = meta_count + count("i_think"|"looking_back"|"the_pattern", event_text)
   incubation_score     = 1.0 IF gap_duration > 30_minutes ELSE 0.0
   crystallization_score = (1.0 IF artifact_count > 0 AND recent_events[-1].type = "artifact_written" ELSE 0.0)
                           + contains_decision_language(event_text) + contains_artifact_structure(event_text)

4. CLASSIFY by dominant signal (priority-ordered for tie-breaking):
   IF crystallization_score > 0 AND last_event.type = "artifact_written":
     mode = "crystallization", confidence = 0.9
     evidence = ["artifact_written event at position " + last_event.index]
   ELSE IF meta_count >= 2:
     mode = "metacognitive", confidence = 0.7
     evidence = [matching meta phrases]
   ELSE IF incubation_score > 0:
     mode = "incubation", confidence = 0.6
     evidence = ["gap of " + gap_duration + " between events"]
   ELSE IF analogy_count >= 2:
     mode = "abductive", confidence = 0.7
     evidence = [matching analogy phrases]
   ELSE IF challenge_count >= 2 AND (question_count > 0 OR decision_count > 0):
     mode = "dialectical", confidence = 0.8
     evidence = [matching challenge phrases + question/decision events]
   ELSE IF question_count >= 3:
     mode = "divergent", confidence = 0.7
     evidence = [question_opened event IDs]
   ELSE IF decision_count >= 1 OR contains("let's go with"|"decided"|"choosing", event_text):
     mode = "convergent", confidence = 0.7
     evidence = [decision events or convergent phrases]
   ELSE:
     mode = "convergent", confidence = 0.4  // safe default
     evidence = ["no dominant signal detected"]

5. APPLY momentum guard:
   IF max(all_scores) < confidence_threshold (0.5):
     mode = previous_event.mode  // don't flip on weak signal
     confidence = 0.3
     evidence += ["momentum: weak signal, retaining previous mode"]

6. MAP mode to DoF hint (from Gestalt Theory):
   divergent      -> very_high    (maximize exploration space)
   convergent     -> low          (narrow to decisions)
   dialectical    -> high         (allow challenge + synthesis)
   abductive      -> very_high    (maximize analogical space)
   metacognitive  -> high         (allow reframing)
   incubation     -> (no prompt)  (gap detected, do not interrupt)
   crystallization -> very_low    (minimize — artifact is forming)

7. Return (mode, confidence, evidence, dof_hint)
```

// WHY THIS MATTERS: Mode classification enables the system to calibrate its guidance intensity without telling the user what to think. A divergent user gets wide-open suggestions; a converging user gets focused constraints. The momentum guard prevents classification noise from causing jarring DoF swings between invocations.

---

### Algorithm: Convergent Thread Selection

Thread selection infers the appropriate thread from conversation content (APP-INV-029). The user never declares a thread unless they want to override.

```
Algorithm: ConvergeThread
Input:
  user_content: string                   -- text from current interaction
  thread_summaries: []ThreadSummary      -- all active/parked threads with summaries
  search_infra: SearchEngine             -- BM25 + LSI search infrastructure (from search-intelligence)
Output:
  selected_thread: ThreadID | NEW        -- existing thread ID or signal to create new
  match_score: float64                   -- confidence in the selection (0.0-1.0)
  method: string                         -- "user_override" | "convergent" | "new_thread"

Steps:
1. CHECK for explicit override:
   IF user provides --thread flag:
     return (user_specified_thread, 1.0, "user_override")

2. COMPUTE similarity scores against each thread:
   candidates = []
   FOR EACH thread IN thread_summaries WHERE thread.status != "merged":
     // BM25 lexical score: catches exact terminology matches
     bm25_score = search_infra.BM25(user_content, thread.summary + thread.last_10_events_text)
     // LSI semantic score: catches synonym-based topic matches
     lsi_score = search_infra.LSI(user_content, thread.summary + thread.last_10_events_text)
     // RRF fusion with LSI weighted higher (semantic > lexical for topic matching)
     combined = 0.6 * lsi_score + 0.4 * bm25_score
     // Recency boost: threads active in last 24h get +0.1
     IF thread.last_event_time > now() - 24h:
       combined += 0.1
     candidates.append((thread.id, combined))

3. SELECT best match:
   best = max(candidates, key=score)
   IF best.score >= 0.4:  // threshold from IntentCoverageScore "uncovered" boundary
     Log: {type: "thread_resumed", thread_id: best.id, score: best.score, method: "convergent"}
     return (best.id, best.score, "convergent")

4. CREATE new thread:
   new_id = generate_thread_id(user_content)
   new_thread = Thread{
     id: new_id,
     status: "active",
     summary: extract_topic(user_content),
     spec_attachment: identify_related_spec_elements(user_content, search_infra),
     branch_from: "spec_trunk",
   }
   Log: {type: "thread_created", thread_id: new_id, initial_content_hash: sha256(user_content)}
   return (new_id, 0.0, "new_thread")
```

**Worked example:**

User starts typing about "cache invalidation." Three existing threads:

| Thread | Summary | BM25 | LSI | Combined | Recency |
|---|---|---|---|---|---|
| t-ttl-strategies | "Exploring TTL-based expiration strategies" | 0.35 | 0.72 | 0.572 | +0.1 = 0.672 |
| t-auth-flow | "Authentication token refresh flow" | 0.05 | 0.12 | 0.092 | 0.0 |
| t-rate-limiting | "API rate limiting and backpressure" | 0.10 | 0.18 | 0.148 | 0.0 |

Best match: t-ttl-strategies at 0.672 (above 0.4 threshold). Selected via LSI semantic similarity --- "cache invalidation" and "TTL-based expiration" are the same domain despite different vocabulary.

// WHY THIS MATTERS: Thread fragmentation is the silent killer of discovery coherence. Without convergent selection, a developer exploring the same topic across sessions creates N disconnected threads instead of one rich thread. The 0.4 threshold balances false merges (too low) against false splits (too high).

---

### Algorithm: Exemplar Selection for Prompt Context

The refine apply phase uses exemplars (demonstrations > constraints, per Gestalt Study 2). This algorithm selects the most relevant exemplars for a given quality dimension.

```
Algorithm: SelectExemplars
Input:
  quality_dimension: string            -- the focused lens for this iteration (one of 5 dimensions)
  spec_index: DB                       -- parsed specification database
  k_budget: int                        -- from k* budget function (controls exemplar count)
Output:
  exemplars: []Exemplar                -- 1-3 demonstrations with annotations
  selection_rationale: []string        -- why each exemplar was chosen

Steps:
1. IDENTIFY target quality dimension mapping:
   dimension_to_query = {
     "coverage"     -> exemplars showing comprehensive invariant sets with broad section coverage
     "depth"        -> exemplars showing rich violation scenarios + semi-formal predicates with WHERE clauses
     "coherence"    -> exemplars showing dense cross-reference networks (3+ xrefs per element)
     "completeness" -> exemplars showing all 4 INV components or all 5 ADR subsections
     "formality"    -> exemplars showing mathematical notation in semi-formal predicates (FOR ALL, EXISTS)
   }

2. QUERY spec_index for candidate elements:
   candidates = spec_index.QueryByDimension(quality_dimension, limit=10)
   // Returns elements scored by relevance to the dimension

3. SCORE candidates using WeakScore (reused from internal/coverage/):
   scored = []
   FOR EACH candidate IN candidates:
     score = exemplar.WeakScore(candidate, quality_dimension)
     IF score >= 0.6:
       scored.append((candidate, score))

4. SELECT diverse subset (max count = min(3, k_budget / 4)):
   max_count = min(3, k_budget / 4)
   // Ensure diversity: no two exemplars from the same section type
   selected = []
   used_types = set()
   FOR EACH (candidate, score) IN sorted(scored, key=score, reverse=true):
     IF candidate.element_type NOT IN used_types:
       selected.append(candidate)
       used_types.add(candidate.element_type)
     IF len(selected) >= max_count:
       break
   // Fallback: if fewer than 1 above threshold, use highest-scoring regardless
   IF len(selected) == 0 AND len(candidates) > 0:
     selected = [candidates[0]]

5. ANNOTATE each selected exemplar:
   FOR EACH exemplar IN selected:
     exemplar.annotation = "This is a strong example because: " + explain_score(exemplar, quality_dimension)
     // e.g., "This invariant has all 4 components including a concrete violation scenario
     //         with specific function names and line numbers."

6. Return (selected, [e.annotation for e in selected])
```

**Worked example:** For `quality_dimension = "completeness"`, querying the DDIS CLI spec:

- Candidate 1: APP-INV-006 (Transaction State Machine) --- WeakScore 0.92 (all 4 components + implementation trace + confidence level). Type: invariant.
- Candidate 2: APP-ADR-007 (JSONL Oplog Format) --- WeakScore 0.88 (all 5 ADR subsections + WHY NOT annotations). Type: ADR.
- Candidate 3: APP-INV-010 (Oplog Append-Only) --- WeakScore 0.90 (all 4 components). Type: invariant. SKIPPED (type "invariant" already used).
- Candidate 4: Negative spec "DO NOT modify oplog records" --- WeakScore 0.75 (3 lines with validation reference). Type: negative_spec.

Selected: [APP-INV-006 (invariant), APP-ADR-007 (ADR), negative spec (negative_spec)] --- one per type, all above 0.6 threshold.

// WHY THIS MATTERS: Demonstrations are empirically more effective than constraints for LLM output quality (Gestalt Study 2: demo 4/4 deep vs constraints 3/4, 33% fewer tokens). Showing the LLM what "good" looks like produces better results than listing rules about goodness.

---

## Discovery Event Schema

Discovery events are recorded in an append-only JSONL stream (`discovery/events.jsonl`), inheriting the append-only guarantee from APP-INV-010 and APP-INV-020. Each event carries both structural metadata (thread, session, timestamp, sequence) and cognitive metadata (mode, dialectical move).

### Event Envelope

```json
{
  "version": 1,
  "type": "<event_type>",
  "timestamp": "<RFC3339 UTC>",
  "thread_id": "<thread identifier>",
  "session_id": "<session identifier>",
  "sequence": "<monotonic integer within thread>",
  "data": { ... }
}
```

### Event Types and Payloads

| Type | Data Fields | Description |
|---|---|---|
| `session_started` | `llm`, `human`, `spec_hash` | Marks the beginning of a discovery session |
| `mode_observed` | `mode`, `score`, `signals` | Classification output (observation, not prescription) |
| `finding_recorded` | `finding`, `source`, `confidence` | A factual observation or insight |
| `question_opened` | `question`, `blocking` | An unresolved question requiring exploration |
| `question_closed` | `question_ref`, `resolution` | A previously opened question resolved |
| `challenge_posed` | `challenge`, `target_finding` | A dialectical challenge to a prior finding |
| `decision_crystallized` | `artifact_type`, `artifact_id`, `title`, `provenance_chain` | A decision solidified into a spec artifact |
| `thread_parked` | `reason`, `open_questions`, `artifacts_crystallized` | Thread suspended for incubation |
| `thread_resumed` | `resume_reason`, `previous_park_time` | Thread reactivated from parked state |

### Concrete Event Stream Example

```jsonl
{"version":1,"type":"session_started","timestamp":"2026-02-24T10:00:00Z","thread_id":"t-cache-invalidation","session_id":"s-abc123","data":{"llm":"claude-opus-4-6","human":"wvandaal","spec_hash":"a1b2c3..."}}
{"version":1,"type":"mode_observed","timestamp":"2026-02-24T10:01:30Z","thread_id":"t-cache-invalidation","session_id":"s-abc123","sequence":1,"data":{"mode":"divergent","score":0.82,"signals":{"question_density":0.15,"hedging_density":0.08}}}
{"version":1,"type":"finding_recorded","timestamp":"2026-02-24T10:05:00Z","thread_id":"t-cache-invalidation","session_id":"s-abc123","sequence":2,"data":{"finding":"TTL-based invalidation has edge cases with write-through caching","source":"user_statement","confidence":6}}
{"version":1,"type":"question_opened","timestamp":"2026-02-24T10:07:00Z","thread_id":"t-cache-invalidation","session_id":"s-abc123","sequence":3,"data":{"question":"Should cache invalidation be eventual or immediate?","blocking":true}}
{"version":1,"type":"challenge_posed","timestamp":"2026-02-24T10:10:00Z","thread_id":"t-cache-invalidation","session_id":"s-abc123","sequence":4,"data":{"challenge":"Event-driven invalidation requires delivery guarantees the system doesn't have","target_finding":"finding@seq2"}}
{"version":1,"type":"decision_crystallized","timestamp":"2026-02-24T10:15:00Z","thread_id":"t-cache-invalidation","session_id":"s-abc123","sequence":5,"data":{"artifact_type":"adr","artifact_id":"ADR-042","title":"Eventual Invalidation over Immediate","provenance_chain":["finding_recorded@seq2","question_opened@seq3","challenge_posed@seq4"]}}
{"version":1,"type":"thread_parked","timestamp":"2026-02-24T10:20:00Z","thread_id":"t-cache-invalidation","session_id":"s-abc123","sequence":6,"data":{"reason":"incubation","open_questions":1,"artifacts_crystallized":1}}
```

// WHY THIS MATTERS: The event schema is the provenance backbone. Every crystallized artifact (APP-INV-025) traces back through this stream. The schema must be rich enough to reconstruct the full reasoning chain, yet simple enough to append atomically. The `sequence` field within each thread guarantees gap detection for provenance chain integrity.

---

## Absorb-Refine Drift Interaction

A critical interaction exists between `ddis absorb` and `ddis refine`. Absorb can increase one kind of drift while refine requires monotonic decrease in another. The resolution: **they target different drift metrics and are complementary, not conflicting.**

```
refine targets:  spec_internal_drift  (unresolved refs, missing components,
                                        coherence gaps WITHIN the spec)
absorb targets:  code_spec_drift      (undocumented behavior, unimplemented spec,
                                        behavioral divergence BETWEEN code and spec)

refine monotonicity (APP-INV-022):
  spec_internal_drift(i+1) <= spec_internal_drift(i)

absorb may increase spec_internal_drift temporarily:
  absorb adds new content -> new cross-references may be unresolved
  -> spec_internal_drift increases

Resolution sequence:
  1. absorb: code_spec_drift decreases (new content documents behavior)
  2. refine: spec_internal_drift decreases (resolves refs introduced by absorb)
  3. Net effect: total_drift = spec_internal_drift + code_spec_drift decreases
```

APP-INV-022 applies only to the `refine` loop. Within a single refine iteration, spec-internal drift must not increase. After an absorb phase introduces new content, a fresh refine loop starts with the new baseline --- the monotonicity constraint resets. This is not a loophole; it reflects the two-loop structure: absorb widens the spec's surface area, refine polishes that surface.

---

## Provenance Chain Connectedness

Every crystallized artifact must have a complete provenance chain (APP-INV-025). The `chain_connected` predicate formalizes what "complete" means:

```
chain_connected(artifact) = true IFF:
  EXISTS ordered sequence [e_1, e_2, ..., e_n] in event_stream WHERE:
    e_1.type IN {question_opened, finding_recorded}        // root event
    e_n.type = "decision_crystallized"
      AND e_n.artifact_id = artifact.id                    // terminal event
    FOR ALL consecutive pairs (e_i, e_{i+1}):
      e_i.thread_id = e_{i+1}.thread_id                   // same thread
      e_i.timestamp <= e_{i+1}.timestamp                   // temporal ordering
      e_{i+1}.sequence = e_i.sequence + 1                  // no gaps in sequence
```

A provenance chain must be **continuous**: no missing events between root and terminal. If event `e_3` is deleted or corrupted, the chain breaks at position 3 even if `e_4` through `e_n` are intact. This is verified by checking that sequence numbers are consecutive within the thread.

---

## Architecture Decision Records

---

### APP-ADR-016: Auto-Prompting over Manual Prompting

#### Problem
Should the CLI generate prompts for LLMs automatically or rely on users to craft prompts manually?

#### Options

A) **Auto-prompting with escape hatch** --- CLI generates context-rich prompts from spec state, drift data, and exemplars. Users override with `--prompt-only`.
- Pros: Encodes Gestalt best practices. Deterministic for given state. Low barrier to entry.
- Cons: Generated prompts may not match expert intent. Additional complexity.

B) **Manual-only** --- Users craft all prompts.
- Pros: Full control. No prompt generation logic.
- Cons: Requires deep familiarity with spec format AND Gestalt principles. No reproducibility.

#### Decision
**Option A: Auto-prompting with human escape hatch.** The CLI generates context-rich prompts from spec state, drift data, and exemplars. Users can override with `--prompt-only` (inspect the generated prompt) or manual prompting. The auto-prompting loop is the default path; manual prompting is the power-user path.

// WHY NOT Manual-only? Manual prompting requires deep familiarity with both the spec format and Gestalt optimization principles. Auto-prompting encodes best practices into the tool.

#### Consequences
- `internal/refine/` and `internal/discover/` packages generate prompts
- `--prompt-only` flag on all auto-prompting commands
- Prompt quality is testable (prompts are deterministic for a given state)

#### Tests
- `ddis refine audit --prompt-only` produces identical output for identical database state
- Generated prompts include all 9 intelligence signals when available
- `--prompt-only` flag available on discover, refine, and absorb commands

---

### APP-ADR-017: Gestalt Theory Integration

#### Problem
How should LLM Gestalt Theory principles be applied to auto-prompting?

#### Options

A) **Structural integration** --- Demonstrations > constraints, spec-first framing, DoF separation, k\* guard embedded in prompt generation.
- Pros: +3-4 quality points from spec-first framing alone (Study 7). Empirically validated.
- Cons: Additional prompt engineering complexity. Requires maintaining Gestalt parameter tables.

B) **Ignore Gestalt** --- Generate prompts without optimization theory.
- Pros: Simpler prompt generation. Fewer parameters.
- Cons: Measurably weaker output quality per empirical studies.

#### Decision
**Option A: Structural principles applied to prompt generation.** Demonstrations > constraints (Study 2). Spec-first framing applied recursively (Study 7). DoF separation per iteration. k\* overprompting guard. These are embedded in the prompt generation logic, not exposed as user-facing configuration.

// WHY NOT Ignore Gestalt? Empirical evidence shows +3-4 quality points from spec-first framing alone. Ignoring available optimization research makes the tool strictly weaker.

#### Consequences
- Each `apply` prompt includes worked exemplars (demonstrations > constraints)
- DoF separation: each refine iteration focuses on ONE quality dimension
- k\* budget: prompts capped at token budget proportional to conversation depth
- Constraint removal test: unused prompt sections are removed

#### Tests
- Refine prompts include at least one worked exemplar per quality dimension
- k\* guard triggers when prompt exceeds budget threshold (verified with known-large specs)
- DoF separation: each iteration's prompt addresses exactly one quality dimension

---

### APP-ADR-018: Observation over Prescription

#### Problem
Should the cognitive mode classification layer direct the user's thinking or just observe it?

#### Options

A) **Observational** --- Classify modes post-hoc, use for internal DoF calibration, never surface to user.
- Pros: Preserves naturalism. User cognitive autonomy intact. Labels inform prompts silently.
- Cons: Users cannot see or correct misclassification.

B) **Prescriptive** --- Actively suggest mode transitions to users.
- Pros: Could accelerate convergence. Explicit mode management.
- Cons: Destroys naturalism. Constrains rather than assists. Users resent being directed.

#### Decision
**Option A: Observe and classify, never prescribe.** The classification layer tags events with cognitive mode and dialectical move labels. These labels inform prompt generation (mode-appropriate DoF) but are never presented as directives to the user. Suggestions emerge from post-hoc reflection, not real-time direction.

// WHY NOT Prescriptive? Prescription destroys naturalism. The moment the system says "switch to convergent mode" it becomes a constraint, not a partner. The user's cognitive autonomy is non-negotiable.

#### Consequences
- APP-INV-026: classification layer is observational only
- Mode labels are internal metadata, not user-facing messages
- Prompt generation uses mode labels for DoF calibration but never mentions modes to the user

#### Tests
- No auto-prompting output contains cognitive mode names in user-facing text
- Mode labels present in `CommandResult.State` but absent from `CommandResult.Output`
- Classification changes do not alter user-visible output format

---

### APP-ADR-019: Threads over Sessions

#### Problem
What is the primary scoping unit for discovery events?

#### Options

A) **Inquiry threads** --- Directed lines of investigation that span sessions, LLMs, and humans.
- Pros: Captures cognitive coherence. Enables branch/merge/park/fork topology.
- Cons: Thread identity must be inferred or declared. Additional complexity.

B) **Sessions** --- Events scoped to the session (LLM instance + time window) that produced them.
- Pros: Simple. Naturally bounded. No inference needed.
- Cons: Sessions are accidents of tooling (context window limits, breaks). Cognitive coherence doesn't respect session boundaries.

#### Decision
**Option A: Inquiry threads.** A thread is a directed line of investigation. Sessions are substrate metadata (which LLM, which timestamp range, which human). A single session may touch multiple threads; a single thread may span sessions, LLMs, and humans. Thread topology (branch, merge, park, fork, converge) captures the structure of thought.

// WHY NOT Sessions? Sessions are accidents of tooling --- they end when the context window fills, the LLM changes, or the human takes a break. Cognitive coherence doesn't respect these boundaries.

#### Consequences
- Every event carries `thread_id` as primary scope + `session_id` as metadata
- Thread lifecycle: branch, merge, park, resume, fork, converge
- Replay by thread produces coherent narratives; replay by session may not

#### Tests
- Events from a single thread spanning 2+ sessions replay as coherent narrative
- Thread merge produces combined event stream with preserved ordering
- `ddis discover status` groups by thread, not session

---

### APP-ADR-020: Conversational over Procedural

#### Problem
Should `ddis discover` expose explicit subcommands (start, explore, answer, decide, risks, probe, map) or present a single conversational entry point?

#### Options

A) **Conversational** --- Single `ddis discover` command with context-aware prompt generation.
- Pros: Natural flow. Mode transitions emerge from discourse. Low ceremony.
- Cons: Less explicit. Power users may want direct access to specific operations.

B) **Procedural** --- Explicit subcommands for each discovery phase.
- Pros: Clear state machine. Predictable workflow.
- Cons: Forces users to declare modes. Prescriptive. Violates APP-ADR-018.

#### Decision
**Option A: Conversational.** Single `ddis discover` command. The system loads context, converges on thread and mode during conversation. Subcommands become optional accelerator commands for power users, not the primary path. The experience should feel like resuming a conversation with a collaborator who shares your full context.

// WHY NOT Procedural? The plan's own creation spanned 5+ threads across multiple sessions and LLMs. At no point did participants declare "I am now in dialectical mode." Mode transitions emerged naturally from discourse. Forcing users to declare modes would be prescriptive.

#### Consequences
- `ddis discover` is ONE command with optional flags
- `ddis discover status`, `ddis discover threads`, `ddis discover park`, `ddis discover merge` are accelerators
- Old subcommands (explore, answer, decide, risks, probe, map) become internal classification events

#### Tests
- `ddis discover` with no subcommand generates a valid prompt
- Accelerator commands (`status`, `threads`, `park`, `merge`) accessible as flags
- Mode classification happens internally without user-facing declarations

---

### APP-ADR-021: Contributor Topology via Git Blame

#### Problem
In multi-contributor projects, how should the system surface epistemic structure?

#### Options

A) **Git blame with graceful degradation** --- Extract per-section authorship, surface cross-pollination and silent disagreements.
- Pros: Catches epistemic incoherence invisible to structural validation. Enriches discovery context.
- Cons: Requires git history. Performance cost on large repos.

B) **Ignore contributors** --- Treat spec as anonymous artifact.
- Pros: Simpler. No git dependency.
- Cons: Misses mental model conflicts between contributors. Two contributors can write individually sound sections that together create invisible contradictions.

#### Decision
**Option A: Git blame with graceful degradation.** Use `git blame --porcelain` to extract per-section authorship. Build contributor-section affinity map. Surface cross-pollination opportunities (threads from contributor A related to sections authored by contributor B) and silent disagreements (different contributors using incompatible mental models for the same concepts). Degrade gracefully: multi-author -> temporal self-disagreement -> skip.

// WHY NOT Ignore contributors? Structural validation catches syntactic contradictions but not epistemic incoherence --- two contributors can write individually sound sections that together create invisible mental model conflicts.

#### Consequences
- `contrib.go` in `internal/discover/`
- Contributor topology as 11th intelligence signal in discovery context bundle
- APP-INV-030: graceful degradation guarantees

#### Tests
- Multi-author repo: contributor topology populated with >=2 authors
- Single-author repo: degrades to temporal self-disagreement (early vs late sections)
- No-git directory: topology skipped, no error, context bundle generated without it

---

### APP-ADR-022: State Monad Architecture

#### Problem
How should the CLI interact with LLMs during auto-prompting workflows?

#### Options

A) **State monad** --- CLI returns `(output, state, guidance)`. LLM is the interpreter. CLI remains pure.
- Pros: Inspectable. Deterministic. Provider-agnostic. Testable.
- Cons: Requires LLM-side orchestration. State snapshot design is non-trivial.

B) **Prompt-only** --- CLI generates prompts with no structured feedback.
- Pros: Simpler. No state management.
- Cons: LLM loses context between invocations. No convergence guarantee.

C) **Full agent** --- CLI embeds LLM runtime, manages conversation internally.
- Pros: Self-contained. No external orchestration needed.
- Cons: Non-deterministic. Non-inspectable. Provider-locked. Violates CLI purity.

#### Decision
**Option A: State monad.** The CLI returns `(output, state, guidance)` --- the `CommandResult` type. The LLM is the interpreter; the human is the input stream. This keeps the CLI pure (no LLM dependency in the binary), makes each interaction inspectable (`--prompt-only`), and lets any LLM serve as the interpreter. The Cognition Model specifies the human half of the translation; Gestalt Theory specifies the LLM half.

// WHY NOT Prompt-only (no structured feedback)? Without state and guidance, the LLM loses context between invocations. Each call becomes independent rather than part of a converging loop.

// WHY NOT Full agent (CLI embeds LLM runtime)? Embedding an LLM runtime makes the CLI non-deterministic, non-inspectable, and dependent on a specific provider. The state monad preserves the CLI's purity.

#### Consequences
- `CommandResult`, `StateSnapshot`, `Guidance` types in `internal/autoprompt/types.go`
- All auto-prompting commands return `CommandResult`
- `--prompt-only` flag emits guidance without side effects

#### Tests
- Every auto-prompting command returns a valid `CommandResult` with all three fields populated
- `--prompt-only` produces output but makes no database writes
- `CommandResult` is JSON-serializable and round-trips faithfully

---

### APP-ADR-023: LLMs as Primary Spec Authors

#### Problem
Is the spec format designed for human authoring or LLM authoring?

#### Options

A) **LLM-first** --- Rigorous format serves as API contract between LLM author and mechanical validator.
- Pros: Machine-parseable. Consistent quality. Enables validation. Matches actual authorship pattern.
- Cons: Format is verbose for human editing. Steep manual authoring curve.

B) **Human-first** --- Simplified format optimized for manual authoring.
- Pros: Lower barrier to entry. More intuitive for humans.
- Cons: Harder to validate mechanically. Doesn't match actual authorship pattern (every spec in this project was LLM-authored).

#### Decision
**Option A: LLM-first.** The rigorous format (4-component invariants, 5-subsection ADRs, section numbering) is the API contract between the LLM author and the mechanical validator. Humans review specs; LLMs write them. User-friendliness means better conversation, not simpler format. The spec format enables machine-parseable validation --- that's its purpose.

// WHY NOT Human-first? Every spec in this project was authored by LLMs during conversational sessions. No human manually wrote a 4-component invariant. The format serves the validator, not the author.

#### Consequences
- Progressive validation (APP-ADR-028) groups checks by maturity rather than simplifying format
- `ddis absorb` and `ddis discover` become the primary spec authoring paths
- Agent surface (skill derivation) becomes more important than human documentation

#### Tests
- `ddis discover` output passes `ddis validate` at Level 1 minimum
- `ddis absorb` output is structurally indistinguishable from hand-authored spec content
- `ddis skeleton` generates templates that LLMs can populate without format errors

---

### APP-ADR-024: Bilateral Specification / The Inverse Principle

#### Problem
Specification flows from human intent to formal spec to implementation. But implementation reveals knowledge the spec doesn't capture. How does the code speak back?

#### Options

A) **Bilateral** --- Every forward operation has an inverse. Four loops form a closed cycle.
- Pros: Code gets a voice. Captures implicit knowledge. Complete round-trip. Category-theoretic elegance.
- Cons: Absorption is complex. Reconciliation requires semantic analysis.

B) **Unidirectional** --- Spec is authoritative. Code is subordinate. Drift only flows spec->impl.
- Pros: Simpler model. Clear authority hierarchy.
- Cons: Loses implicit invariants (assertions), implicit ADRs (patterns), implicit negative specs (error handling). Code knowledge is invisible.

#### Decision
**Option A: Bilateral.** Every forward operation has an inverse. `ddis absorb` (impl->spec) is the inverse of `ddis discover` (idea->spec). Together, specification becomes a dialogue, not a decree. The four loops (discover, refine, drift, absorb) form a closed cycle. In category theory: each forward-inverse pair is an adjunction; drift is the unit measuring round-trip divergence from identity.

// WHY NOT Unidirectional? A unidirectional lifecycle treats the spec as authoritative and the code as subordinate. But the code has knowledge: implicit invariants (assertions), implicit ADRs (architectural patterns), implicit negative specs (error handling). `absorb` gives the code voice.

#### Consequences
- `internal/absorb/` package (scan, prompt, draft, reconcile)
- `ddis absorb <code-root> [--against <db>] [--output <spec.md>]`
- Four-loop bilateral architecture replaces three-loop triad
- `--against` mode enables behavioral drift detection (stronger than structural drift)

#### Tests
- `ddis absorb` on annotated codebase produces valid spec fragments
- Absorbed fragments pass `ddis validate` at Level 1
- Round-trip: discover->implement->absorb produces spec equivalent to original (modulo formatting)

---

### APP-ADR-025: Heuristic Scan over AST Parsing

#### Problem
How should `ddis absorb` extract implementation patterns from source code?

#### Options

A) **Regex + LLM analysis** --- Annotation scanner plus heuristic patterns. LLM handles semantic analysis.
- Pros: Language-agnostic. Builds on existing annotation system (APP-ADR-012). Portable.
- Cons: Less precise than AST. Misses some structural patterns.

B) **AST parsing** --- Language-specific parsers for richer extraction.
- Pros: More precise extraction. Captures full language semantics.
- Cons: Requires per-language parsers. Violates portability (APP-INV-017). Maintenance burden scales with supported languages.

#### Decision
**Option A: Regex + LLM analysis, no language-specific parsers.** The annotation scanner (APP-ADR-012) provides the structural extraction. Additional heuristic patterns (assertions -> candidate invariants, error returns -> candidate negative specs, interface declarations -> candidate module boundaries) extend the scanner. The LLM does the semantic analysis --- interpreting patterns as spec-level concepts.

// WHY NOT AST parsing? Language-specific AST parsers (Go AST, TypeScript AST, etc.) provide richer extraction but violate the portability principle (APP-INV-017). The annotation system already solves cross-language extraction; the absorption engine builds on it.

#### Consequences
- `internal/absorb/scan.go` reuses `internal/annotate/` for extraction
- Heuristic patterns are configurable per language family
- LLM prompt includes code context + skeleton templates + exemplar demonstrations

#### Tests
- Heuristic scan on Go source extracts assertions as candidate invariants
- Heuristic scan on Python source extracts the same pattern categories as Go
- Extracted patterns include source location (file:line) for traceability

---

## Implementation Chapters

---

### Chapter: Refine Engine

**Preserves:** APP-INV-022 (Refinement Drift Monotonicity --- drift must not increase), APP-INV-023 (Prompt Self-Containment --- all context included), APP-INV-024 (Ambiguity Surfacing --- ambiguities are never silently resolved).

**Interfaces:** APP-INV-001 (Round-Trip Fidelity --- apply edits must preserve content), APP-INV-002 (Validation Determinism --- judge compares deterministic results), APP-INV-003 (Cross-Reference Integrity --- audit uses cross-ref graph for drift measurement).

The refine engine powers `ddis refine`. It implements the RALPH (Recursive Autonomous Language Protocol Heuristic) improvement cycle as a 4-phase loop: audit, plan, apply, judge. Each phase generates a prompt for the LLM interpreter and returns a `CommandResult`. The drift monotonicity guard (APP-INV-022) is the loop's central safety mechanism.

#### Phase 1: Audit (audit.go)

The audit phase generates a diagnostic prompt by combining three internal queries:

1. **Drift data**: run `ddis drift` internally to get current drift score and category breakdown
2. **Validation results**: run `ddis validate` internally to get check pass/fail status
3. **Coverage snapshot**: run `ddis coverage` internally to get per-component completeness

```
Algorithm: GenerateAuditPrompt
Input: spec_db (DB), spec_id (int), depth (int)
Output: CommandResult

1. drift_report = drift.Analyze(spec_db, spec_id)
2. validation_report = validator.Validate(spec_db, spec_id, {})
3. coverage_report = coverage.Compute(spec_db, spec_id)
4. confidence = derive_confidence(drift_report, validation_report, coverage_report)
5. limiting_factor = identify_limiting_factor(confidence)
6. exemplars = SelectExemplars(limiting_factor.dimension, spec_db, KStarEff(depth))
7. Assemble output:
   - Human-readable audit report with findings and recommended focus
   - StateSnapshot with drift, confidence, limiting_factor
   - Guidance with dof_hint="low", suggested_next=["Run 'ddis refine plan'"]
   - Include exemplars in guidance only if k* budget allows
8. Return CommandResult{output, state, guidance}
```

The output `CommandResult` always has `Guidance.SuggestedNext[0]` = "Run 'ddis refine plan'" and `Guidance.DoFHint` = "low" (audit is convergent).

#### Phase 2: Plan (plan.go)

The plan phase selects exactly ONE quality dimension (DoF separation per APP-ADR-017):

```
Algorithm: SelectFocusDimension
Input: confidence[5] (coverage, depth, coherence, completeness, formality)
Output: dimension string

1. Find minimum confidence score
2. IF multiple dimensions tied at minimum:
   Priority order: completeness > coherence > depth > coverage > formality
3. Return the selected dimension
```

The `--surface-ambiguity` flag triggers additional analysis: the planner scans for contradictory invariants and missing ADRs. These are emitted as questions in the output, never as resolutions (APP-INV-024).

#### Phase 3: Apply (apply.go)

The apply phase generates the Gestalt-optimized prompt the LLM uses to edit the spec. The prompt structure applies five Gestalt principles (APP-ADR-017):

1. **Spec-first framing** (+3-4 quality points): Formalize what quality means for the dimension before assigning the task. The `dimensionFraming(dimension)` function returns ~150 tokens of domain-specific framing per dimension (completeness, coherence, depth, coverage, formality).
2. **Demonstrations before task** (structure first, content second): Show full exemplar elements before presenting the element to improve. This primes the LLM's quality model.
3. **Full element demonstrations** (demonstrations > constraints): Show complete `RawText` of exemplar elements, not just one component. The full element encodes format, style, depth, tone, and domain simultaneously.
4. **Remove parasitic constraints**: The exemplar demonstration already shows what "good" looks like. Listing generic criteria ("Every invariant MUST have...") is redundant. Removed entirely (passes Gestalt removal test).
5. **Activating directive** (match language to substrate): The `activatingDirective(dimension, weak)` function replaces generic "Return ONLY the improved element" with dimension-specific instructions that activate deep reasoning.

```
Gestalt-Optimized Apply Prompt Structure (bounded by TokenTarget(depth)):
1. Spec-first dimension framing (~150 tokens): formalizes what quality means
2. Full exemplar demonstrations (1-3): complete elements showing excellence
3. Current element + diagnosis (~variable): full text + weak dimension label
4. Activating directive (~80 tokens): dimension-specific output instruction
```

Budget trimming follows a new priority order: reduce exemplars from N to 1, then shorten framing to a 1-sentence version, then drop exemplars entirely. The current element and activating directive are never trimmed.

**Worked example --- Gestalt-optimized refine apply prompt at depth=0, dimension=completeness:**

```
## Completeness in DDIS Specification

A complete invariant creates an interlocking proof structure: the statement
asserts the property, the semi-formal predicate makes it mechanically checkable,
the violation scenario proves it is falsifiable, the validation method makes it
testable, and why-this-matters connects it to system value. Each component
constrains interpretation of the others.

## Exemplar Demonstrations

The following elements demonstrate excellence. Study their structure, tone,
and depth.

### Exemplar 1: INV-ABC (Round-Trip Fidelity)

[complete raw text of the exemplar element, showing all components:
 statement, semi-formal predicate, violation scenario, validation method,
 and why-this-matters]

## Element to Improve: INV-XYZ (Cross-Reference Integrity)

Type: invariant | Weak dimension: completeness

[full raw text of element being improved]

## Your Task

Rewrite INV-XYZ so every component interlocks. What would a reviewer need
to see to trust this property holds? Preserve all existing correct content.
Output only the improved element in the same markdown format.
```

#### Phase 4: Judge (judge.go)

The judge phase compares before/after states and enforces drift monotonicity:

```
Algorithm: JudgeIteration
Input: spec_db (DB), iteration (int)
Output: CommandResult (with halt signal if regression detected)

1. drift_previous = state.Get(spec_db, "refine_drift_" + (iteration - 1))
2. drift_current = drift.Analyze(spec_db, spec_id).total_score
3. IF drift_current > drift_previous:
     output = "REGRESSION DETECTED: drift increased from {prev} to {curr}"
     guidance.suggested_next = ["Review the changes from the apply phase",
                                 "Consider rolling back via 'ddis tx rollback'"]
     state.limiting_factor = "drift regression in iteration " + iteration
   ELSE:
     output = "Quality improved: drift {prev} -> {curr} (delta: {delta})"
     guidance.suggested_next = ["Continue with 'ddis refine audit' for next iteration",
                                 "Or run 'ddis refine status' to check convergence"]
4. state.Set(spec_db, "refine_drift_" + iteration, drift_current)
5. Return CommandResult{output, state, guidance}
```

**Implementation Trace:**
- Source: `internal/refine/audit.go`
- Source: `internal/refine/plan.go`
- Source: `internal/refine/apply.go`
- Source: `internal/refine/judge.go`
- Source: `internal/refine/prompt.go`
- Source: `internal/autoprompt/budget.go::KStarEff`
- Source: `internal/autoprompt/budget.go::TokenTarget`
- Source: `internal/autoprompt/budget.go::Attenuation`

---

### Chapter: Discover Engine

**Preserves:** APP-INV-025 (Discovery Provenance Chain --- every artifact has a complete chain), APP-INV-026 (Classification Non-Prescriptive --- observe only), APP-INV-027 (Thread Topology Primacy --- threads over sessions), APP-INV-028 (Spec-as-Trunk --- threads crystallize into spec), APP-INV-029 (Convergent Thread Selection --- infer threads from content).

**Interfaces:** APP-INV-005 (Context Self-Containment --- discovery bundles are self-contained), APP-INV-008 (RRF Fusion Correctness --- thread matching uses search infrastructure), APP-INV-010 (Oplog Append-Only --- events recorded in append-only stream), APP-INV-030 (Contributor Topology --- graceful degradation).

The discover engine powers `ddis discover`. It manages the conversational workflow from context loading through thread convergence, mode classification, event recording, and artifact crystallization.

#### Context Loading (context.go)

The discovery context bundle extends the standard 9-signal context bundle with two additional signals:

- **Signal 10: Thread topology** --- active threads, summaries, confidence scores, last activity
- **Signal 11: Contributor topology** --- per-section authorship map (when git available, per APP-INV-030)

```
Algorithm: BuildDiscoveryContext
Input: spec_db (DB), thread_db (JSONL), git_root (string, may be "")
Output: DiscoveryBundle (11 signals)

1. base_bundle = context.Build(spec_db, target_element)  // existing 9-signal bundle
2. thread_signal = summarize_active_threads(thread_db)
3. IF git_root != "":
     contrib_signal = extract_contributor_topology(git_root, spec_db)
   ELSE:
     contrib_signal = nil  // graceful degradation (APP-INV-030)
4. return DiscoveryBundle{base_bundle, thread_signal, contrib_signal}
```

#### Thread Selection (thread.go)

Implements the `ConvergeThread` algorithm (see Formal Algorithm Specifications above) and provides lifecycle operations:

| Operation | Description | State Transition |
|---|---|---|
| branch | Create new thread from spec trunk | (none) -> active |
| resume | Resume a parked thread | parked -> active |
| park | Suspend thread for incubation | active -> parked |
| merge | Merge thread findings into spec | active -> merged |
| fork | Split thread into two sub-threads | active -> (active, active) |

Thread state is stored in `discovery/threads.jsonl`. Each record:

```json
{
  "id": "t-cache-invalidation",
  "status": "active",
  "summary": "Exploring TTL-based cache invalidation strategies",
  "spec_attachment": ["INV-007", "ADR-003"],
  "created_at": "2026-02-20T10:00:00Z",
  "last_event_at": "2026-02-22T14:30:00Z",
  "event_count": 12,
  "confidence": [6, 4, 7, 3, 5]
}
```

#### Mode Classification (classify.go)

Implements the `ClassifyMode` algorithm. Key design constraint: **this module has no write access to the output field of CommandResult.** It writes only to `State.ModeObserved` and `Guidance.ObservedMode`. This architectural constraint enforces APP-INV-026 at the code level --- it is impossible for the classification layer to emit prescriptive messages because it cannot modify the user-facing output.

```go
// classify.go signature — note: no access to output buffer
func Classify(events []Event) (mode string, confidence float64, evidence []string)

// Usage in discover command:
mode, conf, evidence := classify.Classify(recentEvents)
result.State.ModeObserved = mode
result.Guidance.ObservedMode = mode
result.Guidance.DoFHint = modeToDoF(mode)
// result.Output is NOT modified by classification
```

#### Event Recording (record.go)

Every significant interaction is recorded as an event in `discovery/events.jsonl`. Events carry both structural metadata (thread, session, timestamp) and cognitive metadata (mode, dialectical move). See Discovery Event Schema section above for the full schema.

The recording layer enforces provenance chain integrity: every `artifact_written` event must be preceded by at least one `question_opened` or `finding_recorded` event in the same thread (APP-INV-025). If this precondition is violated, the recording layer emits a warning but does not block --- the provenance gap is logged for later remediation.

**Worked example --- discover opening prompt at depth=0, k\*=12:**

```
## Discovery Context

Resuming thread t-ttl-strategies (last active 18h ago)
Confidence: coverage=5, depth=3, coherence=6, completeness=2, formality=4
Open questions: 1 (How should TTL expiry interact with write-through updates?)

## Thread Topology
- t-ttl-strategies (active, 7 events, 1 question open) [SELECTED]
- t-auth-flow (parked, 8 events, 0 questions)
- t-rate-limiting (merged, 15 events, 3 artifacts written)

## Relevant Spec Elements
- INV-007: Signal-to-Noise (related to cache overhead measurements)
- ADR-003: Event-Driven Architecture (prior decision on event bus)

## Contributor Topology
- wvandaal: authored INV-007, ADR-003 (strong overlap with current thread)
- collaborator: authored INV-012, ADR-009 (no direct overlap, but ADR-009
  mentions "event ordering" which is relevant to cache invalidation)

Your role: facilitate exploration. The user is in DIVERGENT mode (question density
high, no recent decisions). Keep the space open. Suggest failure modes, edge cases,
and analogies to other systems. Do NOT push toward decisions.
```

**Implementation Trace:**
- Source: `internal/discover/context.go`
- Source: `internal/discover/thread.go`
- Source: `internal/discover/classify.go::Classify`
- Source: `internal/discover/record.go`
- Source: `internal/discover/contrib.go`
- Source: `internal/discover/prompt.go`

---

### Chapter: Absorb Engine

**Preserves:** APP-INV-031 (Absorbed Artifacts Validate --- drafts pass validation), APP-INV-032 (Symmetric Reconciliation --- bidirectional gap analysis), APP-INV-033 (Absorption Format Parity --- indistinguishable from hand-written).

**Interfaces:** APP-INV-017 (Annotation Portability --- scanner uses universal grammar), APP-INV-018 (Scan-Spec Correspondence --- annotations reference resolvable elements), APP-INV-016 (Implementation Traceability --- absorbed artifacts include valid traces).

The absorb engine implements `ddis absorb`, which translates implementation back into spec. It is the inverse of the discover->implement path.

#### Scan Phase (scan.go)

The scanner reuses the annotation scanner from `internal/annotate/` and adds heuristic pattern detectors:

| Pattern | Heuristic | Candidate Spec Element |
|---|---|---|
| `assert(...)` or `if !cond { panic(...) }` | Assertion pattern | Candidate invariant |
| `return fmt.Errorf(...)` or `return errors.New(...)` | Error return pattern | Candidate negative spec |
| `type X interface { ... }` | Interface declaration | Candidate module boundary |
| `// ddis:maintains INV-NNN` | Explicit annotation | Direct spec reference |
| `switch x { case ... }` with `default: return error` | State machine pattern | Candidate state machine |

The scanner outputs a list of `Pattern` structs with source location (file:line), pattern type, extracted text, and confidence score.

#### Prompt Generation (prompt.go)

The absorption prompt combines:

1. **Extracted patterns** from `scan.go` (what the code does)
2. **Skeleton templates** from `ddis skeleton` (what a well-formed spec element looks like)
3. **Exemplar demonstrations** from `ddis exemplar` (what a high-quality spec element looks like)

This follows the "demonstrations > constraints" principle: the LLM sees concrete examples of good spec elements, not a list of rules about what makes a good spec element.

#### Draft Assembly (draft.go)

The drafter takes LLM output and structures it into valid DDIS format:

```
Algorithm: AssembleDraft
Input: llm_output (text), skeleton_templates
Output: draft_spec (valid DDIS markdown)

1. PARSE llm_output for:
   - Invariant blocks (match InvHeaderRe pattern)
   - ADR blocks (match ADR header pattern)
   - Negative spec blocks (match NegSpecRe pattern)
2. VALIDATE each block:
   - Invariant: has statement + violation + validation? (4 components)
   - ADR: has problem + options + decision + consequences + tests? (5 subsections)
   - If missing components: re-prompt LLM for missing pieces
3. ASSEMBLE into spec markdown with header and frontmatter
4. VALIDATE draft:
   ddis_parse(draft) -> success?
   ddis_validate(draft, level=1) -> pass?
   IF fail: return to step 2 with error feedback (max 3 retries)
5. Return draft_spec
```

#### Reconciliation (reconcile.go)

When `--against <db>` is used, the reconciler performs symmetric comparison:

```
Algorithm: Reconcile
Input: absorbed_patterns ([]Pattern), existing_spec_db (DB)
Output: ReconciliationReport

1. LOAD existing spec elements from DB
2. MATCH absorbed patterns to spec elements via LSI similarity:
   FOR EACH pattern IN absorbed_patterns:
     match = fuzzy_match(pattern, spec_elements)
     IF match.score >= 0.6: record correspondence(pattern, match)
     ELSE: record undocumented_behavior(pattern)
3. CHECK spec elements against absorbed patterns (REVERSE direction):
   FOR EACH element IN spec_elements:
     match = fuzzy_match(element, absorbed_patterns)
     IF match.score < 0.4: record unimplemented_spec(element)
4. DETECT behavioral divergence:
   FOR EACH (pattern, element) IN correspondences:
     IF semantic_conflict(pattern.behavior, element.claim):
       record behavioral_divergence(pattern, element)
5. RETURN report with three sections:
   undocumented_behavior, unimplemented_spec, behavioral_divergence
```

**Worked example --- absorption of a Go function with `assert` patterns:**

Given this source:

```go
func CommitTransaction(db DB, txID string) error {
    result, err := db.Exec(
        `UPDATE transactions SET status='committed' WHERE tx_id=? AND status='pending'`, txID)
    if err != nil { return fmt.Errorf("commit transaction: %w", err) }
    rows, _ := result.RowsAffected()
    if rows == 0 {
        return fmt.Errorf("transaction %s not found or not pending", txID)
    }
    return nil
}
```

The scanner extracts two patterns:
1. `error_return` at line 4: "commit transaction: %w" (confidence 0.6)
2. `assertion` at line 6: "rows == 0 guard" -> candidate invariant: "only pending transactions can be committed" (confidence 0.8)

The LLM drafts a candidate invariant:

```
**INV-XYZ: Pending-Only Commit**
*Only transactions in `pending` status can transition to `committed`.*

FOR ALL tx IN transactions:
  commit(tx) IMPLIES prev(tx.status) = "pending"

Violation scenario: A bug omits the `AND status='pending'` predicate...

Validation: Attempt commit on committed transaction; verify error returned.
```

Draft passes `ddis validate` Level 1. The candidate is presented to the human for review.

**Implementation Trace:**
- Source: `internal/absorb/scan.go`
- Source: `internal/absorb/prompt.go`
- Source: `internal/absorb/draft.go`
- Source: `internal/absorb/reconcile.go`

---

## CLI Commands

### `ddis discover`

**Interface**: `ddis discover [--thread <id>] [--prompt-only] [--auto]`

ONE command. The system handles context loading, thread selection, mode classification, and artifact crystallization. The user starts talking; the system converges.

- Default: load discovery context, present topology, open conversation
- `--thread <id>`: explicit thread selection (override convergent matching)
- `--prompt-only`: output the opening context prompt without executing
- `--auto`: non-interactive mode for CI/automation

**Accelerator subcommands** (power-user, not primary path):
- `ddis discover status` --- topology, confidence, threads
- `ddis discover threads` --- thread listing with summaries
- `ddis discover park [--thread <id>]` --- park thread for incubation
- `ddis discover merge <source> [--into <target>]` --- merge findings

### `ddis refine`

**Subcommands** (inherently procedural):
- `ddis refine audit [--prompt-only]` --- generate audit from drift + validation
- `ddis refine plan [--prompt-only] [--surface-ambiguity]` --- generate improvement plan
- `ddis refine apply [--prompt-only]` --- generate apply prompt with exemplars
- `ddis refine judge [--prompt-only]` --- evaluate quality trajectory
- `ddis refine status` --- iteration count, drift history, convergence rate
- `ddis refine history` --- drift trajectory over iterations

### `ddis absorb`

**Interface**: `ddis absorb <code-root> [--against <db>] [--output <spec.md>] [--prompt-only] [--auto]`

- Default: scan code, generate draft spec
- `--against <db>`: reconciliation mode --- compare absorbed draft against existing spec
- `--output <spec.md>`: write draft to file (default: stdout)
- `--prompt-only`: output the LLM prompt without executing
- `--auto`: non-interactive mode

---

## Package Structure

```
internal/autoprompt/          --- shared types across discover/refine/absorb
+-- types.go                  --- CommandResult, StateSnapshot, Guidance
+-- translate.go              --- translation framework loading (cognition model + gestalt)
+-- budget.go                 --- k* attention budget management

internal/refine/              --- refinement prompt engine
+-- audit.go                  --- generates audit prompt from drift + validation + coverage
+-- plan.go                   --- generates planning prompt from progress + impl-order
+-- apply.go                  --- generates apply prompt from context + exemplars
+-- judge.go                  --- generates judge prompt from before/after drift comparison
+-- prompt.go                 --- shared prompt construction, Gestalt principles

internal/discover/            --- discovery prompt engine
+-- context.go                --- discovery context bundle (persistent field configuration)
+-- prompt.go                 --- single opening prompt generation
+-- thread.go                 --- convergent thread selection + lifecycle management
+-- classify.go               --- real-time mode classification (observation only)
+-- record.go                 --- event recording with cognitive metadata
+-- contrib.go                --- contributor topology extraction (graceful degradation)

internal/absorb/              --- absorption engine
+-- scan.go                   --- reuses internal/annotate/ + heuristic pattern detection
+-- prompt.go                 --- LLM prompt generation with skeleton + exemplar context
+-- draft.go                  --- assembles LLM output into DDIS-conformant structure
+-- reconcile.go              --- --against mode: bidirectional gap analysis
```

---

## Negative Specifications

These constraints prevent the most likely implementation errors and LLM hallucination patterns for the auto-prompting subsystem. Each addresses a failure mode that an LLM, given only the positive specification, would plausibly introduce.

**DO NOT** generate prompts that exceed LLM context budget. The k\* guard ensures prompt size is proportional to conversation depth and available context. If the complete context cannot fit, the prompt must degrade gracefully: summarize instead of include, prioritize by relevance, drop constraints before exemplars. No prompt may exceed `TokenTarget(depth)` tokens. An LLM implementing the apply phase must check token count before assembly and trim sections in reverse priority order. (Validates APP-INV-023, APP-INV-035)

**DO NOT** hide intermediate state from the user. Every `CommandResult` includes a human-readable output. The `--prompt-only` flag exposes the raw guidance. No command executes side effects without the user being able to inspect what happened. An LLM must never suppress `CommandResult.Output` or emit guidance without a corresponding output explanation. (Validates APP-INV-034)

**DO NOT** substitute LLM judgment for mechanical verification. Absorbed artifacts must pass `ddis validate`. Refined specs must show drift reduction. The LLM generates; the validator verifies. This separation is inviolable --- the validator is the authority, not the LLM. An LLM must never skip the validate-after-apply step or claim an artifact is valid without running `ddis validate`. (Validates APP-INV-031)

**DO NOT** prescribe cognitive mode transitions --- only observe and classify. The classification layer is read-only with respect to the user experience. Mode labels flow into prompt generation (DoF calibration) but never into user-facing messages. The user never sees "you are in divergent mode" unless they explicitly query status. An LLM generating discover prompts must never include mode names in the output field of CommandResult. (Validates APP-INV-026)

**DO NOT** treat absorbed spec artifacts as authoritative without human review --- absorption is draft, not declaration. Absorbed artifacts are candidates. The human reviews, edits, and confirms crystallization. The absorption engine never writes directly to the authoritative spec without a confirmation step. An LLM must present absorbed artifacts as proposals with "[DRAFT]" markers, not as final spec content. (Validates APP-INV-031, APP-INV-033)

**DO NOT** embed LLM provider keys or model identifiers in spec content. The state monad architecture keeps the CLI provider-agnostic. No command generates output containing API keys, model names, or provider-specific configuration. Session metadata may record model names in the event stream (type: "session_started"), but these never appear in spec artifacts. (Validates CLI purity, APP-ADR-022)

**DO NOT** select threads by explicit user declaration --- thread identity is inferred. The `--thread` flag is an override escape hatch, never the primary path. Default behavior always infers thread from content via LSI/BM25 similarity. An LLM must never prompt the user "which thread do you want to use?" as the first interaction --- it must run ConvergeThread and present the inferred selection. (Validates APP-INV-029)

**DO NOT** treat guidance as mandatory --- attenuation to zero is always valid. At any point, the guidance may be fully attenuated (attenuation approaching 0.75 at the floor), producing minimal guidance. The LLM must be able to function with near-zero guidance after sufficient conversation depth. An LLM must never fail or error when receiving a CommandResult with `attenuation > 0.7` and sparse guidance fields. (Validates APP-INV-035)

**DO NOT** break provenance chain continuity during event recording. Every `decision_crystallized` event must have a traceable chain back to a root event (`question_opened` or `finding_recorded`) with consecutive sequence numbers within the same thread. An LLM recording events must never skip sequence numbers, assign events to wrong threads, or emit a crystallization event without a preceding root event. (Validates APP-INV-025)

**DO NOT** report reconciliation gaps in only one direction. The `--against` mode must always report both undocumented behavior (code does things the spec doesn't mention) AND unimplemented specification (spec claims things the code doesn't do). An LLM implementing reconciliation must never short-circuit after finding undocumented behavior --- it must also check for unimplemented spec in a second pass. (Validates APP-INV-032)

---

## Worked Examples

### Worked Example 1: A Complete Refine Cycle

This example shows three iterations of the refine loop on a spec with initial drift of 6.0, demonstrating monotonic drift decrease and the state monad in action.

**Initial state:** A CLI spec with 10 invariants, 3 missing violation scenarios, 2 unresolved cross-references, 1 ADR missing Tests subsection. Drift = 6.0.

**Iteration 1: Audit**

```bash
$ ddis refine audit index.db --json
```

```json
{
  "output": "## Audit Report (Iteration 1)\n\nDrift: 6.0\nLimiting factor: completeness (3 invariants missing violation scenarios)\n\nFindings:\n1. APP-INV-003 missing violation scenario\n2. APP-INV-007 missing violation scenario\n3. APP-INV-012 missing violation scenario\n4. APP-ADR-006 missing Tests subsection\n5. 2 unresolved cross-references in §2.3",
  "state": {
    "active_thread": "",
    "confidence": [8, 5, 6, 4, 7],
    "limiting_factor": "completeness: 3 invariants missing violation scenarios",
    "open_questions": 0,
    "artifacts_written": 0,
    "spec_drift": 6.0,
    "iteration": 1,
    "mode_observed": ""
  },
  "guidance": {
    "observed_mode": "",
    "dof_hint": "low",
    "suggested_next": [
      "Run 'ddis refine plan' to focus on completeness dimension",
      "Prioritize invariant violation scenarios over ADR Tests"
    ],
    "relevant_context": ["APP-INV-003", "APP-INV-007", "APP-INV-012", "APP-ADR-006"],
    "translation_hint": "audit identified completeness as limiting factor — plan should focus there",
    "attenuation": 0.08
  }
}
```

**Iteration 1: Plan -> Apply -> Judge**

Plan selects "completeness" as focus dimension. Apply generates prompt with exemplar. Judge compares: drift 6.0 -> 4.5 (delta: -1.5). Changes: +1 violation scenario (APP-INV-003), +1 validation method.

**Iterations 2-3** follow the same pattern: drift goes 4.5 -> 3.0 -> 1.5. By iteration 3, all violation scenarios are complete and the ADR Tests subsection is added. The remaining drift (1.5) comes from 2 unresolved cross-references.

---

### Worked Example 2: A Discovery Session

**Step 1: Initial invocation** (depth=0, k\*=12, full framework)

```bash
$ ddis discover index.db --json
```

The user has been thinking about cache invalidation. ConvergeThread runs:
- Existing thread "t-ttl-strategies" has summary "Exploring TTL-based expiration strategies"
- LSI similarity: 0.72, BM25: 0.35, Combined: 0.672 (above 0.4 threshold)
- System selects thread "t-ttl-strategies" (natural resumption)

**Step 2: User says something divergent**

User: "What if we used event-driven invalidation instead of TTL?"

Classification: `divergent` (confidence 0.7). Event recorded with `type: "question_opened"`.

**Step 3: Three more exchanges** (depth=3, k\*=12 still)

Classification shifts: dialectical -> abductive -> convergent as the conversation evolves through challenge, analogy, and proposal.

**Step 4: Crystallization**

User: "OK, I think the answer is: TTL as baseline, event-driven as optimization."

Classification: `convergent` (confidence 0.8). The LLM authors ADR-XYZ. Crystallization event recorded. Provenance chain verified: `evt-d4e5f6 (question) -> evt-d5f6g7 (challenge) -> evt-e6g7h8 (analogy) -> evt-f7h8i9 (proposal) -> evt-g7h8i9 (crystallization)`. Chain connected (APP-INV-025 satisfied).

---

### Worked Example 3: Absorption Round-Trip

**Step 1: Scan** extracts patterns from annotated Go source (annotations, error returns, assertion patterns).

**Step 2: Reconciliation** (`--against cli.db`)

```bash
$ ddis absorb ddis-cli/ --against cli.db --json
```

Result: 2 correspondences confirmed, 2 undocumented behaviors (code does things spec doesn't fully capture), 1 unimplemented spec (no code evidence for a spec claim). Behavioral divergence: 0.

The reconciliation found gaps in both directions (APP-INV-032 satisfied): the code has behaviors the spec is silent about, and the spec has claims the code doesn't evidence.

---

## Verification Prompt

Use this self-check after implementing or modifying the auto-prompting subsystem.

**Positive checks (DOES the implementation...):**

1. DOES every auto-prompting command return a valid `CommandResult` with non-empty output, state, and guidance? (APP-INV-034)
2. DOES `--prompt-only` suppress all side effects (no DB writes, no file writes, no event appends) while still emitting guidance? (APP-INV-034)
3. DOES the refine loop halt when drift increases between iterations? (APP-INV-022)
4. DOES the refine judge compare spec-internal drift (not code-spec drift) for monotonicity? (APP-INV-022)
5. DOES discovery classify modes without prescribing transitions? (APP-INV-026)
6. DOES the classification layer's output appear only in `State.ModeObserved` and `Guidance.ObservedMode`, never in `Output`? (APP-INV-026)
7. DOES `ConvergeThread` match threads via LSI/BM25 similarity rather than exact keywords? (APP-INV-029)
8. DOES thread convergence create a new thread when best match score is below 0.4? (APP-INV-029)
9. DOES every crystallized artifact have a connected provenance chain from root to terminal event? (APP-INV-025)
10. DOES `ddis discover` function correctly in a directory without `.git/`? (APP-INV-030)
11. DOES `ddis absorb` output pass `ddis validate` at Level 1? (APP-INV-031)
12. DOES `ddis absorb --against` report gaps in both directions (undocumented + unimplemented)? (APP-INV-032)
13. DOES the refine audit surface ambiguities as questions, never as resolutions? (APP-INV-024)
14. DOES `k_star_eff(0) = 12` and `k_star_eff(45) = 3`? (APP-INV-035)
15. DOES guidance attenuation produce monotonically decreasing guidance size? (APP-INV-035)
16. DOES every prompt include all spec elements it references by ID, with no dangling references? (APP-INV-023)
17. DOES the SelectExemplars algorithm ensure diversity across element types (invariant, ADR, negative spec)? (APP-ADR-017)
18. DOES the thread merge operation verify that all crystallization events produced spec artifacts? (APP-INV-028)

**Negative checks (does NOT the implementation...):**

1. Does NOT generate prompts exceeding the k\* token budget for the current depth? (NEG-AUTOPROMPT-001, APP-INV-023, APP-INV-035)
2. Does NOT include cognitive mode names ("divergent", "convergent", etc.) in `CommandResult.Output`? (NEG-AUTOPROMPT-002, APP-INV-026)
3. Does NOT resolve ambiguities autonomously --- ambiguities are surfaced, never silently decided? (NEG-AUTOPROMPT-003, APP-INV-024)
4. Does NOT require git for core discovery features --- degradation is graceful? (NEG-AUTOPROMPT-004, APP-INV-030)
5. Does NOT produce absorbed artifacts that fail Level 1 validation? (NEG-AUTOPROMPT-005, APP-INV-031)
6. Does NOT report reconciliation gaps in only one direction? (NEG-AUTOPROMPT-006, APP-INV-032)
7. Does NOT emit prompts with dangling references (spec elements referenced by ID but not included in context)? (NEG-AUTOPROMPT-007, APP-INV-023)
8. Does NOT force users to declare thread IDs --- thread selection is inferred by default? (NEG-AUTOPROMPT-008, APP-INV-029)
9. Does NOT write absorbed artifacts directly to the authoritative spec without human confirmation? (NEG-AUTOPROMPT-009, APP-INV-033)
10. Does NOT embed provider keys, model names, or LLM-specific configuration in spec artifact output? (NEG-AUTOPROMPT-010, APP-ADR-022)
11. Does NOT skip sequence numbers in event recording within a single thread? (NEG-AUTOPROMPT-011, APP-INV-025)
12. Does NOT allow the apply phase to trim spec-first framing or the current spec element from prompts? (NEG-AUTOPROMPT-012, APP-ADR-017)

---

## Referenced Invariants from Other Modules

Per the cross-module reference completeness convention, this section lists invariants
owned by other modules that this module depends on or interfaces with:

| Invariant    | Owner              | Relationship | Usage in This Module                                            |
|--------------|--------------------|--------------|------------------------------------------------------------------|
| APP-INV-001  | parse-pipeline     | interfaces   | Round-trip fidelity ensures refine apply edits preserve content |
| APP-INV-002  | query-validation   | interfaces   | Validation determinism ensures refine judge comparisons are meaningful |
| APP-INV-003  | query-validation   | interfaces   | Cross-ref integrity ensures discovery context bundles are complete |
| APP-INV-005  | search-intelligence| interfaces   | Context self-containment extends to discovery bundles |
| APP-INV-008  | search-intelligence| interfaces   | RRF fusion correctness ensures thread matching accuracy |
| APP-INV-009  | parse-pipeline     | interfaces   | Monolith-modular equivalence for absorption across formats |
| APP-INV-010  | lifecycle-ops      | interfaces   | Oplog append-only pattern reused for discovery event streams |
| APP-INV-015  | parse-pipeline     | interfaces   | Deterministic hashing for event stream correlation |
| APP-INV-016  | lifecycle-ops      | interfaces   | Implementation traceability for absorbed artifact annotations |
| APP-INV-017  | code-bridge        | interfaces   | Annotation portability for absorb scanner |
| APP-INV-018  | code-bridge        | interfaces   | Scan-spec correspondence for absorb reconciliation |
| APP-INV-020  | code-bridge        | interfaces   | Event stream append-only for discovery JSONL |

**APP-INV-042: Guidance Emission**

*Every CLI command that produces non-empty findings MUST emit at least one navigational guidance hint as a postscript, unless suppressed by --no-guidance.*

```
FOR ALL cmd IN DataCommands, result = cmd.Execute():
  result.Findings \!= [] AND NOT NoGuidance
  IMPLIES len(result.GuidanceHints) >= 1

DataCommands = {parse, validate, coverage, drift, search, context, exemplar, impact, progress}
```

Violation scenario: An LLM agent runs ddis validate and receives 3 failing checks but no hint about what to do next. The agent stalls, unable to determine whether to run context, exemplar, or search. The improvement loop halts until a human intervenes with the right command.

Validation: Run each DataCommand against a spec with known findings. Verify that stdout includes a "Next:" postscript with at least one ddis command. Run with -q flag and verify the postscript is absent. Parse the output to confirm the hint is a valid ddis command.

// WHY THIS MATTERS: The CLI is an LLM-facing API (APP-ADR-023). Without guidance, each command is a dead end requiring external knowledge to continue. Guidance transforms the CLI from a passive tool into an active navigator that drives the improvement loop forward.

---

**APP-INV-045: Universal Auto-Discovery**

*Every CLI command that reads from a spec database MUST support auto-discovery: when no database path is given, the command searches the current directory for a single *.ddis.db file and uses it automatically.*

```
FOR ALL cmd IN DBReadingCommands:
  cmd.Args = [] AND EXISTS unique db IN cwd/*.ddis.db
  IMPLIES cmd.Execute() uses db

DBReadingCommands = AllCommands \ {parse, init, skeleton, help}
```

Violation scenario: An LLM agent in a directory with manifest.ddis.db runs ddis discover --content "new idea" and the command crashes with a nil pointer because no --spec flag was provided. The agent must already know the DB path to use discovery, defeating the zero-knowledge adoption property.

Validation: For each DB-reading command, run it in a directory containing exactly one *.ddis.db file with no database argument. Verify the command succeeds and operates on the discovered database. Then run in a directory with zero or multiple *.ddis.db files and verify a clear error message.

// WHY THIS MATTERS: Database path is parasitic boilerplate (APP-ADR-023). Every command that requires it forces the LLM to track state that the filesystem already provides. Auto-discovery eliminates a class of argument-order errors and enables zero-knowledge adoption.

---

### APP-ADR-031: Navigational Guidance as Postscript

#### Problem

CLI commands return results but provide no indication of what to do next. LLM agents must have external knowledge of the DDIS workflow to chain commands productively.

#### Options

A) **Embed guidance in command help text** --- Static, not context-sensitive. Requires reading help before using the command.
B) **Separate guidance command** --- Requires knowing to run it. Does not solve the "what next" problem at the point of output.
C) **Append context-sensitive guidance as postscript to every data command** --- Guidance is a pure projection of domain results, no additional DB queries. Suppressed with -q/--no-guidance. JSON output includes guidance as a top-level key.

#### Decision

**Option C: Append context-sensitive guidance as postscript.** Every data command appends a "Next:" postscript with 1-3 context-sensitive hints derived from its output. The pattern already exists in drift.renderRecommendation() --- this ADR generalizes it to all data commands. Pure projection ensures guidance adds zero latency.

// WHY NOT Option A (embed in help)? Static help text cannot adapt to the command's actual output. An agent that ran validate and got 3 failures needs "ddis context APP-INV-006", not generic help.

// WHY NOT Option B (separate command)? Defeats the purpose --- the agent must already know to run the guidance command, which is the problem we're solving.

**Confidence:** Committed

#### Consequences

The CLI is an LLM-facing API (APP-ADR-023). Postscript guidance transforms passive tool output into active navigation. Without guidance, each command is a dead end requiring external knowledge to continue. With guidance, the tool directs the improvement loop forward. The -q flag preserves backward compatibility for scripting.

#### Tests

- Run ddis validate with failures: verify Next: appears with relevant command
- Run ddis validate -q: verify no Next: appears
- Run ddis validate --json: verify guidance key in JSON output
- Verify no guidance generator calls db.Query (pure projection)

---

### APP-ADR-033: ddis next as Universal Entry Point

#### Problem

An LLM encountering DDIS for the first time has no starting point. Bare ddis shows cobra help --- a list of 30 commands with no indication of which to run or in what order. The agent must have prior knowledge of the workflow to begin.

#### Options

A) **Enhanced help text** --- Better descriptions in cobra help. Still a list, not a directed suggestion. Requires reading all entries to find the right one.
B) **Workflow documentation** --- ddis help --workflow shows a guide. Requires knowing the flag exists. Static, not context-sensitive.
C) **Meta-command that reads state and directs** --- ddis next inspects the current workspace (DB presence, validation status, coverage, drift) and emits exactly one suggested command with rationale.

#### Decision

**Option C: ddis next as meta-command.** Bare ddis delegates to next logic. ddis next reads workspace state and emits the single highest-value next action. Zero-knowledge adoption: round 0 (ddis) tells you to parse, round 1 (ddis parse) tells you to validate, round 2 (ddis validate) produces actionable output. Three rounds to autonomy.

// WHY NOT Option A (enhanced help)? Help is a menu, not a recommendation. An agent reading 30 command descriptions still faces a decision problem. next solves it by choosing for you.

// WHY NOT Option B (workflow docs)? Static documentation cannot adapt to workspace state. An agent that has already parsed but not validated needs different guidance than one starting fresh.

**Confidence:** Committed

#### Consequences

The state monad architecture (APP-ADR-022) ensures every command returns guidance. ddis next is the bootstrap --- it provides the first guidance when no command has been run yet. Combined with universal auto-discovery (APP-INV-045), the complete zero-to-productive path requires zero configuration and zero prior knowledge.

#### Tests

- Run ddis next with no DB: verify it suggests ddis parse manifest.yaml
- Run ddis next with valid DB: verify it reports status and suggests next action
- Run bare ddis: verify it delegates to next, not cobra help
- Verify ddis next reads workspace state (DB existence, validation, coverage, drift)

---

**APP-INV-046: Error Recovery Guidance**

*Every CLI command that fails with an actionable error MUST emit at least one recovery hint on stderr, unless suppressed by --no-guidance.*

```
FOR ALL cmd IN Commands, err = cmd.Execute():
  err != nil AND IsActionable(err) AND NOT NoGuidance
  IMPLIES stderr CONTAINS RecoveryHint(err)

Actionable(err) = err.category IN {no_db, stale_db, bad_args, missing_spec, empty_query}
RecoveryHint(err) = "Tip: " + corrective_ddis_command
```

Violation scenario: An LLM agent runs ddis validate without a database present. The error says 'open database: no such file or directory' but provides no guidance on how to create the database. The agent has no way to recover without external documentation.

Validation: For each Actionable error category, trigger the error condition and verify stderr contains a Tip: line with a valid ddis command. Verify the tip is suppressed with -q. Verify non-actionable errors (e.g., I/O failures) do NOT emit tips.

// WHY THIS MATTERS: Error messages are the highest-friction interaction point in CLI UX. An agent that encounters an error and receives a recovery hint can self-correct immediately. Without recovery hints, every error becomes a dead end requiring human intervention or external documentation lookup.

---
