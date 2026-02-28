---
module: triage-workflow
domain: triage
maintains: [APP-INV-063, APP-INV-064, APP-INV-065, APP-INV-066, APP-INV-067, APP-INV-068, APP-INV-069, APP-INV-070]
interfaces: [APP-INV-001, APP-INV-002, APP-INV-010, APP-INV-016, APP-INV-020, APP-INV-025, APP-INV-027, APP-INV-042, APP-INV-050, APP-INV-053, APP-INV-057, APP-INV-062]
implements: [APP-ADR-053, APP-ADR-054, APP-ADR-055, APP-ADR-056, APP-ADR-057]
adjacent: [lifecycle-ops, auto-prompting, workspace-ops, code-bridge]
negative_specs:
  - "Must NOT store mutable issue state — derive from event stream only"
  - "Must NOT suggest implementation before spec convergence"
  - "Must NOT close issues without complete evidence chain"
  - "Must NOT allow infinite triage loops — well-founded ordering must decrease"
  - "Must NOT require network for lifecycle state derivation"
---

# Triage Workflow Module

This module owns the recursive self-improvement triage workflow for the DDIS CLI. It defines the formal mechanisms by which issues are filed, triaged, specified, implemented, verified, and closed — all mediated by the CLI, all derived from append-only event streams, and all converging to a fixpoint via a well-founded ordering.

The architectural principle: **triage is a contractive endofunctor on the spec state space.** Each triage step maps state S to a successor F(S) such that the triage measure μ(S) = (open_issues, unspecified, drift) strictly decreases in the lexicographic ordering on ℕ³. The Spec Fitness Function F(S) ∈ [0,1] provides a continuous objective whose Lyapunov complement V(S) = 1 - F(S) unifies the discrete measure μ with the continuous fitness into a single convergence proof.

This module is its own first test case: it was specified, triaged, implemented, and verified through the workflow it defines (APP-INV-067). If the workflow cannot process its own creation, it is not complete.

**Invariants interfaced from other modules (cross-module reference completeness --- restated at point of use):**

- APP-INV-001: Round-Trip Fidelity --- parse then render produces byte-identical output (maintained by parse-pipeline). *Triage-generated spec modifications must survive parse-render round-trips; a triage step that corrupts whitespace during re-render breaks the fixpoint.*
- APP-INV-002: Validation Determinism --- results independent of clock, RNG, execution order (maintained by query-validation). *The spec-convergence gate (APP-INV-064) depends on deterministic validation: the same spec must always produce the same 17/17 result.*
- APP-INV-010: Oplog Append-Only --- no modification or deletion after write (maintained by lifecycle-ops). *Triage events appended to Stream 3 inherit the oplog append-only guarantee; retroactive editing would falsify the evidence chain.*
- APP-INV-016: Implementation Traceability --- valid Source/Tests/Validates-via paths (maintained by lifecycle-ops). *The evidence chain (APP-INV-065) requires that every claimed implementation trace resolves to existing files.*
- APP-INV-020: Event Stream Append-Only --- JSONL streams are immutable append logs (maintained by code-bridge). *Issue lifecycle state is derived from Stream 3 event replay (APP-ADR-053); mutation would corrupt the derived state machine.*
- APP-INV-025: Discovery Provenance Chain --- every artifact traces to source (maintained by auto-prompting). *Issue-discovery linkage (APP-INV-063) extends provenance from discovery threads to triaged issues.*
- APP-INV-027: Thread Topology Primacy --- threads are the primary organizational unit (maintained by auto-prompting). *Each triaged issue links to a discovery thread; thread identity governs the triage scope.*
- APP-INV-042: Guidance Emission --- every data command with findings emits guidance (maintained by auto-prompting). *The triage command emits ranked work queue as guidance for the next agent action.*
- APP-INV-050: Challenge-Witness Adjunction Fidelity --- challenge(witness(inv)) returns verdict (maintained by lifecycle-ops). *The verified→closed transition (APP-INV-065) depends on challenge returning a deterministic verdict for every witnessed invariant.*
- APP-INV-053: Event Stream Completeness --- every state-mutating command emits a typed event (maintained by lifecycle-ops). *Every triage lifecycle transition emits a corresponding Stream 3 event.*
- APP-INV-057: External Tool Graceful Degradation --- clear error with recovery guidance (maintained by workspace-ops). *Issue filing via `gh` degrades gracefully when gh is absent.*
- APP-INV-062: Lifecycle Reachability --- every reachable state has forward path to ValidatedSpec (maintained by lifecycle-ops). *The triage state machine must have no dead-end states; every issue has a path to closed or wont_fix.*

---

## Formal Foundation

### The Triage Endofunctor

Let **DDIS** denote the category whose objects are spec states S = (SpecFiles, Index, SearchState, OpLog, TxState, EventStreams, DiscoveryState, Workspace) and whose morphisms are CLI command sequences that transform one state into another.

Define the **triage endofunctor** T : DDIS → DDIS by:

```
T(S) = S' where S' is obtained by:
  1. Compute F(S) — the fitness function
  2. Identify the signal s* with maximal ΔF (the steepest descent direction)
  3. Execute the single CLI command that addresses s*
  4. S' = result of that command application
```

T is well-defined because F(S) is computable from the index (deterministic, offline) and the command set is finite.

### The Triage ⊣ Close Adjunction

The **triage** functor L : Issues → Spec and **close** functor R : Spec → Issues form an adjunction L ⊣ R:

```
Unit   η : Id_Issues → R ∘ L   (filing an issue then closing it recovers the issue)
Counit ε : L ∘ R → Id_Spec     (closing then re-triaging converges to identity on spec)
```

The triangle identities hold:
- R ∘ η = ε ∘ R  (closing a closed issue is idempotent)
- η ∘ L = L ∘ ε  (triaging a triaged issue is idempotent)

### Convergence via Well-Founded Ordering

Define μ : DDIS → ℕ³ by:

```
μ(S) = (open_issues(S), unspecified_elements(S), drift_score(S))
```

The lexicographic ordering on ℕ³ is well-founded (no infinite descending chains). Each triage step must satisfy:

```
μ(T(S)) <_lex μ(S)   ∨   μ(S) = (0, 0, 0)
```

The fixpoint is S* where μ(S*) = (0, 0, 0) — all issues closed, all elements specified, zero drift.

### Spec Fitness Function

F : DDIS → [0, 1] is defined as:

```
F(S) = w₁·V(S) + w₂·C(S) + w₃·(1-D(S)) + w₄·H(S) + w₅·(1-K(S)) + w₆·(1-I(S))

where:
  V(S) = validation_passed / validation_total          (validate signal)
  C(S) = coverage_pct                                  (coverage signal)
  D(S) = drift_score / max_drift                       (drift signal, inverted)
  H(S) = challenges_confirmed / challenges_total       (challenge health signal)
  K(S) = contradictions_found / invariant_pairs         (contradictions, inverted)
  I(S) = open_issues / total_issues                    (issue backlog, inverted)

  Weights: w = (0.20, 0.20, 0.20, 0.15, 0.15, 0.10), Σwᵢ = 1
```

