# Logical Form Extraction: §08-SYNC, §09-SIGNAL, §10-BILATERAL

> Source files: `spec/08-sync.md`, `spec/09-signal.md`, `spec/10-bilateral.md`
> Vocabulary: `exploration/coherence-convergence/PROPERTY_VOCABULARY.md`
> Output: `exploration/coherence-convergence/logical-forms-08-10.json`

---

## Summary

**Total elements extracted: 34**

| Type | Count | Elements |
|------|-------|---------|
| Invariant (INV) | 16 | SYNC: 5, SIGNAL: 6, BILATERAL: 5 |
| ADR | 9 | SYNC: 3, SIGNAL: 3, BILATERAL: 3 |
| Negative case (NEG) | 7 | SYNC: 2, SIGNAL: 3, BILATERAL: 2 |
| Uncertainty (UNC) | 2 | BILATERAL: 2 (from ADR body markers) |

**Unique properties used**: 29 (out of 109 in vocabulary)

**Stage distribution**:
- Stage 1: 10 elements (SIGNAL-002, NEG-SIGNAL-002, BILATERAL-001 through 005, ADR-BILATERAL-001/002, NEG-BILATERAL-001/002)
- Stage 2: 3 elements (SIGNAL-005, BILATERAL-003, ADR-BILATERAL-003)
- Stage 3: 19 elements (all SYNC, most SIGNAL)
- Stage unassigned: 2 (UNC-BILATERAL-001, UNC-BILATERAL-002)

---

## Classification Difficulties

**NEG-SIGNAL-001**: The spec states the safety property as "no signal emitted and not recorded as datom." The vocabulary has no `no_signal_loss` property. Mapped to the combination of `signal_as_datom` (safety) and `no_data_loss` as the prohibited-property violation. The `no_data_loss` property in the vocabulary is listed under SAFETY as a global property, not a SIGNAL property — this is acceptable since the spec intends signal persistence as an instance of general data-loss prevention.

**NEG-BILATERAL-001 and NEG-BILATERAL-002**: Both have empty `prohibited_properties` lists. This is correct: neither NEG defines a property that must NOT be present — they define liveness/safety requirements using `safety_properties`. The vocabulary `prohibited_properties` field captures "anti-properties" (imaginary bad properties), but for these NEGs the prohibition is on a _state_ (regression) or _action_ (skipping a check), not on a system property. There is no vocabulary term for `fitness_regression_allowed` or `coherence_check_skippable` to put in `prohibited_properties`.

**INV-SIGNAL-006 (Taxonomy Completeness)**: Confidence set to 0.85, not 1.0. Temporal and Procedural divergences are mapped to existing signal types through indirection (UncertaintySpike and GoalDrift respectively), not through dedicated signals. The mapping is asserted by the spec but the adequacy of the indirection is not proven within §09 — it depends on the Temporal divergence detection mechanism (frontier comparison) actually wiring up to UncertaintySpike emission, which is specified nowhere in §09.

**ADR-BILATERAL-003**: The `commitments` list is thin — the ADR commits to "periodic human sessions" producing datoms, which maps to `bilateral_lifecycle_stages` and `signal_as_datom`. No vocabulary property captures "human-in-the-loop for intent validation" precisely.

---

## Tensions and Contradictions

These are the most important findings. Ordered by severity.

---

### T-02 [HIGH]: Fitness Monotonicity Depends on an Unguarded Convention

**Elements**: `INV-BILATERAL-001`, `NEG-BILATERAL-001`, `ADR-BILATERAL-001`, `UNC-BILATERAL-001`

`INV-BILATERAL-001` asserts `F(S_{n+1}) >= F(S_n)` unconditionally for every bilateral cycle. The spec justifies this by claiming that residual documentation is a "non-decreasing fitness operation."

The fitness formula is:
```
F(S) = 0.18×V + 0.18×C + 0.18×(1-D) + 0.13×H + 0.13×(1-K) + 0.08×(1-I) + 0.12×(1-U)
```

