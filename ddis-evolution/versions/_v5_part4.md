# PART I: FOUNDATIONS

## Chapter 1: The Formal Model of a Specification

### 1.1 A Specification as a State Machine

A specification is itself a stateful artifact that transitions through well-defined phases:

```
States:
  Skeleton    — Structure exists but sections are empty
  Drafted     — All sections have initial content
  Threaded    — Cross-references connect all sections
  Tested      — Automated spec tests pass (Gate 8)
  Gated       — All quality gates pass (Gates 1–8)
  Validated   — External implementer confirms readiness (Gate 6 + Gate 7)
  Living      — In use, being updated as implementation reveals gaps
  Evolved     — Major version change; spec re-enters Drafted for affected sections

Transitions (with guards):
  Skeleton  →[fill_sections]→     Drafted
    Guard: every required section (§0.3) has non-empty content
  Drafted   →[add_cross_refs]→    Threaded
    Guard: every section is reachable in the reference graph;
           cross-references use machine-readable syntax ([[INV-022|parseable refs]])
  Threaded  →[run_spec_tests]→    Tested
    Guard: automated checks pass (Gate 8); all [[ID|substance]] refs resolve;
           proportional weight within tolerance ([[INV-021|weight compliance]])
  Tested    →[run_gates]→         Gated
    Guard: Gates 1–7 pass; all invariant restatements match source ([[INV-018|substance restated]])
  Gated     →[external_validate]→ Validated
    Guard: Gates 6–7 pass (human and LLM implementation readiness)
  Validated →[begin_impl]→        Living
    Guard: at least one implementer has confirmed readiness
  Living    →[discover_gap]→      Drafted
    Guard: gap is documented; regression is scoped to affected sections only
  Living    →[major_change]→     Evolved
    Guard: formal model or non-negotiable changes; version major increment
  Evolved   →[scope_drafted]→    Drafted
    Guard: affected sections identified; unchanged sections preserved

Invalid transition policy: Reject and log. A transition that skips phases
indicates incomplete specification work.

  Skeleton → Gated:     INVALID — cannot pass gates with empty sections
  Skeleton → Validated:  INVALID — cannot validate without content
  Drafted → Validated:   INVALID — cannot validate without cross-references
  Drafted → Gated:      INVALID — unthreaded specs cannot pass Gate 5
  Threaded → Gated:     INVALID — must pass automated tests (Tested) first
  Living → Skeleton:    INVALID — cannot regress past Drafted; gaps are patches
  Evolved → Skeleton:   INVALID — evolution patches sections, does not restart
```

// WHY the Tested state (new in 3.0)? Automated spec testing ([[INV-022|parseable cross-refs]], Gate 8) catches structural issues before expensive manual gate reviews. The Tested→Gated transition ensures structural soundness before semantic validation.

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

### 1.3 Complexity of Specification Elements

| Element | Authoring Complexity | Reading Complexity | Verification Complexity |
|---|---|---|---|
| Invariant | O(domain_understanding) | O(1) per invariant | O(1) (construct counterexample) |
| ADR | O(alternatives × analysis_depth) | O(alternatives) per ADR | O(1) (check genuine alternatives) |
| Algorithm | O(algorithm_complexity × edge_cases) | O(pseudocode_length) | O(worked_examples) |
| Cross-reference | O(1) per reference | O(1) per reference | O(1) automated with [[INV-022|parseable refs]] |
| End-to-end trace | O(subsystems × interactions) | O(subsystems) | O(1) (follow the trace) |
| Negative specification | O(domain_understanding) | O(1) per constraint | O(1) (check plausibility) |
| Verification prompt | O(invariants_per_chapter) | O(1) per chapter | O(1) (run the prompt) |

### 1.4 End-to-End Trace (of DDIS Itself)

This trace demonstrates DDIS coherence by following one element — an ADR — from the author's initial recognition of a decision through the DDIS authoring process to its final validated form.

**Scenario**: An author writing a domain spec (an event-sourced task scheduler) recognizes a decision: "Should the kernel loop be single-threaded or multi-threaded?"

**Step 1 — Recognition** ([[INV-002|every choice with reasonable alternative needs an ADR]]). The author realizes two reasonable alternatives exist. Per [[INV-002|decision completeness]], this requires an ADR.