The **Lyapunov function** V(S) = 1 - F(S) satisfies:
- V(S) ≥ 0 for all S
- V(S) = 0 iff S is the fixpoint (F(S) = 1.0)
- V(T(S)) < V(S) for all non-fixpoint S (strict decrease per triage step)

This unifies the discrete measure μ (gradient direction) with the continuous fitness F (objective) into a single convergence proof.

---

## Issue Lifecycle State Machine

### States

```
Q = {filed, triaged, specified, implementing, verified, closed, wont_fix}
```

Terminal states: {closed, wont_fix}. Initial state: filed.

### Transitions with Mechanical Preconditions

```
δ : Q × Event → Q

δ(filed, issue_triaged)         = triaged
  Precondition: discovery thread linked (APP-INV-063)
  Action: ddis issue triage <number>

δ(triaged, issue_specified)     = specified
  Precondition: validate = 17/17 AND drift = 0 for affected invariants (APP-INV-064)
  Action: auto-detected by ddis next / ddis triage --auto

δ(specified, issue_implementing) = implementing
  Precondition: first ddis witness for an affected invariant
  Action: ddis witness <inv-id> <db>

δ(implementing, issue_verified) = verified
  Precondition: ddis challenge --all (0 refuted for affected invariants) (APP-INV-065)
  Action: ddis challenge --all <db>

δ(verified, issue_closed)       = closed
  Precondition: evidence chain complete for all affected invariants (APP-INV-065)
  Action: ddis issue close <number>

δ(any, issue_wontfix)           = wont_fix
  Precondition: explicit --wont-fix flag with --reason
  Action: ddis issue close <number> --wont-fix --reason "..."

δ(verified, issue_triaged)      = triaged   [regression path]
  Precondition: challenge refuted an affected invariant
  Action: automatic on challenge refutation (APP-INV-066)
```

### State Derivation (Event Sourcing)

Issue state is NEVER stored mutably. It is derived from replay of Stream 3 events:

```
DeriveState(events, issueNumber) :=
  let relevant = filter(events, e => e.issue_number == issueNumber)
  let sorted   = sort(relevant, by: timestamp, ascending)
  foldl(applyTransition, filed, sorted)
```

This is the categorical semantics of APP-ADR-053: the state is the image of the event stream under the fold homomorphism.

---

## Invariants

This module maintains eight invariants. Each invariant is fully specified with all six components: plain-language statement, semi-formal expression, violation scenario, validation method, WHY THIS MATTERS annotation, and implementation trace.

---

**APP-INV-063: Issue-Discovery Linkage**

*Every triaged issue has a linked discovery thread containing at least one observation event. Filing alone is insufficient; triage creates the provenance chain from issue to investigation.*

```
FOR ALL issue i IN issues WHERE state(i) ∈ {triaged, specified, implementing, verified, closed}:
  EXISTS thread t IN discovery_threads:
    t.id = i.thread_id
    AND count(events(t)) >= 1
    AND events(t)[0].type IN {question_opened, finding_recorded}

WHERE:
  state(i) = DeriveState(stream3_events, i.number)
  events(t) = filter(stream1_events, e => e.thread_id = t.id)
  thread_id is set by the issue_triaged event payload
```

Violation scenario: An agent files issue #42 for "add caching layer" and immediately runs `ddis issue triage 42` without first opening a discovery thread. The issue transitions to `triaged` but has no linked investigation — no questions asked, no findings recorded. A second agent picks up issue #42 via `ddis triage --protocol` and sees "specified" as the next state, but has no understanding of what the issue actually requires because there is no discovery provenance. The agent implements a naive in-memory cache that conflicts with the existing stateless architecture. The root cause: triage without discovery produces issues with no intellectual foundation.

Validation: (1) Create an issue via `ddis issue file`. Attempt `ddis issue triage <n>` without a discovery thread. Verify the command returns an error requiring a `--thread` argument or existing thread linkage. (2) Open a discovery thread via `ddis discover --thread t-test`. Record one observation. Then run `ddis issue triage <n> --thread t-test`. Verify the event payload contains the thread ID. (3) Replay Stream 3 events for the issue and verify DeriveState produces `triaged` with a non-empty thread reference. (4) Query Stream 1 for the linked thread and verify at least one event exists.

// WHY THIS MATTERS: The triage-discovery link is the provenance chain from action to understanding. Without it, issues are just titles — context-free work items that any agent can interpret in any direction. The discovery thread IS the shared understanding. This invariant ensures that every triaged issue carries its intellectual history, enabling any agent to reconstruct the reasoning behind the work. This is the bilateral lifecycle in microcosm: issues flow from discovery (idea → issue), and closing flows back to spec (implementation → verification).

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/triage/state.go::DeriveIssueState`
- Source: `internal/cli/issue.go::runIssueTriage`
- Tests: `tests/triage_test.go::TestIssueDiscoveryLinkage`
- Validates-via: `internal/triage/state.go` (thread_id presence check in triaged state)

---

**APP-INV-064: Spec-Before-Code Gate**

*The `ddis next` command and `ddis triage --auto` suppress implementation suggestions for any issue whose affected spec elements have not converged. Convergence means: all 17 validation checks pass for the affected elements and drift score is 0. This is the mechanical enforcement of "formalize before you build."*

```
FOR ALL issue i IN issues WHERE state(i) = triaged:
  LET affected = affectedInvariants(i)
  LET converged = validate(db, focus=affected) = 17/17 AND drift(db, scope=affected) = 0
  ddis_next_suggests_implement(i) IMPLIES converged

WHERE:
  affectedInvariants(i) = invariant IDs extracted from issue body or discovery thread
  validate(db, focus=affected) runs validate --focus for each affected invariant
  drift(db, scope=affected) measures drift scoped to the affected elements
```

Violation scenario: An agent files issue #37 to "implement APP-INV-065 evidence chain." The spec for APP-INV-065 has only a stub — no semi-formal expression, no violation scenario. Validation Check 2 (falsifiability) fails for APP-INV-065. Despite this, `ddis next` suggests "implement APP-INV-065" because it only checks global validation, not scoped to the issue's affected elements. The agent writes code for a half-specified invariant, producing an implementation that matches the stub but not the intent. When the spec is later completed, the implementation must be rewritten. The root cause: premature implementation before spec convergence.

Validation: (1) Create an issue affecting APP-INV-063. Ensure APP-INV-063 has incomplete spec (missing violation scenario). Run `ddis next` and verify it does NOT suggest implementation for this issue; instead it suggests `ddis refine` or `ddis discover` to complete the spec. (2) Complete the APP-INV-063 spec so validate passes 17/17 and drift is 0. Run `ddis next` and verify it NOW suggests implementation. (3) Introduce a drift regression (modify implementation without updating spec). Run `ddis next` and verify it reverts to suggesting spec work, not implementation.

// WHY THIS MATTERS: This is the formal enforcement of the first cognitive discipline principle: "formalize before you build." Without this gate, the triage workflow degenerates into a task tracker — issues go straight from "filed" to "implementing" with no spec convergence step. The gate ensures that the bilateral lifecycle's spec phase completes before the implementation phase begins. This is not a soft guideline; it is a mechanical precondition checked by the CLI.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/triage/gate.go::SpecConverged`
- Source: `internal/cli/next.go::runNext` (triage-aware priority injection)
- Tests: `tests/triage_test.go::TestSpecBeforeCodeGate`
- Validates-via: `internal/triage/gate.go` (validation + drift check)

