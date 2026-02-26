# Comprehensive Report: Event Stream Design Intent vs. Implementation Reality

**Date:** 2026-02-26
**Trigger:** Discovery that `ddis init` was gitignoring `.ddis/events/*.jsonl` — directly contradicting the design rationale for using JSONL (VCS-trackable provenance).
**Method:** Parallel deep search across cass (session history), cm (procedural memory), codebase (Go source), and spec (DDIS CLI spec + parent spec). Four research agents, 16+ cass queries, 6 cm queries, 57+ file reads.

---

## I. Your Original Intent — Reconstructed from Primary Sources

From session `571bdf7e` (Feb 23), you proposed:

> "what do you think about an append-only jsonl log that backs the sqlite storage for the specification as it evolves? this would be somewhat similar to how beads (`br`) is implemented. This would allow us to keep the entire spec and its evolution in VCS and share it across our team."

From session `bf0563ae` (Feb 24), you expanded:

> "I think this JSONL append only log could potentially serve as our 'event-sourcing' mechanism for the kind of time travel you outline."

From session `4d01234a` (Feb 24-25), you articulated the cognitive model motivation:

> "I think the gestalt theory for LLM cognition has been *incredibly* accretive to how I communicate with LLMs. Im wondering if a similar exploration of the human theory of mind and modes of cognition would be equally valuable for us to codify so that the LLMs fully understand how humans think. This would map onto your insight of the bidirectional relationship between the DDIS spec and the humans' Discovery process. Specifically if a deep investigation and codifying of the various human modes of cognition (and crucially, discursive cognition, e.g. dialectics, since this is happening in a natural language substrait with an LLM) might help deeply inform both our system architecture and the data structures that underpin them (e.g. the JSONL fine-grain event format)."

From session `a89f2818` (Feb 23), the tagged bottom type:

> "I actually think you've identified the final failure mode: undefined, ambiguous, or unknown details or speculative pointers to possible future states. Is there some kind of TODO, marker, annotation, mathematically formal expression (i.e. like a bottom type in type theory or a symbol or something in that vein). The tension in our dialogue is my intuition and desire to explore the unknown unknown space, coming into contact with your legitimate desire for specificity, clarity, precision, and accuracy."

And today (Feb 26), your definitive restatement of the complete pipeline:

> "The whole point of capturing the conversation and decision event stream was to iteratively build the final JSON definition for the human definition of the plan. The event stream would show us how we got to the human definition of the problem and preserve the evolution (i.e. discovery) process by which the human user arrived at the design. Once the design is codified by the user, it is converted by the AI, piece by piece, iteratively to the DDIS spec, verifying and validating internal consistency (no implementation yet) and interrogating the user when the translation process surfaces contradictions, ambiguities, questions, or other areas in need of resolution."

---

## II. The Intended Pipeline (10 Steps)

Reconstructed from all sessions:

1. **Human explores and thinks** in natural language, across cognitive modes
2. **Every utterance is captured** as a JSONL event with cognitive mode, dialectical move, thread ID, and raw content
3. **The event stream builds iteratively** — showing HOW the human arrived at each insight
4. **When the human crystallizes a decision**, it becomes a `decision_crystallized` event with provenance chain
5. **The AI translates** crystallized decisions piece-by-piece into formal DDIS spec, validating consistency
6. **During translation, contradictions surface** — the AI interrogates the user, creating new discovery events
7. **Unresolved areas get tagged bottom types** — named markers (`⊥[AUTH_STRATEGY]`) that type-check but are explicitly uninhabited
8. **The complete event stream preserves provenance** — any spec element traces back through crystallization → discussion → original finding
9. **The spec IS the trunk** — all threads branch from and merge back into the specification
10. **Starting a new session feels like resuming a conversation** — LLM converges on context and mode automatically

---

## III. What the Spec Captures

The spec is **remarkably faithful** to your intent. Key elements:

| Intent | Spec Element | Status |
|--------|-------------|--------|
| Three-stream event sourcing | APP-ADR-015 | Fully specified with envelope schema, event types, cross-stream correlation |
| Append-only guarantee | APP-INV-020 | Fully specified with content-hash equality |
| Provenance chains | APP-INV-025 | Formally specified with consecutive-sequence predicate |
| Threads over sessions | APP-INV-027, APP-ADR-019 | "Sessions are accidents of tooling; threads are the structure of thought" |
| Seven cognitive modes | ClassifyMode algorithm | Divergent, convergent, dialectical, abductive, metacognitive, incubation, crystallization |
| Observation, not prescription | APP-INV-026, APP-ADR-018 | "Classification labels are metadata, not directives" |
| Bilateral specification | APP-ADR-024 | Four adjunctions: discover⊣absorb, parse⊣render, tasks⊣traceability, refine⊣drift |
| Human never learns format | APP-INV-036, APP-ADR-023 | "LLMs write specs, humans review" |
| State monad architecture | APP-ADR-022, APP-INV-034 | CLI returns (output, state, guidance); LLM is interpreter |
| Persistent manifold metaphor | auto-prompting.md §A.1 | "Discovery explores tangent space; crystallization projects back onto manifold" |
| Spec-as-trunk | APP-INV-028 | "Every thread branches from the spec and crystallizes back into it" |
| Convergent thread selection | APP-INV-029 | Thread attachment inferred from content via LSI/BM25 |