**Step 2 — Formal model check** (§3.3). The author's first-principles model defines `Reducer: (State, Event) → State` with a determinism invariant. Both approaches are compatible with the model, confirming this is a genuine decision.

**Step 3 — ADR authoring** (§3.5). Following the required format:
- Problem: kernel concurrency model
- Confidence: Decided (validated by determinism invariant analysis)
- Options: (A) Single-threaded — serialized events, deterministic replay, no locking. (B) Multi-threaded with locks — higher throughput, complex reasoning, replay requires lock ordering. (C) Actor model — message passing, natural for agents, higher latency per event.
- Decision: (A) Single-threaded, citing the determinism invariant.
- WHY NOT (B)? Lock ordering makes replay non-trivial; replay is a non-negotiable.

**Step 4 — Cross-reference web** ([[INV-006|no orphan sections]], [[INV-018|substance restated]]). The author adds machine-readable references:
- From ADR → [[APP-INV-003|determinism: same events → identical state]]
- From ADR → the kernel implementation chapter
- From the kernel chapter → [[APP-ADR-003|single-threaded for deterministic replay]]

**Step 5 — Negative specification** ([[INV-017|negative spec per chapter]]). The kernel chapter states: "Must NOT spawn threads for event processing. Must NOT read wall-clock time during reduction. Must NOT acquire locks in the event loop."

**Step 6 — Verification prompt** ([[INV-019|self-check per chapter]]). The kernel chapter ends with: "Verify: Is your event loop single-threaded? Does your reducer avoid wall-clock reads? Can you replay 10K events and get byte-identical state?"

**Step 7 — Automated spec testing** ([[INV-022|parseable cross-refs]]). The cross-reference parser validates: all `[[ID|substance]]` references resolve; the kernel chapter has inbound and outbound references; no orphan sections.

**Step 8 — Quality gate validation**:
- Gate 2: kernel chapter → ADR-003 → INV-003 → formal model ✓
- Gate 3: reviewer finds no obvious alternative not in ADR-003 ✓
- Gate 5: kernel chapter has inbound and outbound references with substance ✓
- Gate 7: an LLM given only the kernel chapter + glossary + restated invariants identifies the single-threaded constraint, lists the negative specs, and produces a single-threaded event loop ✓
- Gate 8: automated parser confirms all references resolve and proportional weight is within tolerance ✓

**Step 9 — Conformance level check** ([[ADR-014|graduated conformance]]). The author's spec declares "Conformance: Standard." The validation confirms: invariants present ✓, ADRs with alternatives ✓, negative specs per chapter ✓, verification prompts ✓, machine-readable cross-refs ✓, Gates 1–7 pass ✓. Standard level satisfied.

**Step 10 — Implementation mapping** ([[INV-025|spec-to-code traceability]]). The author creates a mapping entry in the kernel chapter:
```
| Spec Element | Artifact | Type | Notes |
|---|---|---|---|
| APP-INV-003 (determinism) | src/kernel/event_loop.rs::reduce() | Enforces | Single-threaded reduce guarantees determinism |
| APP-INV-003 (determinism) | tests/replay_test.rs::test_deterministic_replay | Validates | Replays 10K events, byte-compares state |
| APP-ADR-003 (single-threaded) | src/kernel/event_loop.rs | Implements | No thread spawning in event loop module |
| Negative spec #1 (no threads) | src/kernel/event_loop.rs | Enforces | Module-level comment: 'MUST NOT spawn threads' |
| Negative spec #2 (no wall-clock) | tests/replay_test.rs::test_no_clock_reads | Validates | Assertion that replay produces identical state |
```
This mapping enables Pass 4 of the multi-pass workflow (§0.2.7): an LLM can verify that every spec element has at least one code artifact enforcing or validating it.

This trace exercises: element specs (§3.5, §3.8, §5.6), invariants ([[INV-002|decision completeness]], [[INV-006|cross-ref density]], [[INV-017|negative specs]], [[INV-018|substance restated]], [[INV-019|verification prompts]], [[INV-022|parseable refs]], [[INV-023|example correctness]], [[INV-025|spec-to-code traceability]]), quality gates (2, 3, 5, 7, 8, 9, 10), and the cross-reference web.

---