---

**APP-INV-065: Resolution Evidence Chain**

*`ddis issue close` requires a complete evidence chain for all invariants affected by the issue. A complete chain means: (a) each affected invariant has a non-stale witness, (b) each witness has been challenged with verdict "confirmed", and (c) no challenge has verdict "refuted" or "inconclusive" for any affected invariant. Closing without evidence is forbidden.*

```
FOR ALL issue i WHERE transition(i, closed):
  LET affected = affectedInvariants(i)
  FOR ALL inv IN affected:
    EXISTS witness w:
      w.invariant_id = inv
      AND w.status = 'valid'
      AND w.spec_hash = current_spec_hash(inv)
    AND EXISTS challenge c:
      c.invariant_id = inv
      AND c.verdict = 'confirmed'
      AND c.witness_id = w.id
    AND NOT EXISTS challenge c':
      c'.invariant_id = inv
      AND c'.verdict IN {refuted, inconclusive}
      AND c'.timestamp > c.timestamp

WHERE:
  transition(i, closed) means the issue_closed event is being emitted
  affectedInvariants(i) extracted from issue metadata or discovery thread
  current_spec_hash(inv) = content_hash of the invariant in the current DB
```

Violation scenario: Issue #50 affects APP-INV-064 and APP-INV-065. An agent runs `ddis witness APP-INV-064` and gets a valid witness. The agent then runs `ddis issue close 50` without witnessing APP-INV-065 or challenging either invariant. The issue transitions to "closed" despite having only partial evidence. A later audit finds that APP-INV-065 was never implemented — the closure was premature. The root cause: close without complete evidence chain.

Validation: (1) Create an issue affecting two invariants. Witness one but not the other. Attempt `ddis issue close`. Verify the command rejects with a list of missing evidence. (2) Witness both invariants. Attempt close without challenging. Verify rejection listing unchallenged invariants. (3) Challenge both with "confirmed" verdicts. Run `ddis issue close`. Verify success and the emitted `issue_closed` event contains the full evidence chain. (4) Re-challenge one with "refuted" after initial confirmation. Attempt close. Verify rejection due to the newer refutation.

// WHY THIS MATTERS: The evidence chain is the formal certificate of completion. Without it, "closed" means "someone decided to stop working on it" — a social claim, not a mechanical one. The chain makes closure verifiable by any agent: given the issue number, replay the event stream, check witnesses, check challenges, determine if closure was justified. This is the foundation of autonomous agent triage — no human gatekeeper needed because the evidence speaks for itself.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/triage/evidence.go::VerifyEvidenceChain`
- Source: `internal/cli/issue.go::runIssueClose`
- Tests: `tests/triage_test.go::TestCloseRequiresEvidenceChain`
- Validates-via: `internal/triage/evidence.go` (per-invariant witness + challenge check)

---

**APP-INV-066: Recursive Feedback**

*When `ddis challenge` returns a "refuted" verdict, the system suggests filing a remediation issue with strictly smaller scope than the original. The scope reduction is measured by the number of affected invariants: the remediation issue must affect a proper subset of the refuted issue's invariants.*

```
FOR ALL challenge c WHERE c.verdict = refuted:
  LET parent_issue = issueContaining(c.invariant_id)
  LET suggested = suggestedRemediationIssue(c)
  suggested.title CONTAINS c.invariant_id
  AND |suggested.affected_invariants| < |parent_issue.affected_invariants|
  AND suggested.affected_invariants ⊂ parent_issue.affected_invariants

WHERE:
  issueContaining(inv) = the issue whose affected set includes inv
  suggestedRemediationIssue(c) = the auto-generated issue suggestion
  |S| denotes cardinality of set S
  ⊂ denotes proper subset
```

Violation scenario: Issue #55 affects invariants {APP-INV-063, APP-INV-064, APP-INV-065}. The agent challenges APP-INV-064 and gets "refuted." The system suggests filing a remediation issue with the same three invariants as affected. The new issue #56 covers the identical scope — it is not smaller. When #56 is triaged, APP-INV-064 fails again, generating another issue with the same scope. The process loops indefinitely because the scope never decreases. The root cause: remediation issues without strict scope reduction violate the well-founded ordering.

Validation: (1) Challenge an invariant with a setup that produces "refuted." Verify the guidance output includes a suggested `ddis issue file` command. (2) Parse the suggested command and verify the title contains the specific refuted invariant ID. (3) Verify the suggested issue's affected invariants are a proper subset of the parent issue's affected invariants. (4) Verify that repeated refutation-suggestion cycles produce strictly decreasing scope (eventually reaching a singleton set, at which point the remediation targets a single invariant).

// WHY THIS MATTERS: Recursive feedback with strict scope reduction is the mechanism that guarantees termination of the triage loop. Without it, the workflow can cycle: refute → file → triage → refute → file → ... with no convergence. The proper subset requirement maps directly to the well-founded ordering on ℕ: each remediation issue has fewer affected invariants, so the chain must terminate. This is the constructive content of APP-INV-068 (fixpoint termination) — the well-founded ordering decreases because scope decreases.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/triage/feedback.go::SuggestRemediationIssue`
- Source: `internal/cli/challenge.go::runChallengeSingle` (refutation guidance)
- Tests: `tests/triage_test.go::TestRefutationSuggestsIssue`
- Validates-via: `internal/triage/feedback.go` (proper subset check)

---

**APP-INV-067: Self-Bootstrap Closure**

*This triage-workflow module processes its own implementation through the complete lifecycle: filed → triaged → specified → implementing → verified → closed. The event stream contains all 7 triage event types in chronological order for the bootstrap issue. The module is both the specification and the test case.*

```
EXISTS issue i IN issues:
  i.title CONTAINS "triage workflow"
  AND DeriveState(events, i.number) = closed
  AND LET history = eventHistory(i.number)
  AND {issue_triaged, issue_specified, issue_implementing,
       issue_verified, issue_closed} ⊆ types(history)
  AND isChronological(history)

WHERE:
  eventHistory(n) = filter(stream3_events, e => e.issue_number = n)
  types(history) = {e.type | e ∈ history}
  isChronological(h) = ∀ i < j: h[i].timestamp ≤ h[j].timestamp
```

