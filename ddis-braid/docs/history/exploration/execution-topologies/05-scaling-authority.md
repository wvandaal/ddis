# 05 — Scaling Authority: A(d) = R(1-C)T

> **Summary:** The authority to make scaling decisions autonomously is a function of three
> factors: reversibility R (how easy to undo), commitment weight C (how many downstream
> decisions are affected), and trust score T (earned over time from successful outcomes).
> This produces a three-tier delegation model where the system starts conservative and
> earns autonomy through demonstrated competence.

---

## 1. The Delegation Problem

For a given scaling decision, the system must choose:
- **(a) Act autonomously** — decide and execute without human involvement
- **(b) Recommend and act unless vetoed** — decide, announce, wait for veto window, then execute
- **(c) Defer to human** — recommend only, wait for explicit human approval

The correct choice depends on the decision's properties, not on a static policy.

---

## 2. The Authority Function

### 2.1 Definition

```
A(decision) = R(decision) * (1 - C(decision)) * T(system)
```

Where:
- **R(decision)** in [0, 1]: Reversibility — how easy is it to undo this scaling action?
- **C(decision)** in [0, 1]: Normalized commitment weight — how many downstream decisions are affected?
- **T(system)** in [0, 1]: Trust score — has the system's scaling been accurate historically?

### 2.2 Three-Tier Mapping

```
A(decision) > 0.5   -> Tier 1: Act autonomously
A(decision) in [0.2, 0.5] -> Tier 2: Recommend and act unless vetoed
A(decision) < 0.2   -> Tier 3: Recommend only, wait for human approval
```

The thresholds (0.5, 0.2) are datoms, tunable:

```
[authority:thresholds :authority/autonomous 0.5 tx:genesis assert]
[authority:thresholds :authority/recommend 0.2 tx:genesis assert]
```

---

## 3. R(decision): Reversibility

How easy is it to undo this scaling action if it turns out to be wrong?

| Decision | R | Rationale |
|----------|---|-----------|
| Add 1 agent to ready work | 0.95 | Trivially reversible (just remove if unneeded) |
| Remove idle agent | 0.80 | Reversible but loses agent's local context/frontier |
| Add N agents (N > 1) | 0.95/N | Each individually reversible, but N undo operations needed |
| Remove active agent | 0.30 | Uncommitted work at risk (must harvest first) |
| Restructure topology | 0.60 | Reversible via rollback but disruptive during transition |
| Scale down by half | 0.15 | Massive context loss, costly to reverse |

R is computed from the action type, not learned. It is a structural property of the action.

---

## 4. C(decision): Commitment Weight

How many downstream decisions are affected by this scaling action?

### 4.1 Five Impact Dimensions

```
C(decision) = w1 * causal_cone_size(d)
            + w2 * compute_cost(d)
            + w3 * token_commitment(d)
            + w4 * coordination_complexity(d)
            + w5 * throughput_impact(d)
```

| Dimension | What It Measures | Range |
|-----------|-----------------|-------|
| causal_cone_size | Number of future decisions affected | Fraction of active datoms |
| compute_cost | Dollar cost: $/hour * estimated duration | Normalized to [0, 1] |
| token_commitment | Estimated token consumption for new agents | Normalized to [0, 1] |
| coordination_complexity | O(n^2) for mesh, O(n) for star, O(1) for solo | Normalized to [0, 1] |
| throughput_impact | Estimated effect on tasks/hour (positive or negative) | Normalized to [0, 1] |

### 4.2 Default Dimension Weights

```
[commitment:weights :commitment/causal    0.35 tx:genesis assert]
[commitment:weights :commitment/compute   0.25 tx:genesis assert]
[commitment:weights :commitment/tokens    0.15 tx:genesis assert]
[commitment:weights :commitment/complexity 0.15 tx:genesis assert]
[commitment:weights :commitment/throughput 0.10 tx:genesis assert]
```

### 4.3 Concrete Commitment Weight Examples