**Verdict: The spec captures your vision with high fidelity.**

---

## IV. What the Implementation Actually Has

Here is where it falls apart. The code agent's analysis is damning:

| Capability | Spec | Implementation |
|-----------|------|----------------|
| Three-stream architecture | Fully specified | **NOT IMPLEMENTED.** Two unrelated systems: oplog + discovery stream |
| `internal/events/` package | Referenced 6 times in spec | **DOES NOT EXIST** |
| Discovery event envelope | Specified (`id`, `spec_hash`, `stream`, typed payloads) | Missing `id`, `spec_hash`, `stream`; untyped `map[string]interface{}` |
| `mode_observed` events | Specified with `mode`, `score`, `signals` | **NEVER EMITTED.** Mode computed ephemerally at read time |
| Dialectical moves | Mentioned 3x in spec | **NOT IMPLEMENTED.** Only `challenge_posed` event type exists |
| Provenance chains | APP-INV-025 requires chain on `decision_crystallized` | **NOT IMPLEMENTED.** No enforcement, no verification |
| `CorrelateStreams()` | Specified in code-bridge | **DOES NOT EXIST** |
| Stream 3 (Implementation) | Specified with 4 event types | **DOES NOT EXIST AT ALL** |
| Schema enforcement | Per-type field specs | **NONE.** `Data` is `map[string]interface{}` |
| Tagged bottom type (`⊥[NAME]`) | **NOT IN SPEC** | **NOT IN CODE** |
| Crystallize → event recording | Required by APP-INV-025 | **CRYSTALLIZE BYPASSES EVENT STREAM** — writes directly to spec files |
| Dual Event structs | N/A | `discover.Event` ≠ `discovery.DiscoveryEvent` — accidental fork |

---

## V. What cm (Procedural Memory) Captured

**Almost nothing.** Two rules tangentially related:

1. `b-mm2e6bpt-460dd5` — "Use typed holes (`⊥[COMPARISON_TARGET]`) instead of bare TODO" — captures the notation but not the full philosophy
2. `b-mm2eqbm4-urn8dz` — "Distinguish between specified requirements, exploratory ideas, and theoretical context using formal markers" — sourced from a DDIS session

**Zero rules** about: three-stream event sourcing, discovery event architecture, bilateral specification lifecycle, cognitive model, Gestalt theory application, or the human-intent-to-formal-spec pipeline.

---

## VI. Formal Failure Mode Analysis

Your question identifies a **systemic failure with three distinct levels**:

### Level 1: The Gitignore Bug (Surface Symptom)

The JSONL streams were gitignored, directly contradicting their purpose as VCS-tracked provenance. **Fixed today.**

### Level 2: Spec-Implementation Divergence (Structural Gap)

The spec faithfully captures your vision. The implementation does not. This is precisely the kind of divergence that DDIS was built to catch, yet it was not caught. Why?

- **`ddis drift` measures spec↔code annotation alignment**, not spec↔code behavioral fidelity. The annotations say `ddis:implements APP-ADR-026` and `ddis:maintains APP-INV-037`, which is true — the init command exists. But the annotations don't capture that the init command's gitignore behavior contradicts the design rationale of APP-ADR-015.
- **The three-stream architecture was specified but never implemented.** The `internal/events/` package (referenced 6 times in the spec) does not exist. `ddis drift` wouldn't catch this because drift measures what IS annotated, not what's MISSING.
- **No mechanical check links "JSONL is VCS-tracked" to "gitignore must not exclude JSONL."** This is a cross-cutting concern that falls between the cracks of individual checks.

### Level 3: The Discovery System Didn't Catch Its Own Purpose (Meta-Failure)

This is the most profound failure. The discovery event stream was designed to capture the evolution of human thinking so that design intent is never lost. But the design intent for the discovery event stream itself was lost. The system failed at its own stated purpose, on its own use case.

**Root causes:**