Violation scenario: The triage workflow module is specified and implemented, but the bootstrap issue is closed via `git commit` and manual status update — bypassing the `ddis issue close` command. The event stream contains no `issue_closed` event for the bootstrap issue. A later `ddis triage --auto` check finds that the module's own lifecycle is incomplete: all 7 event types are required but only 5 are present. The self-bootstrap property is violated — the module could not process itself through its own workflow. The root cause: manual workaround instead of using the workflow.

Validation: (1) At end of implementation, query Stream 3 for events with the bootstrap issue number. (2) Verify all 7 triage event types are present. (3) Verify chronological ordering. (4) Verify DeriveState produces `closed`. (5) Run `ddis triage --auto --history` and verify the bootstrap issue appears in the fitness trajectory with F reaching the target.

// WHY THIS MATTERS: Self-bootstrap closure is the ultimate consistency check. If the workflow cannot process its own creation, it has a fundamental gap — either the spec is underspecified, the implementation is incomplete, or the lifecycle has a dead-end state. This invariant forces the developers to eat their own cooking. Every gap in the workflow is discovered during its own implementation because the implementation IS the first test case. The category-theoretic interpretation: the triage endofunctor must have a fixpoint, and the bootstrap issue is a witness for that fixpoint.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/triage/state.go::DeriveIssueState`
- Source: `internal/events/schema.go` (Stream 3 event types)
- Tests: `tests/triage_test.go::TestSelfBootstrapClosure`
- Validates-via: Event stream replay for bootstrap issue number

---

**APP-INV-068: Fixpoint Termination**

*The triage measure μ(S) = (open_issues, unspecified, drift) decreases lexicographically with each triage step. No infinite sequence of triage steps is possible — the well-founded ordering on ℕ³ guarantees termination. The fixpoint μ(S*) = (0, 0, 0) is reached in finitely many steps.*

```
FOR ALL spec states S, S' = T(S):
  μ(S) ≠ (0, 0, 0) IMPLIES μ(S') <_lex μ(S)

WHERE:
  μ(S) = (|{i ∈ issues(S) : state(i) ∉ {closed, wont_fix}}|,
          |{e ∈ spec_elements(S) : ¬specified(e)}|,
          drift_score(S))
  <_lex is the lexicographic ordering:
    (a₁,a₂,a₃) <_lex (b₁,b₂,b₃) iff
      a₁ < b₁ ∨ (a₁ = b₁ ∧ a₂ < b₂) ∨ (a₁ = b₁ ∧ a₂ = b₂ ∧ a₃ < b₃)
  T is the triage endofunctor (one step of the triage loop)
```

Violation scenario: An agent runs `ddis triage --auto` which files 3 new issues (increasing open_issues from 2 to 5). The triage step was supposed to close issues, but instead it discovered new deficiencies and filed them. μ went from (2, 1, 0) to (5, 1, 0) — the first component increased. The well-founded ordering was violated: the triage loop may now cycle indefinitely as each step discovers more issues than it resolves. The root cause: auto-filing without bounding the number of new issues relative to closed issues.

Validation: (1) Record μ(S) before a triage step. Execute one step via `ddis triage --auto`. Record μ(S'). Verify μ(S') <_lex μ(S). (2) Repeat for 10 consecutive steps, verifying strict decrease each time. (3) Set up a state where μ = (1, 0, 0) with one closeable issue. Execute triage step. Verify μ = (0, 0, 0) — fixpoint reached. (4) Set up a state where a triage step would file new issues: verify the step closes at least as many issues as it files (net non-positive change to open_issues) OR reduces a lower component.

// WHY THIS MATTERS: Termination is the fundamental property that separates a convergent workflow from an infinite loop. Without it, `ddis triage --auto` could run forever — filing issues, triaging them, discovering problems, filing more issues, in a divergent spiral. The well-founded ordering on ℕ³ is the mathematical certificate that this cannot happen. The lexicographic structure gives priority to the most impactful dimension: closing issues first, then completing specs, then reducing drift. This ordering is not arbitrary — it reflects the causal dependencies in the bilateral lifecycle.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/triage/measure.go::ComputeMeasure`
- Source: `internal/cli/triage.go::runTriageAuto`
- Tests: `tests/triage_test.go::TestTriageMeasureDecreases`
- Validates-via: `internal/triage/measure.go` (lexicographic comparison)

---

**APP-INV-069: Triage Monotonic Fitness**