| Decision | Causal | Compute | Tokens | Complexity | Throughput | C |
|----------|--------|---------|--------|------------|------------|---|
| Add 1 agent to independent task | 0.02 | 0.05 | 0.05 | 0.02 | 0.01 | 0.03 |
| Remove agent on critical path | 0.40 | 0.10 | 0.00 | 0.05 | 0.60 | 0.26 |
| Switch mesh to star (8 agents) | 0.25 | 0.02 | 0.00 | 0.50 | 0.20 | 0.19 |
| Scale down from 10 to 3 | 0.70 | 0.30 | 0.00 | 0.60 | 0.80 | 0.50 |

---

## 5. T(system): Trust Score

### 5.1 How Trust Is Earned

```
T(system) = sigmoid(sum(outcome_deltas) / n_decisions)

outcome_delta(d) = actual_quality(d) - predicted_quality(d)
sigmoid normalizes to [0, 1]
```

A positive outcome_delta means the system's scaling decision produced better-than-predicted
results. A negative delta means worse.

### 5.2 Initial Trust

```
T_initial = 0.3
```

Conservative: at T=0.3, most decisions require human approval (Tier 2 or 3).

### 5.3 Trust Evolution

Trust increases with successful autonomous decisions:

```
After 10 good decisions: T ~ 0.6 (moderate autonomy)
After 30 good decisions: T ~ 0.8 (high autonomy for reversible actions)
```

Trust decreases with bad outcomes:

```
Bad outcome: T -= 0.1 * harmful_weight
harmful_weight = 4.0 (mirroring cm feedback protocol: harmful marks count 4x)
```

This means one bad outcome undoes approximately four good outcomes, creating strong
incentive for conservative decisions.

### 5.4 Trust as Datom

```
[trust:scaling :trust/score 0.30 tx:genesis assert]
[trust:scaling :trust/total-decisions 0 tx:genesis assert]
[trust:scaling :trust/positive-outcomes 0 tx:genesis assert]
[trust:scaling :trust/negative-outcomes 0 tx:genesis assert]
```

Updated via harvest after each scaling decision's outcome is measured.

---

## 6. Worked Examples

### 6.1 With Initial Trust (T = 0.3)

| Decision | R | C | T | A = R(1-C)T | Tier |
|----------|---|---|---|-------------|------|
| Add 1 agent to ready work | 0.95 | 0.03 | 0.3 | 0.28 | Recommend |
| Remove idle agent | 0.80 | 0.05 | 0.3 | 0.23 | Recommend |
| Add 3 agents | 0.32 | 0.10 | 0.3 | 0.09 | Human |
| Remove active agent | 0.30 | 0.26 | 0.3 | 0.07 | Human |
| Restructure topology | 0.60 | 0.19 | 0.3 | 0.15 | Human |
| Scale down by half | 0.15 | 0.50 | 0.3 | 0.02 | Human |

Note: Even the simplest action (add 1 agent) is Tier 2 (recommend), not autonomous.
This is by design for the initial trust level.

### 6.2 With Mature Trust (T = 0.75)

| Decision | R | C | T | A = R(1-C)T | Tier |
|----------|---|---|---|-------------|------|
| Add 1 agent to ready work | 0.95 | 0.03 | 0.75 | 0.69 | Autonomous |
| Remove idle agent | 0.80 | 0.05 | 0.75 | 0.57 | Autonomous |
| Add 3 agents | 0.32 | 0.10 | 0.75 | 0.22 | Recommend |
| Remove active agent | 0.30 | 0.26 | 0.75 | 0.17 | Human |
| Restructure topology | 0.60 | 0.19 | 0.75 | 0.36 | Recommend |
| Scale down by half | 0.15 | 0.50 | 0.75 | 0.06 | Human |

Note: Even at maximum trust, removing an active agent and scaling down by half still
require human approval. This is by design: some decisions should never be fully
autonomous because their commitment weight is inherently high.

---

## 7. Tier 2 Protocol: Recommend-and-Act-Unless-Vetoed

### 7.1 Protocol