The `D` (drift) component increases when the implementation regresses — code that was aligned with the spec gets changed. If an external agent modifies implementation between bilateral cycles, `D` increases and `F(S)` decreases. No protocol rule prevents this. The residual-documentation escape hatch only covers gaps already in the `divergence_map` — it cannot absorb external implementation regression.

**The invariant is stated as unconditional but can be violated without any protocol violation.** This is a gap between the algebraic law (Level 0) and the implementation reality.

**Resolution options**:
- (a) Qualify the invariant: "fitness is monotonic given no external implementation regression between cycles."
- (b) Snapshot the implementation state at cycle start and measure fitness against that snapshot only.
- (c) Redefine F(S) to measure only spec elements (V, C, K, U), dropping implementation-dependent terms (D, I) into a separate implementation health metric.

---

### T-03 [HIGH]: Automated CYCLE vs Human-Gated Intent Validation

**Elements**: `INV-BILATERAL-002`, `NEG-BILATERAL-002`, `ADR-BILATERAL-003`

`INV-BILATERAL-002` mandates that every CYCLE evaluates all five coherence conditions C1–C5. `NEG-BILATERAL-002` prohibits any CYCLE from skipping any of C1–C5.

`ADR-BILATERAL-003` states that C3 (spec ≈ intent) is checked via "periodic human sessions" — not on every automated cycle. The human's confirmation is a datom, but the frequency is periodic, not per-cycle.

**Direct contradiction**: NEG-BILATERAL-002 says no cycle skips C3; ADR-BILATERAL-003 says C3 is only checked periodically. These cannot both be true unless "cycle" is overloaded to mean two different things.

Possible interpretations:
1. CYCLE as defined in §10.2 is the automated cycle; ADR-BILATERAL-003 defines a separate "intent-cycle." NEG-BILATERAL-002 applies only to the automated cycle, which uses a cached C3 result. **This is the most viable interpretation but is not stated.**
2. CYCLE always waits for human confirmation — serializing all automated work on human availability. **This is operationally unacceptable.**
3. CYCLE skips C3 silently when no human confirmation is cached. **This violates NEG-BILATERAL-002 as stated.**

**Resolution**: Split the CYCLE definition into two types: `automated-cycle` (C1, C2, C4, C5) and `intent-cycle` (adds C3). NEG-BILATERAL-002 should apply to the union of both types, not to automated-cycle alone. ADR-BILATERAL-003 should reference `intent-cycle` explicitly.

---

### T-01 [MEDIUM]: CALM Theorem Scope Ambiguity Between SYNC and QUERY

**Elements**: `ADR-SYNC-001`, `INV-SYNC-001`, `INV-SYNC-005`

`ADR-SYNC-001` commits to `calm_compliant` (monotonic queries have coordination-free implementations) and to `local_first`. `INV-SYNC-005` commits to `post_barrier_deterministic` — Barriered queries require a resolved barrier.

Vocabulary incompatibility I8 states: `calm_compliant` is incompatible with `requires_barrier_for_reads`. The spec resolves this by limiting barriers to non-monotonic queries only. But **the boundary between monotonic and non-monotonic queries is specified in the QUERY namespace (§03), not in §08-SYNC.** §08 defers to strata classification.

**Tension**: A caller issuing a query at Stage 3 (multi-agent) may not know the query's stratum classification without invoking the query engine. If the caller issues a non-monotonic query without requesting a barrier (because they mistakenly believe it is monotonic), INV-SYNC-005 is violated — but the violation is undetectable until query evaluation time. The query engine must enforce the barrier requirement, not rely on the caller.

**Resolution**: The query engine layer must reject or warn on non-monotonic queries (strata 2–5) issued without a Barriered mode, rather than silently producing approximate results.

---

### T-04 [MEDIUM]: Confusion Dispatch Timing vs Subscription Debounce

**Elements**: `INV-SIGNAL-002`, `INV-SIGNAL-003`, `ADR-SIGNAL-003`

`INV-SIGNAL-002` requires re-association to occur within the **same agent cycle** as Confusion signal emission.

`ADR-SIGNAL-003` allows subscriptions to have an optional debounce window — signals are batched and the callback fires once after the window expires.