*F(S') ≥ F(S) after every triage step, where F is the Spec Fitness Function. F(S) = 1.0 if and only if the spec has reached fixpoint: all validation checks pass, full coverage, zero drift, all challenges confirmed, no contradictions, and no open issues. A fitness decrease is flagged as regressive and triggers a remediation issue.*

```
FOR ALL spec states S, S' = T(S):
  F(S') >= F(S)
  AND (F(S) = 1.0 IFF
    V(S) = 1.0 AND C(S) = 1.0 AND D(S) = 0.0
    AND H(S) = 1.0 AND K(S) = 0.0 AND I(S) = 0.0)
  AND (F(S') < F(S) IMPLIES
    autoFileIssue("Fitness regression: F decreased from " + F(S) + " to " + F(S')))

WHERE:
  F(S) = 0.20·V(S) + 0.20·C(S) + 0.20·(1-D(S)) + 0.15·H(S) + 0.15·(1-K(S)) + 0.10·(1-I(S))
  T is the triage endofunctor
  autoFileIssue is the remediation mechanism from APP-INV-066
```

Violation scenario: An agent runs `ddis triage --auto` which reports F(S) = 0.87. The agent then adds a new invariant to the spec (expanding coverage scope). Re-running fitness gives F(S') = 0.82 because the new invariant is unwitnessed and unchallenged, dragging down H(S). The fitness decreased by 0.05 but no regression issue was filed. Future agents see F = 0.82 and have no record of the regression event. The root cause: fitness regression without automatic detection and remediation.

Validation: (1) Compute F(S) for a known state. Execute a triage step. Compute F(S'). Verify F(S') >= F(S). (2) Artificially introduce a regression (e.g., break a validation check). Run `ddis triage --auto`. Verify the output flags the regression and suggests filing a remediation issue. (3) Verify F(S) = 1.0 exactly when all six signals are perfect: V=1, C=1, D=0, H=1, K=0, I=0. (4) Verify F(S) < 1.0 when any signal is imperfect: set each signal to a non-ideal value individually and verify F < 1.0.

// WHY THIS MATTERS: Monotonic fitness is the continuous-domain analog of fixpoint termination (APP-INV-068). While μ guarantees termination in ℕ³, F provides a smooth gradient that enables intelligent prioritization. The fitness function IS the triage oracle — it tells agents which work has the highest marginal value. The Lyapunov complement V(S) = 1 - F(S) unifies both convergence proofs: V is a Lyapunov function (non-negative, zero at fixpoint, strictly decreasing) and μ is its discrete shadow. Regression detection prevents silent quality erosion — every fitness decrease is recorded and tracked.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/triage/fitness.go::ComputeFitness`
- Source: `internal/cli/triage.go::runTriageAuto`
- Tests: `tests/triage_test.go::TestTriageFitnessMonotonic`
- Tests: `tests/triage_test.go::TestFitnessFixpoint`
- Validates-via: `internal/triage/fitness.go` (weighted signal combination)

---

**APP-INV-070: Protocol Completeness**

*The output of `ddis triage --protocol` is a self-contained JSON document sufficient for any agent — with zero prior DDIS knowledge — to drive the triage lifecycle to fixpoint. The protocol includes: current fitness, ranked work queue, issue lifecycle states, transition preconditions, convergence metrics, and the Lyapunov trajectory. An agent executing `ranked_work[0].action`, then re-running `--protocol`, and repeating, will converge to F(S) = 1.0.*

```
LET P = output(ddis triage --protocol)
FOR ALL agents A with no prior DDIS knowledge:
  LET loop(S) =
    if P.fitness.current = 1.0 then S
    else let S' = execute(A, P.ranked_work[0].action, S) in
         let P' = output(ddis triage --protocol, S') in
         loop(S')
  loop(S₀) terminates AND F(loop(S₀)) = 1.0

WHERE:
  P contains: version, spec_id, fitness, measure, issues, ranked_work, convergence
  P.ranked_work is ordered by delta_f (estimated fitness improvement, descending)
  P.convergence.lyapunov_decreasing = true at every step
  "sufficient" means: P alone (no DDIS manual, no context bundles) enables A to act
```

Violation scenario: An external CI agent receives the protocol JSON from `ddis triage --protocol`. The agent reads `ranked_work[0].action = "ddis witness APP-INV-063 manifest.ddis.db --type test"` and executes it. But the witness command requires `--evidence` and `--by` flags not mentioned in the protocol. The command fails. The agent has no recovery path because the protocol was its only source of information. The root cause: protocol output is not self-contained — it omits required flags.

Validation: (1) Run `ddis triage --protocol --json`. Parse the JSON output. Verify it contains all required fields: version, spec_id, fitness, measure, issues, ranked_work, convergence. (2) For each action in ranked_work, verify it is a complete, executable CLI command (no missing required flags). (3) Execute the first ranked_work action. Re-run `--protocol`. Verify fitness.current >= previous fitness.current. (4) Verify convergence.lyapunov_decreasing is true. (5) Run the protocol loop (execute top action, re-run protocol, repeat) for up to 20 iterations. Verify convergence to F = 1.0 or stable state.

// WHY THIS MATTERS: Protocol completeness is the foundation of autonomous multi-agent triage. If the protocol is sufficient, any agent — Claude, GPT, Gemini, a bash script — can participate in spec improvement without knowing what DDIS is. The protocol is the universal API for the triage endofunctor: it externalizes the state, transitions, and objective into a self-describing document. The Lyapunov trajectory provides convergence evidence, so agents can verify they are making progress. This is the practical realization of the category-theoretic framework: the protocol IS the functorial image of the spec state.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/triage/protocol.go::GenerateProtocol`
- Source: `internal/cli/triage.go::runTriageProtocol`
- Tests: `tests/triage_test.go::TestProtocolCompleteness`
- Tests: `tests/triage_test.go::TestProtocolConvergence`
- Tests: `tests/triage_test.go::TestProtocolSoundness`
- Validates-via: `internal/triage/protocol.go` (JSON schema validation)

---

## Architecture Decision Records

This module implements five architecture decisions. Each ADR is fully specified with Problem, Options, Decision, WHY NOT, Consequences, and Implementation Trace.

---

### APP-ADR-053: Issue Lifecycle as Event-Sourced State Machine

#### Problem

How should issue lifecycle state (filed, triaged, specified, etc.) be persisted? The two options are: (a) a mutable `issues` table in the SQLite database with a `status` column, or (b) event-sourced derivation from Stream 3 events.

#### Options

1. **Mutable table** — simple SQL UPDATE on state transitions
2. **Event sourcing** — derive state from event replay

#### Decision

**Option B: Event sourcing.** Issue state is derived from replay of Stream 3 events. There is no `issues` table with a mutable `status` column. The function `DeriveIssueState(events, issueNumber)` replays all events for an issue and returns the current state.

WHY NOT mutable table: A mutable status column creates a second source of truth alongside the event stream. If the events say "triaged" but the table says "specified", which is canonical? Event sourcing eliminates this ambiguity by construction — the events ARE the state. This is consistent with APP-INV-020 (event stream append-only) and the oplog philosophy (APP-ADR-007): immutable records as primary data.

#### Consequences

- State derivation requires replaying events (O(n) in number of events per issue). For the expected scale (< 1000 events per issue), this is negligible.
- No database migrations needed for state schema changes — just update the fold function.
- Event stream is the complete audit trail — every state transition is recorded with timestamp and payload.
- Debugging is easier: replay events to reproduce any historical state.

#### Tests

- `tests/triage_test.go::TestDeriveState_EmptyEvents` — empty stream returns `filed`
- `tests/triage_test.go::TestDeriveState_FullLifecycle` — walks filed→triaged→specified→implementing→verified→closed
- `tests/triage_test.go::TestDeriveState_InvalidTransition` — rejects e.g. filed→verified

**Implementation Trace:**
- Source: `internal/triage/state.go::DeriveIssueState`
- Source: `internal/events/schema.go` (Stream 3 event types)
- Tests: `tests/triage_test.go::TestDeriveState_FullLifecycle`

---

### APP-ADR-054: Fixpoint Convergence via Well-Founded Ordering

#### Problem

How do we guarantee that the recursive triage loop terminates? Without a termination argument, `ddis triage --auto` could cycle indefinitely.

#### Options

1. **Iteration cap** — hard limit of N iterations
2. **Well-founded ordering** — mathematical proof of termination via ℕ³ ordering
3. **Timeout** — stop after T seconds

#### Decision

**Option B: Well-founded ordering.** Define μ : DDIS → ℕ³ as μ(S) = (open_issues, unspecified, drift). The lexicographic ordering on ℕ³ is well-founded (equivalent to ordinal ω³). Each triage step must produce μ(S') <_lex μ(S) or reach the fixpoint μ = (0, 0, 0). This is enforced mechanically: `ComputeMeasure` is called before and after each step, and a violation (non-decrease) halts the loop with an explicit error.

WHY NOT iteration cap: An iteration cap is an engineering workaround, not a proof. It says "we give up after N steps" instead of "we converge." The well-founded ordering says "we WILL converge, and here is the mathematical certificate." Caps also mask bugs: if the loop cycles, a cap hides the cycle instead of detecting it.

WHY NOT timeout: Same problem as iteration cap, but worse — termination depends on wall-clock time, violating determinism (APP-INV-002).

#### Consequences

- Termination is provable, not empirical.
- Each triage step must be designed to decrease at least one component of μ.
- Auto-filing new issues is constrained: net issue count must not increase (or a lower component must decrease to compensate).
- The fixpoint (0, 0, 0) is a concrete, testable condition.

#### Tests

- `tests/triage_test.go::TestTriageMeasureDecreases` — 10 consecutive steps with strict lexicographic decrease
- `tests/triage_test.go::TestLexLess` — exhaustive comparison of ℕ³ triples

**Implementation Trace:**
- Source: `internal/triage/measure.go::ComputeMeasure`
- Source: `internal/triage/measure.go::LexLess`
- Tests: `tests/triage_test.go::TestTriageMeasureDecreases`

---

### APP-ADR-055: Full Agent Autonomy with Spec-Convergence Guardrails

#### Problem

How much autonomy should an AI agent have in the triage lifecycle? Options range from fully manual (human approves every transition) to fully autonomous (agent drives the entire lifecycle).

#### Options

1. **Fully manual** — human approves every state transition
2. **Fully autonomous** — agent drives lifecycle end-to-end
3. **Guardrailed autonomy** — full autonomy between mechanical gates

#### Decision

**Option C: Guardrailed autonomy.** Two mechanical gates are enforced by the CLI:

1. **Spec-convergence gate** (triaged → specified): validate = 17/17 AND drift = 0 for affected elements. This prevents premature implementation.
2. **Evidence-chain gate** (verified → closed): complete witness + challenge chain for all affected invariants. This prevents premature closure.

Between these gates, agents have full autonomy: they choose which invariants to witness, which order to implement, how to structure code. The gates are observational (APP-ADR-043), not prescriptive — they check postconditions, not dictate process.

WHY NOT fully manual: Human-in-the-loop for every transition creates a bottleneck. The mechanical gates are more rigorous than human judgment — they check actual spec state, not subjective readiness.

WHY NOT fully autonomous: Without gates, agents skip directly from "filed" to "closed" with no evidence. The gates are the minimal constraint set that ensures quality.

#### Consequences

- Agents can work independently between gates.
- Gates are mechanically checked — no human judgment required.
- The two gates correspond to the two critical invariants: APP-INV-064 (spec-before-code) and APP-INV-065 (evidence chain).
- New gates can be added by adding new invariants with mechanical preconditions.

#### Tests

- `tests/triage_test.go::TestAgentAutonomyLoop` — full lifecycle with gate enforcement
- `tests/triage_test.go::TestSpecBeforeCodeGate` — triaged→specified blocked without convergence
- `tests/triage_test.go::TestCloseRequiresEvidenceChain` — verified→closed blocked without evidence

**Implementation Trace:**
- Source: `internal/triage/gate.go::SpecConverged`
- Source: `internal/triage/evidence.go::VerifyEvidenceChain`
- Tests: `tests/triage_test.go::TestAgentAutonomyLoop`

---

### APP-ADR-056: Spec Fitness Function as Endogenous Quality Signal

#### Problem

How should the triage system prioritize work? Options include manual priority assignment, heuristic rules, or an endogenous quality signal derived from spec state.

#### Options

1. **Manual priority** — human assigns P0-P4 labels
2. **Heuristic rules** — if validation fails then P0, if drift > 0 then P1, etc.
3. **Fitness function** — weighted combination of normalized quality signals

#### Decision

**Option C: Fitness function.** F(S) = Σ wᵢ · sᵢ(S) where sᵢ are 6 normalized quality signals (validation, coverage, drift, challenges, contradictions, open issues) and wᵢ are fixed weights summing to 1.0. The ranked work queue is ordered by estimated marginal fitness improvement ΔF per unit work.

The weights w = (0.20, 0.20, 0.20, 0.15, 0.15, 0.10) reflect the causal structure: validation, coverage, and drift are the foundation (higher weight); challenge health and contradictions are verification quality (medium weight); open issues are process state (lower weight).

WHY NOT manual priority: Requires human judgment that may conflict with spec state. An issue marked P0 by a human may be less impactful than a P2 that fixes a validation failure.

WHY NOT heuristic rules: Heuristics are fragile and don't compose. The fitness function provides a single scalar that enables gradient descent on spec quality.

#### Consequences

- `ddis triage --auto` has a principled ranking algorithm.
- The Lyapunov complement V(S) = 1 - F(S) provides convergence evidence.
- Weights are fixed in code (not configurable) to prevent gaming.
- F(S) trajectory over time measures spec improvement velocity.

#### Tests

- `tests/triage_test.go::TestTriageAutoRanking` — deficiencies ranked by ΔF descending
- `tests/triage_test.go::TestFitnessFixpoint` — F=1.0 iff all signals perfect
- `tests/triage_test.go::TestTriageFitnessMonotonic` — F non-decreasing over triage steps

**Implementation Trace:**
- Source: `internal/triage/fitness.go::ComputeFitness`
- Source: `internal/triage/fitness.go::RankDeficiencies`
- Tests: `tests/triage_test.go::TestTriageAutoRanking`
- Tests: `tests/triage_test.go::TestFitnessFixpoint`

---

### APP-ADR-057: Agent-Executable Protocol for Zero-Knowledge Participation

#### Problem

How can an AI agent that knows nothing about DDIS participate in spec improvement? The agent needs a self-contained document that describes the current state, valid actions, and convergence criteria.

#### Options

1. **Documentation** — point the agent at the DDIS spec
2. **Context bundle** — extend `ddis context` with triage state
3. **Self-contained protocol** — `ddis triage --protocol` emits executable JSON

#### Decision

**Option C: Self-contained protocol.** `ddis triage --protocol` emits a JSON document containing: spec fitness (current + trajectory), triage measure, issue lifecycle states with valid transitions, ranked work queue with executable CLI commands, and convergence metrics (Lyapunov value, estimated steps to fixpoint). The protocol is sufficient for any agent to execute `ranked_work[0].action`, re-run `--protocol`, and repeat until `fitness.current == 1.0`.

The Lyapunov function V(S) = 1 - F(S) unifies the discrete measure μ (gradient direction) with the continuous fitness F (objective). The ranked work queue IS gradient descent on the Lyapunov surface.

WHY NOT documentation: Requires the agent to read and understand the DDIS spec — a high barrier for a zero-knowledge participant.

WHY NOT context bundle: Context bundles are read-only; they don't include executable actions or convergence criteria.

#### Consequences

- Any AI agent can participate in triage without DDIS training.
- The protocol is the universal API for the triage endofunctor.
- Convergence is verifiable: agents can check that Lyapunov is decreasing.
- The protocol enables CI/CD integration: a pipeline can run `ddis triage --protocol --json` and execute the top-ranked action.

#### Tests

- `tests/triage_test.go::TestProtocolCompleteness` — all required JSON fields present
- `tests/triage_test.go::TestProtocolConvergence` — repeated execute→protocol loop converges
- `tests/triage_test.go::TestProtocolSoundness` — preconditions prevent invalid transitions

**Implementation Trace:**
- Source: `internal/triage/protocol.go::GenerateProtocol`
- Source: `internal/cli/triage.go::runTriageProtocol`
- Tests: `tests/triage_test.go::TestProtocolCompleteness`
- Tests: `tests/triage_test.go::TestProtocolConvergence`

---

## Implementation Chapters

### Chapter 1: Event-Sourced State Machine

The issue lifecycle state machine is implemented as a pure function over the event stream. No mutable state is stored — all state is derived from event replay. This chapter specifies the event schema extension (6 new Stream 3 event types), the state derivation function, and the transition validation logic.

**Event types (Stream 3 extension):**

| Event Type | Trigger | Payload |
|---|---|---|
| `issue_triaged` | `ddis issue triage` | issue_number, thread_id |
| `issue_specified` | auto-detected by triage | issue_number, affected_invariants, validation_result |
| `issue_implementing` | first `ddis witness` for affected invariant | issue_number, invariant_id, witness_id |
| `issue_verified` | `ddis challenge --all` with 0 refuted | issue_number, challenge_batch_id |
| `issue_closed` | `ddis issue close` | issue_number, evidence_chain |
| `issue_wontfix` | `ddis issue close --wont-fix` | issue_number, reason |

All 6 event types are registered in `events/schema.go` under `StreamImplementation` (Stream 3). The `ValidateEvent` function enforces that each event type belongs to its declared stream — a triage event written to Stream 1 (discovery) is rejected. Each event carries a mandatory `issue_number` field in its payload, establishing the join key for state derivation.

**DeriveIssueState algorithm:**

The state derivation function is the fold homomorphism from the event monoid to the state lattice. It processes events in timestamp order and applies the transition function at each step. The function is total: every valid event sequence produces a well-defined state; invalid sequences produce explicit errors.

```
func DeriveIssueState(events []Event, issueNumber int) (State, error):
  state = Filed
  for each event in sorted(events, by=timestamp):
    if event.issue_number != issueNumber: continue
    next = transitionTable[state][event.type]
    if next == nil: return error("invalid transition from " + state + " via " + event.type)
    state = next
  return state
```

The transition table encodes the state machine from §Transitions above. Invalid transitions return errors, never silently discard events. The table is a 7×6 sparse matrix (7 states × 6 event types) with exactly 7 valid entries (6 forward transitions + 1 regression path). All other cells contain `nil`, making invalid transitions a hard error.

**Companion functions:**

- `DeriveAllIssueStates(events) → map[int]State` — derives state for every known issue number in a single pass (O(n) in total events)
- `NextValidTransitions(state) → []EventType` — returns the set of event types valid from the current state (used by protocol generation)
- `AffectedInvariants(issueNumber, events) → []string` — extracts invariant IDs from the issue's discovery thread and triage events

**Edge-case handling:**

- **Duplicate events:** If the same event type is emitted twice for the same issue (e.g., two `issue_triaged` events), the state machine processes both. The second event is a no-op if the state already advanced past it; otherwise it produces an invalid-transition error. Event deduplication is the responsibility of the event writer, not the state machine.
- **Out-of-order timestamps:** Events are sorted by timestamp before processing. If two events share the same timestamp, they are sub-sorted by event type ordinal (the order in the StreamImplementation registration). This deterministic tie-breaking ensures identical state derivation across replays.
- **Partial sequences:** When events reference an unknown `issue_number`, `DeriveIssueState` returns the zero state (`Filed`) — the absence of events for an issue is interpreted as "just filed, no actions taken."

### Chapter 2: Triage Measure and Fitness

The triage measure μ and fitness function F are computed from the spec index and event stream. Both are deterministic and offline — no network calls, no randomness. Together they provide the convergence machinery: μ guarantees termination (well-founded ordering), F provides the gradient for intelligent prioritization.

**ComputeMeasure algorithm:**

The measure extracts three components from the current state. Each component maps to a distinct quality dimension: issue resolution, spec completeness, and implementation alignment.

```
func ComputeMeasure(db *DB, events []Event) Measure:
  openIssues = count(issues where DeriveState != closed and != wont_fix)
  unspecified = count(spec elements without complete specification)
  driftScore = computeDrift(db)
  return Measure{openIssues, unspecified, driftScore}
```

The lexicographic comparison `LexLess(a, b Measure) bool` implements the well-founded ordering. Priority flows left-to-right: open_issues dominates unspecified, which dominates drift. This ordering reflects causal dependency: issues must be resolved before specs can be completed, and specs must be completed before drift is meaningful.

**ComputeFitness algorithm:**

The fitness function is a convex combination of 6 normalized signals. Each signal is independently computable from the spec index or event stream. The weights are fixed constants — not user-configurable — to prevent gaming.

```
func ComputeFitness(db *DB, events []Event) FitnessResult:
  v = runValidate(db).passed / runValidate(db).total
  c = runCoverage(db).pct
  d = runDrift(db).score / maxDrift
  h = challengeHealth(db)
  k = contradictions(db) / invariantPairs(db)
  i = openIssues(events) / totalIssues(events)
  f = 0.20*v + 0.20*c + 0.20*(1-d) + 0.15*h + 0.15*(1-k) + 0.10*(1-i)
  return FitnessResult{Score: f, Signals: [v, c, d, h, k, i]}
```

**RankDeficiencies algorithm:**

For each signal where score < 1.0, identify specific deficiencies and estimate ΔF — the fitness improvement from addressing each deficiency. The ranked list is sorted by ΔF descending, giving agents a greedy approximation to steepest descent on the Lyapunov surface V(S) = 1 - F(S).

**Edge cases:** When totalIssues = 0, I(S) = 0 (no backlog — perfect). When maxDrift = 0, D(S) = 0 (no drift — perfect). When challengesTotal = 0, H(S) defaults to 1.0 (no challenges needed). Division by zero is guarded at every signal computation.

**Signal independence:** Each of the 6 signals (v, c, d, h, k, i) is computed from a disjoint subset of the database state, making them independently testable. Validation (v) reads the check results, coverage (c) reads module-to-invariant mappings, drift (d) reads spec-impl annotations, challenge health (h) reads challenge_results, consistency (k) reads invariant pairs, and issue backlog (i) reads the event stream. No signal computation depends on another signal's output, which guarantees that fitness is a pure function of the spec state alone.

**Normalization bounds:** All signals are clamped to [0, 1], ensuring F(S) is bounded in [0, 1]. The Lyapunov function V(S) = 1 - F(S) is therefore bounded in [0, 1] and non-negative, satisfying the definiteness requirement for convergence proofs.

**Worked example:** Given a spec with 17 validation checks (16 passing), 70 invariants at 100% coverage, drift score 1 with maxDrift 10, 63/63 challenges confirmed, 0 contradictions in 2415 pairs, and 0/0 open issues: v=16/17≈0.941, c=1.0, d=1/10=0.1, h=1.0, k=0, i=0. F = 0.20(0.941) + 0.20(1.0) + 0.20(0.9) + 0.15(1.0) + 0.15(1.0) + 0.10(1.0) = 0.9882. V(S) = 0.0118. The dominant deficiency is validation (ΔF ≈ 0.20 × 0.059 = 0.0118), matching the Lyapunov gap.

**Lyapunov decrease guarantee:** Each ranked deficiency's ΔF estimate is a lower bound on the fitness improvement achievable by addressing that single item. The ranking by ΔF descending ensures that the greedy strategy (fix highest-ΔF first) achieves the steepest descent. Since each remediation either fixes the deficiency (ΔF > 0) or reveals it as a false positive (no change), the sequence F(S₀), F(S₁), ... is monotonically non-decreasing. Combined with the well-founded ordering on μ, this guarantees termination at F(S) = 1.0.

### Chapter 3: Evidence Chain Verification

The evidence chain is the formal certificate of issue completion. It verifies that every affected invariant has a non-stale witness with a confirmed challenge verdict. The chain is the categorical product of per-invariant evidence — completeness requires ALL invariants to have evidence, not just some.

**VerifyEvidenceChain algorithm:**

The verification proceeds per-invariant and collects all violations before returning. This all-or-nothing semantics ensures that `ddis issue close` never partially closes — it either succeeds with a complete chain or fails with a comprehensive violation list.

```
func VerifyEvidenceChain(db *DB, issueNumber int, events []Event) (*EvidenceChain, []Violation):
  affected = getAffectedInvariants(issueNumber, events)
  violations = []
  for each inv in affected:
    witness = getLatestWitness(db, inv)
    if witness == nil: violations.append(MissingWitness{inv})
    else if witness.specHash != currentSpecHash(db, inv): violations.append(StaleWitness{inv})
    challenge = getLatestChallenge(db, inv)
    if challenge == nil: violations.append(MissingChallenge{inv})
    else if challenge.verdict != "confirmed": violations.append(NonConfirmed{inv, challenge.verdict})
  if len(violations) > 0: return nil, violations
  return buildChain(affected, witnesses, challenges), nil
```

**Violation types:**

| Type | Meaning | Remedy |
|---|---|---|
| `MissingWitness` | Invariant has no witness record | `ddis witness <inv-id> <db>` |
| `StaleWitness` | Witness spec_hash differs from current | Re-witness after spec change |
| `MissingChallenge` | Witness exists but never challenged | `ddis challenge <inv-id> <db>` |
| `NonConfirmed` | Challenge verdict is not "confirmed" | Fix implementation, re-witness, re-challenge |

The violation list is rendered as a checklist by `ddis issue close`, showing each missing element and the exact command to provide it. This is the operational content of APP-INV-042 (guidance emission) applied to the close precondition.

**SpecConverged gate:**

The spec-convergence gate (APP-INV-064) is a strict precondition at the triaged→specified transition. It runs validation scoped to the issue's affected invariants and checks drift for those elements. Convergence is binary — partial convergence is not sufficient. The gate returns a list of non-converged elements with specific remediation commands.

**Staleness detection algorithm:**

A witness is stale when `witness.spec_hash != currentSpecHash(db, inv)`. The `currentSpecHash` function computes SHA-256 over the invariant's raw text from the latest parse. Any spec edit that changes an invariant's text (even whitespace) invalidates all existing witnesses for that invariant. Staleness detection runs in O(1) per invariant via the indexed `content_hash` column. The `ddis challenge` command checks staleness before running verification — a stale witness is automatically re-challenged with the updated spec hash.

**Violation remediation flowchart:**

For each violation type, the remediation path is deterministic: `MissingWitness` → run `ddis witness <inv> <db>` → re-run evidence chain. `StaleWitness` → re-run `ddis witness` with updated spec → `ddis challenge`. `MissingChallenge` → run `ddis challenge <inv> <db>`. `NonConfirmed` → investigate implementation, fix code, re-witness, re-challenge. The flowchart is acyclic: each remediation action produces exactly one of {success, new violation of different type}. Cycles are impossible because witness+challenge together cover all 4 violation types.

### Chapter 4: Agent-Executable Protocol

The protocol generator assembles a self-contained JSON document from all quality signals. The protocol is the externalization of the triage endofunctor — it makes the spec state, valid transitions, and convergence criteria visible to any consumer without requiring DDIS domain knowledge.

**GenerateProtocol algorithm:**

The generator runs all quality signals (validate, coverage, drift, challenge, contradict) and assembles their results into a structured JSON document. Each signal is computed independently and normalized to [0,1].

```
func GenerateProtocol(db *DB, events []Event) Protocol:
  fitness = ComputeFitness(db, events)
  measure = ComputeMeasure(db, events)
  issues = deriveAllIssueStates(events)
  ranked = rankDeficiencies(fitness, db)
  trajectory = loadFitnessHistory(events)
  return Protocol{
    Version: "1.0",
    SpecID: db.specID,
    Fitness: FitnessSection{Current: fitness.Score, Target: 1.0, Trajectory: trajectory, Lyapunov: 1.0 - fitness.Score},
    Measure: measure,
    Issues: issues,
    RankedWork: ranked,
    Convergence: ConvergenceSection{LyapunovDecreasing: isDecreasing(trajectory), MeasureDecreasing: true, EstimatedSteps: estimateSteps(fitness)},
  }
```

**Protocol JSON schema:**

The protocol document contains 6 top-level sections: `version` (semver string), `spec_id` (integer), `fitness` (current score, target, trajectory array, Lyapunov value), `measure` (the ℕ³ triple), `issues` (array of issue objects with state and valid transitions), and `ranked_work` (array of action objects sorted by ΔF).

Each action in `ranked_work` is a complete, executable CLI command string — no placeholder arguments, no missing required flags. The agent executes the string verbatim and re-runs `--protocol` to get the updated state. This execute-observe loop is the operational semantics of gradient descent on the Lyapunov surface.

**Convergence estimation:**

The `estimated_steps_to_fixpoint` field is a linear extrapolation from the fitness trajectory. Given the last k fitness values F₁, ..., Fₖ, the average improvement per step ΔF_avg = (Fₖ - F₁) / (k-1), and the remaining gap = 1.0 - Fₖ, the estimate is ceil(gap / ΔF_avg). This is intentionally simple — it provides a rough progress indicator, not a formal bound. The formal bound comes from the well-founded ordering on μ (APP-INV-068).

**Sample protocol output snippet:**

```json
{
  "version": "1.0",
  "fitness": {"current": 0.9862, "target": 1.0, "lyapunov": 0.0138},
  "measure": [0, 0, 1],
  "ranked_work": [
    {"action": "ddis validate manifest.ddis.db", "delta_f": 0.0118},
    {"action": "ddis drift manifest.ddis.db --report", "delta_f": 0.0020}
  ],
  "convergence": {"decreasing": true, "estimated_steps": 2}
}
```

**Convergence estimation edge cases:** When k < 2 (fewer than 2 trajectory points), `estimated_steps` is omitted — insufficient data for extrapolation. When ΔF_avg ≤ 0 (stagnation or regression), the estimate is set to `null` with a `"stalled"` flag, signaling that the agent should switch strategies rather than repeat the same actions. Oscillating trajectories (alternating increases and decreases) are detected by checking sign changes in consecutive ΔF values; if > 50% of steps oscillate, the `"oscillating"` flag is set.