```
1. RECOMMEND: System asserts scaling recommendation with rationale
   [scaling:s1 :scaling/action :add-agent tx:t1 assert]
   [scaling:s1 :scaling/rationale "3 unblocked tasks, 2 agents at 95% util" tx:t1 assert]
   [scaling:s1 :scaling/authority-score 0.42 tx:t1 assert]
   [scaling:s1 :scaling/status :recommended tx:t1 assert]
   [scaling:s1 :scaling/veto-window-ms 60000 tx:t1 assert]

2. NOTIFY: Signal::ScalingRecommendation fired
   Human sees recommendation in guidance footer / CLAUDE.md
   Human can veto within window

3. TIMEOUT: If no veto within window:
   [scaling:s1 :scaling/status :enacted tx:t2 assert]
   System proceeds with scaling action

4. VETO: If human asserts veto:
   [scaling:s1 :scaling/status :vetoed tx:t3 assert]
   [scaling:s1 :scaling/veto-reason "..." tx:t3 assert]
   System records veto reason, does NOT update trust score T
   (human decision is Observed provenance, always respected, not a "failure")

5. LEARN: Outcome measurement after action (or veto):
   If enacted and good outcome -> T increases
   If enacted and bad outcome -> T decreases (4x harmful weight)
   If vetoed -> no T change (human decision, not system error)
```

### 7.2 Veto Window Duration

The veto window scales with commitment weight:

```
veto_window(decision) = base_window * (1 + C(decision))

Low commitment (C=0.1):  base * 1.1 = 33 seconds (if base = 30s)
High commitment (C=0.5): base * 1.5 = 45 seconds
```

Default base window:
```
[authority:defaults :authority/base-veto-window-ms 30000 tx:genesis assert]  ;; 30 seconds
```

---

## 8. The Authority Lattice

### 8.1 Conservative Join

Authority levels form a total order: Autonomous > Recommend > Human-only

When two authority computations disagree (different coupling models produce different
commitment weights), take the MINIMUM authority level (most conservative):

```
authority_resolution(a1, a2) = min(a1, a2)
```

This is a meet-semilattice with Human-only as the bottom element.

### 8.2 Safety Argument

The conservative direction is always safe:
- Tier 3 (Human) can never be worse than Tier 1 (Autonomous) — it adds a human check
- Tier 2 (Recommend) can never be worse than Tier 1 — it adds a veto window
- The cost of unnecessary caution is delay (human review time)
- The cost of unnecessary autonomy is potentially wrong scaling decision

Since wrong scaling decisions can be expensive (knowledge loss, coordination collapse),
the conservative default minimizes maximum regret (minimax).

---

## 9. Connecting to the Provenance Lattice

The authority function connects to the existing provenance lattice (ADR-STORE-008):

| Provenance Type | Scaling Authority |
|----------------|-------------------|
| Observed (human decision) | Always accepted, always Tier 3+ authority |
| Derived (computed from data) | Subject to A(d) authority function |
| Inferred (estimated from patterns) | Subject to A(d) with stricter thresholds |
| Hypothesized (speculated) | Never auto-accepted for scaling |

A human scaling decision (provenance = Observed) always overrides a system recommendation.
This is consistent with the provenance ordering and the three-tier conflict routing.

---

## 10. Traceability

| Concept | Traces to |
|---------|-----------|
| Authority function A(d) | SEED.md S6 (Reconciliation Taxonomy — procedural divergence) |
| Commitment weight C(d) | docs/design/ADRS.md AS-002 (continuous commitment weight model) |
| Provenance-weighted authority | spec/01-store.md ADR-STORE-008 (provenance lattice) |
| Three-tier routing | spec/04-resolution.md (three-tier conflict routing) |
| Trust as earned score | SEED.md S7 (self-improvement loop) |
| Signal::ScalingRecommendation | spec/09-signal.md (signal types) |
| Harmful marks count 4x | cm feedback protocol (established convention) |
| Veto window mechanism | spec/11-deliberation.md (deliberation stability guard) |

---

*Next: `06-cold-start.md` — how to begin coordinating agents with no prior history.*