1. **The conversation where you stated your intent (sessions `571bdf7e`, `a89f2818`, `4d01234a`) predated the implementation.** The intent lived in cass session logs but was never formally crystallized via `ddis discover`. It existed as raw conversation, not as a provenance-chained event stream.

2. **The tagged bottom type was never specified.** You proposed it in session `a89f2818`. The agent formalized it as `⊥[NAME]` with a 4-step discovery protocol. cm captured two rules about the notation. But it never made it into the DDIS spec as an invariant, ADR, or event type. The concept fell into the gap between "discussed" and "specified."

3. **The cognitive model was implemented as ephemeral computation, not durable record.** `ClassifyMode()` runs at read time and returns results in `CommandResult.Guidance`. But no `mode_observed` event is ever written to the JSONL. The cognitive provenance — the very thing the system was designed to preserve — evaporates after each command invocation.

4. **Crystallization bypasses the event stream.** `ddis discover crystallize` writes directly to spec files without recording a `decision_crystallized` event. This means APP-INV-025 (provenance chain) is structurally unenforceable through the actual code path.

---

## VII. Formal Verification: Which Properties Hold?

Using the spec's own formal predicates:

| Property | Formal Predicate | Holds? | Evidence |
|----------|-----------------|--------|----------|
| APP-INV-020 (Append-only) | No record modified/deleted after write | **PARTIALLY.** Append-only file semantics enforced via `O_APPEND`. Content-hash equality NOT implemented. |
| APP-INV-025 (Provenance chain) | `chain_connected(a) = true` for all artifacts | **DOES NOT HOLD.** Crystallize bypasses event recording. No chain verification code exists. |
| APP-INV-026 (Non-prescriptive) | `c.prescription = null` for all classification events | **VACUOUSLY TRUE.** No `mode_observed` events are ever written, so the predicate holds on an empty set. |
| APP-INV-027 (Thread primacy) | `e.thread_id IS NOT NULL` for all events | **HOLDS** for events written by `RecordEvent()`. SessionID is correctly optional. |
| APP-INV-028 (Spec-as-trunk) | No orphan threads bypass spec integration | **DOES NOT HOLD.** Crystallize doesn't record events; threads can be merged without verifying artifact emission. |
| APP-INV-034 (State monad universality) | Every command returns (output, state, guidance) | **HOLDS** for commands with autoprompt integration. |
| APP-INV-036 (Human format transparency) | `d.user_writes_spec_format = false` | **HOLDS by design** — `ddis discover` conversations never expose format. |

---

## VIII. The Tagged Bottom Type: Current Status

**Not in the spec. Not in the code. Exists only in:**
- Session `a89f2818` (your proposal)
- Two cm rules (`b-mm2e6bpt-460dd5`, `b-mm2eqbm4-urn8dz`) capturing the notation `⊥[NAME]`
- Your memory of the design intent

The feature-discovery-state-template at `ddis-evolution/feature-discovery-state-template.json` (from your collaborator) has a `question_tree` with question statuses that could include "deferred", and an `artifact_map` where entries can be partial. But this is not a type-theoretic bottom type — it's a workflow status.

The formal gap: DDIS has no mechanism for a spec element to be **present but explicitly uninhabited** — known to be needed, tracked by ID, visible to validation, but without concrete content. The closest existing mechanism is an invariant with `Confidence: falsified` (never witnessed), but that's about implementation evidence, not specification completeness.

---

## IX. Recommendations (Ordered by Impact)

1. **Crystallize the tagged bottom type into the spec** — This is the missing bridge between informal exploration and formal specification. Define it as a first-class element type with its own invariant and ADR.

2. **Make `mode_observed` events durable** — Write them to the event stream instead of computing ephemerally. This preserves the cognitive provenance that is the entire point.

3. **Record `decision_crystallized` events during crystallize** — The crystallize code path must go through the event stream, not bypass it. APP-INV-025 is unenforceable otherwise.

4. **Implement `internal/events/` package** — The common envelope, cross-stream correlation, and schema validation. This is the largest single gap.

5. **Add a mechanical check for VCS-tracking** — A new validation check that verifies JSONL streams are not gitignored, preventing the surface bug that prompted this investigation from recurring.

---

## X. Meta-Conclusion

Your memory of the intent is accurate and well-grounded. The conversations exist in cass. The spec faithfully translates the vision into formal invariants and ADRs. But the implementation diverged from the spec in exactly the ways the system was designed to prevent — and the system didn't catch it, because the system itself was the thing that was incompletely implemented.

This is not a bug report. This is a **proof by construction** that the discovery event stream is necessary: the very failure you're investigating is the failure that proper event capture would have prevented. The provenance of "JSONL should be VCS-tracked" was lost because there was no provenance chain from your original statement (session `571bdf7e`) through the design discussion through the crystallized decision to the implementation. The conversation happened, but it wasn't captured in the event stream, because the event stream wasn't built yet.