If Confusion signal dispatch is implemented through the subscription mechanism (a natural design — the agent cycle subscribes to Confusion signals), and that subscription has a debounce window, the re-association fires after the window, potentially in the **next** agent cycle.

**Tension**: The two mechanisms are compatible only if Confusion signals are explicitly excluded from debounced subscriptions and dispatched through a synchronous, non-subscription path.

**Resolution**: Confusion signals must be dispatched synchronously by the agent cycle handler before entering the subscription dispatch pipeline. The spec should state this explicitly as a special-case dispatch rule: `dispatch(Confusion) = Synchronous(ReAssociate)` rather than routing through the subscription layer.

---

### T-06 [MEDIUM]: Temporal Divergence — Two Mechanisms, Undefined Escalation

**Elements**: `INV-SIGNAL-006`, `INV-SYNC-001`, `ADR-SIGNAL-001`

`INV-SIGNAL-006` states that temporal divergence is surfaced as `UncertaintySpike`. The dispatch table routes `UncertaintySpike → Guidance`.

The SYNC namespace (§8) provides sync barriers as the explicit mechanism for resolving temporal divergence between agents (frontier misalignment).

**Tension**: UncertaintySpike routes to Guidance (a low-cost, informational response), but resolving actual temporal divergence requires a sync barrier (an expensive, blocking coordination). The escalation path from UncertaintySpike → SyncBarrier is unspecified. Nothing in §09 says that Guidance may suggest initiating a barrier, nor does §08 specify what triggers barrier initiation beyond explicit `braid sync --with` CLI invocation.

This means temporal divergence generates a signal, the signal routes to Guidance, and Guidance may or may not suggest running a barrier — but the system never automatically initiates one. Temporal divergence can persist indefinitely without triggering barrier resolution.

**Resolution**: Define an escalation predicate: if `UncertaintySpike.delta > threshold` AND signal type is `Temporal`, Guidance should emit a `DelegationRequest` or `BranchReady` signal that routes to barrier initiation. Alternatively, define a new `TemporalDivergence` signal type whose dispatch target is `SyncBarrier`, keeping UncertaintySpike for consequential divergence only.

---

### T-05 [LOW]: Cache Invalidation Scope After Conflict Signal

**Elements**: `INV-SYNC-005`, `INV-SIGNAL-004`, `ADR-SIGNAL-002`

`INV-SYNC-005` states Barriered queries are evaluated against `barrier.cut`, a fixed consistent cut.

`ADR-SIGNAL-002` states that step 6 of the conflict routing cascade is "Invalidate cached query results touching affected entities."

If the cache is keyed only on `(query_expr, frontier)` and not on `(query_expr, barrier_id)`, invalidating a cache entry for an entity that appears in a Barriered query would cause re-evaluation — but the re-evaluation would still use the same `barrier.cut`, so the result should be identical. This is a performance concern, not a correctness one.

However, if the cache key includes the local frontier, and a Barriered query's cache entry is keyed on `(query_expr, barrier.cut_as_frontier)`, then post-barrier conflict invalidation for an entity in `barrier.cut` would trigger a re-query against the same cut — wasteful but correct.

The low-severity rating stands: this is an efficiency concern. The spec should clarify that cache invalidation from conflict signals only applies to local-frontier queries, not to Barriered queries whose cut is fixed.

---

## Property Vocabulary Gaps Noticed

These situations required vocabulary stretching or omission:

1. **No `synchronous_dispatch` property** — needed to capture INV-SIGNAL-002's within-cycle timing guarantee. Used `signal_triggers_resolution` as closest approximation, which is weaker.

2. **No `human_confirmation_required` property** — ADR-BILATERAL-003's intent validation mechanism is not capturable with the current vocabulary. The `bilateral_lifecycle_stages` property is a poor substitute.

3. **No `cache_invalidation_scoped` property** — T-05 resolution requires distinguishing local-frontier caches from barrier-cut caches. The vocabulary has no property for this distinction.

4. **`prohibited_properties` field underutilized for state-based NEGs** — NEG-BILATERAL-001 and NEG-BILATERAL-002 prohibit states (fitness regression, skipping a check), not system properties. The extraction schema works better for property-presence prohibitions than for operational prohibitions.