The system needs to eat its own cooking — and right now, it's specified the recipe but hasn't finished building the kitchen.

---

## Appendix A: Key Session IDs and Timestamps

| Session ID | Timestamp (epoch ms) | Key Content |
|-----------|---------------------|-------------|
| `571bdf7e-e632-462f-8b9b-f037cdd43ece` | 1771868106517 | Original JSONL append-only log proposal |
| `bf05ea34-57b9-4abe-92b7-bff82604f34c` | 1771871275361 | Agent response: you already have event sourcing via oplog |
| `cf3b2349-0fde-448c-830c-749d7f88db4c` | 1771809825218 | Original tooling exploration (conversational iteration) |
| `a89f2818-3fe1-4968-bec6-dda4924a945f` | 1771909267776-1771916586278 | Drift planning, tagged bottom type, intent formalization |
| `bf0563ae-6113-4c9f-95c7-c0ec304ebf43` | 1771959804902 | User feedback: event-sourcing for time-travel, modes of cognition |
| `4d01234a-16fa-4137-b3da-a5fb5f400860` | 1771965061434-1771969779433 | Deep cognitive model, thread architecture, seven-mode taxonomy, JSONL event format |
| `eb53b2b8-425f-45ac-a4b9-7eb742582335` | 1772074504013 | Current session: user's definitive restatement of original intent |

## Appendix B: Spec Elements Analyzed

### Architecture Decision Records

| ADR | Title | Module | Purpose |
|-----|-------|--------|---------|
| APP-ADR-015 | Three-Stream Event Sourcing | code-bridge | Temporal backbone for lifecycle events |
| APP-ADR-017 | Gestalt Theory Integration | auto-prompting | Empirically-grounded prompt optimization |
| APP-ADR-018 | Observation over Prescription | auto-prompting | Cognitive autonomy preservation |
| APP-ADR-019 | Threads over Sessions | auto-prompting | Cognitive coherence unit |
| APP-ADR-020 | Conversational over Procedural | auto-prompting | Naturalistic discovery experience |
| APP-ADR-021 | Contributor Topology via Git Blame | auto-prompting | Epistemic structure detection |
| APP-ADR-022 | State Monad Architecture | auto-prompting | Pure CLI + LLM interpreter separation |
| APP-ADR-023 | LLMs as Primary Spec Authors | auto-prompting | Format serves validator, not author |
| APP-ADR-024 | Bilateral Specification / Inverse Principle | auto-prompting | Code gets a voice |
| APP-ADR-025 | Heuristic Scan over AST Parsing | auto-prompting | Language-agnostic absorption |
| APP-ADR-031 | Navigational Guidance as Postscript | auto-prompting | LLM-facing workflow navigation |
| APP-ADR-032 | Gestalt-Optimized CLI Output | query-validation | Failures-first, spec framing |

### Invariants

| INV | Title | Module | Purpose |
|-----|-------|--------|---------|
| APP-INV-020 | Event Stream Append-Only | code-bridge | Temporal record integrity |
| APP-INV-022 | Refinement Drift Monotonicity | auto-prompting | Improvement loop safety |
| APP-INV-023 | Prompt Self-Containment | auto-prompting | No implicit context dependencies |
| APP-INV-024 | Ambiguity Surfacing | auto-prompting | Human decides, not tool |
| APP-INV-025 | Discovery Provenance Chain | auto-prompting | Bridge organic thinking to formal spec |
| APP-INV-026 | Classification Non-Prescriptive | auto-prompting | Observe, never prescribe |
| APP-INV-027 | Thread Topology Primacy | auto-prompting | Threads as structure of thought |
| APP-INV-028 | Spec-as-Trunk | auto-prompting | No orphan thinking |
| APP-INV-029 | Convergent Thread Selection | auto-prompting | Invisible thread management |
| APP-INV-030 | Contributor Topology Graceful Degradation | auto-prompting | No hard git dependency |
| APP-INV-031 | Absorbed Artifacts Validate | auto-prompting | Absorption output quality |
| APP-INV-032 | Symmetric Reconciliation | auto-prompting | Bilateral gap analysis |
| APP-INV-033 | Absorption Format Parity | auto-prompting | No quality gap between authored and absorbed |
| APP-INV-034 | State Monad Universality | auto-prompting | Every command returns (output, state, guidance) |
| APP-INV-035 | Guidance Attenuation | auto-prompting | k* overprompting prevention |
| APP-INV-036 | Human Format Transparency | auto-prompting | Humans think, LLMs format |
