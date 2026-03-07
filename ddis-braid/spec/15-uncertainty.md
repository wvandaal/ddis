> **Section**: Uncertainty Register | **Wave**: 4 (Integration)
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §15. Uncertainty Register

> **Purpose**: All claims in this specification with confidence < 1.0, organized by
> resolution urgency. Each entry identifies what is uncertain, why it matters, what
> would resolve it, and what breaks if the assumption is wrong.
>
> **Methodology**: Uncertainty markers follow UA-006 — explicit confidence levels with
> resolution criteria. Claims without markers are considered confidence 1.0 (settled).

### §15.1 Explicit Uncertainty Markers

These are claims explicitly flagged during specification production.

#### UNC-BILATERAL-001: Fitness Function Component Weights

**Source**: ADR-BILATERAL-001 (§10.5)
**Confidence**: 0.6
**Stage affected**: 1+

**Claim**: F(S) = 0.18×V + 0.18×C + 0.18×(1-D) + 0.13×H + 0.13×(1-K) + 0.08×(1-I) + 0.12×(1-U)

**Why uncertain**: Weights are derived from theoretical analysis of component importance
(primary triad V/C/D = 0.18, secondary pair H/K = 0.13, etc.) but have not been
calibrated against empirical data from actual Braid usage.

**Impact if wrong**: Fitness score gives misleading convergence signal. A high F(S) could
mask real divergence if a low-weight component is actually critical, or vice versa.

**Resolution**: Run Stage 0 for ≥10 sessions. Compute F(S) after each. Compare weights
that correlate with successful outcomes (sessions where harvested knowledge was correct
and complete) against weights that correlate with failures. Adjust weights to maximize
predictive power.

**What breaks**: INV-BILATERAL-001 (monotonic convergence) still holds regardless of weights.
The question is whether convergence is toward actual coherence or toward a local optimum.

---

#### UNC-BILATERAL-002: Divergence Boundary Weights

**Source**: ADR-BILATERAL-002 (§10.5)
**Confidence**: 0.5
**Stage affected**: 1+

**Claim**: D(spec, impl) = Σᵢ wᵢ × |boundary_gap(i)| with default equal weights
across the four boundaries (Intent→Spec, Spec→Spec, Spec→Impl, Impl→Behavior).

**Why uncertain**: Different projects may have very different boundary-gap distributions.
A project with a stable spec but turbulent implementation needs different weights than
one where intent is shifting.

**Impact if wrong**: Divergence metric under-weights the critical boundary, causing the
bilateral loop to focus remediation effort on the wrong gaps.

**Resolution**: After Stage 0, analyze which boundaries produce the most actionable
gaps. Weight boundaries proportional to their remediation cost × occurrence frequency.
Consider per-project weight profiles.

**What breaks**: The bilateral loop still detects all gaps (completeness is structural,
not weight-dependent). But prioritization of remediation effort may be misguided.

---

#### UNC-LAYOUT-001: Filesystem Performance at Scale

**Source**: INV-LAYOUT-008 (sharded directory), `spec/01b-storage-layout.md`
**Confidence**: 0.85
**Stage affected**: 0+

**Claim**: 256-way hash-prefix sharding provides adequate filesystem performance up to
100,000 transaction files (~390 files per directory).

**Why uncertain**: Filesystem performance depends on the specific filesystem (ext4, XFS,
ZFS, btrfs), inode allocation strategy, and directory hashing implementation.

**Impact if wrong**: Startup time or write_tx latency becomes unacceptable. May require
deeper sharding (4-char prefix, 65,536 dirs).

**Resolution**: Benchmark with 100K synthetic transaction files on ext4 and XFS.

**What breaks**: Layout correctness is unaffected. Only performance is at risk.

---

#### UNC-LAYOUT-002: EDN Parser Throughput for Bulk Startup

**Source**: INV-LAYOUT-003 (directory-store isomorphism), `spec/01b-storage-layout.md`
**Confidence**: 0.90
**Stage affected**: 0+

**Claim**: An EDN parser in Rust can process transaction files fast enough for interactive
startup (target: 10,000 files in < 1 second).

**Why uncertain**: EDN parsing throughput depends on implementation quality and transaction
file size. Mitigated by index caching (.cache/).

**Impact if wrong**: Cold start is too slow for interactive use.

**Resolution**: Implement EDN parser, benchmark with realistic transaction sizes.

**What breaks**: Warm starts (cached indexes) are unaffected. Only cold-start performance.

---

#### UNC-LAYOUT-003: Git Packfile Efficiency with Small Files

**Source**: ADR-LAYOUT-004 (hash-prefix sharding), `spec/01b-storage-layout.md`
**Confidence**: 0.80
**Stage affected**: 3+

**Claim**: Git packfile compression handles 100K+ small EDN files efficiently.

**Why uncertain**: Per-object overhead (header, SHA-1) may dominate for very small files.

**Impact if wrong**: Repository clone size grows linearly. Network transfer slower.

**Resolution**: Create test repository with 100K synthetic files, measure pack size.

**What breaks**: Store/merge correctness unaffected. Only git transport performance.

---

### §15.2 Implicit Uncertainties

These are areas where the specification makes commitments that depend on assumptions
not yet validated by implementation experience.

#### UNC-STORE-001: Content-Addressable EntityId Collision Rate

**Source**: INV-STORE-002, ADR-STORE-002
**Confidence**: 0.95
**Stage affected**: 0

**Claim**: BLAKE3 hash of content produces unique EntityIds with negligible collision
probability.

**Why uncertain**: BLAKE3 collision probability is astronomically low (2^{-128} for
random inputs) but content-addressed systems at scale can hit birthday-bound issues
with certain workload patterns.

**Impact if wrong**: Two different entities map to the same EntityId. Silent data
corruption — one entity's attributes overwrite another's.

**Resolution**: Monitor EntityId generation during implementation. Verify uniqueness
across ≥10^6 datoms. Consider a secondary check (entity content comparison on hash
match) as defense in depth.

**What breaks**: INV-STORE-002 (content identity), INV-STORE-003 (merge deduplication).

---

#### UNC-STORE-002: HLC Clock Skew Tolerance

**Source**: INV-STORE-008, ADR-STORE-004
**Confidence**: 0.9
**Stage affected**: 0

**Claim**: Hybrid Logical Clocks maintain causal ordering across agents with bounded
clock skew.

**Why uncertain**: HLC assumes clock skew is bounded. On a single VPS with NTP, skew
is typically <1ms. But container environments, VM migration, or suspended processes
can introduce larger skew.

**Impact if wrong**: Transaction ordering violations. Causally-later transactions appear
before causally-earlier ones, breaking frontier monotonicity (INV-STORE-009).

**Resolution**: Implement HLC with configurable max-skew parameter. Alert when observed
skew exceeds threshold. For Stage 0 (single VPS), this is very low risk.

**What breaks**: INV-STORE-008 (HLC monotonicity), INV-STORE-009 (frontier durability).

---

#### UNC-QUERY-001: Datalog Evaluation Performance at Scale

**Source**: INV-QUERY-002, ADR-QUERY-001
**Confidence**: 0.8
**Stage affected**: 1+

**Claim**: Semi-naive Datalog evaluation is efficient for Braid's query patterns
at expected scale (thousands of datoms, dozens of query patterns).

**Why uncertain**: Semi-naive evaluation is well-studied for databases but Braid's
query patterns include recursive graph traversal (causal-ancestor, depends-on),
aggregation (uncertainty tensor), and derived functions (spectral authority). These
may have pathological performance characteristics on certain store topologies.

**Impact if wrong**: Query latency exceeds acceptable limits, making the CLI unusable
for interactive agent workflows. Budget-aware output degrades to π₃ not from attention
pressure but from query timeout.

**Resolution**: Benchmark query patterns against synthetic stores of 10^3, 10^4, 10^5
datoms. Identify performance cliffs. Optimize hot paths (EAVT/AEVT index lookups).
Consider incremental materialization for Stratum 4–5 queries.

**What breaks**: INV-QUERY-002 (fixpoint termination — technically guaranteed by
Datalog semantics, but timeout is a practical termination condition).

---

#### UNC-HARVEST-001: Proactive Warning Thresholds

**Source**: INV-HARVEST-005, INV-INTERFACE-007
**Confidence**: 0.7
**Stage affected**: 0

**Claim**: Q(t) < 0.15 (~75% consumed) triggers harvest warning; Q(t) < 0.05 (~85%)
triggers harvest-only mode.

**Why uncertain**: Thresholds are calibrated to Claude Code context windows (~200K tokens)
with observed attention degradation patterns. Different LLM providers, model sizes, or
future context window changes may shift the optimal thresholds.

**Impact if wrong**: Warnings too early → annoyance, wasted budget on premature harvest.
Warnings too late → unharvested knowledge loss (FM-001).

**Resolution**: Track harvest outcomes vs. Q(t) at harvest time across 50+ sessions.
Compute the Q(t) threshold below which harvest quality degrades measurably. Adjust
thresholds to match.

**What breaks**: INV-HARVEST-005 (warning correctness), INV-INTERFACE-007 (proactive warning).

---

#### UNC-GUIDANCE-001: Basin Competition Crossover Point

**Source**: ADR-GUIDANCE-002, INV-GUIDANCE-004
**Confidence**: 0.7
**Stage affected**: 0

**Claim**: Without intervention, agents drift to Basin B (pretrained patterns) within
15–20 turns. The six anti-drift mechanisms maintain Basin A dominance.

**Why uncertain**: The "15–20 turns" figure is based on observed behavior with specific
LLM models (Claude). Different models may have different crossover points. The
effectiveness of the six mechanisms is theoretical — no empirical measurement yet.

**Impact if wrong**: If crossover is earlier (10 turns), the mechanisms may be insufficient.
If later (30+ turns), the mechanisms may be unnecessarily aggressive (wasting budget).

**Resolution**: Instrument drift detection during Stage 0. Measure turn count at which
agents first skip a DDIS step (transact gap, guidance miss). Plot Basin A probability
over turns. Calibrate mechanism intensity to the measured crossover.

**What breaks**: INV-GUIDANCE-004 (drift detection responsiveness — the threshold of 5
bash commands may be too lenient or too strict).

---

#### UNC-DELIBERATION-001: Crystallization Stability Threshold

**Source**: INV-DELIBERATION-002, ADR-DELIBERATION-004
**Confidence**: 0.7
**Stage affected**: 2

**Claim**: Default stability_min = 0.7 provides the right balance between premature
crystallization and unnecessary delay.

**Why uncertain**: The threshold interacts with commitment weight, confidence, coherence,
and conflict state. The optimal threshold may vary by entity type (architectural
decisions need higher stability than implementation details).

**Impact if wrong**: Too high → deliberation takes too long, blocking downstream work.
Too low → premature decisions create cascading incompleteness (FM-004).

**Resolution**: Run deliberation simulations with varying thresholds during Stage 2.
Measure: time-to-decision, downstream error rate from premature decisions, developer
frustration from blocked work. Find the Pareto frontier.

**What breaks**: INV-DELIBERATION-002 (stability guard enforcement — the invariant holds
regardless, but the quality of decisions may suffer).

---

#### UNC-SCHEMA-001: Seventeen Axiomatic Attributes Sufficiency

**Source**: INV-SCHEMA-001, ADR-SCHEMA-001
**Confidence**: 0.85
**Stage affected**: 0

**Claim**: Exactly 17 axiomatic meta-schema attributes are sufficient to bootstrap
the full schema system.

**Why uncertain**: The 17 were identified through design analysis (Transcript 02:379–420)
but have not been tested against a real implementation. Missing an attribute at Layer 0
requires a breaking change to the genesis transaction.

**Impact if wrong**: Schema system cannot express a required concept. Workaround:
add attributes at Layer 1+ (non-breaking) or revise genesis (breaking — all stores
become incompatible).

**Resolution**: Implement genesis transaction during Stage 0. Attempt to define all
Layer 1–5 schema using only the 17 axiomatic attributes. Any failure reveals a gap.

**What breaks**: INV-SCHEMA-001 (genesis completeness), INV-SCHEMA-008 (self-description).

---

#### UNC-RESOLUTION-001: Per-Attribute Resolution Mode Ergonomics

**Source**: ADR-RESOLUTION-001, ADR-RESOLUTION-002
**Confidence**: 0.8
**Stage affected**: 0

**Claim**: Schema authors can and will correctly declare resolution modes (LWW, Lattice,
Multi) for each attribute.

**Why uncertain**: This places a cognitive burden on schema designers. Incorrect mode
selection (e.g., LWW on an attribute that should be Lattice) silently loses data.
There's no mechanism to detect "probably wrong" mode selections.

**Impact if wrong**: Silent data loss or incorrect conflict resolution. The system
behaves correctly per its configuration but the configuration doesn't match intent.

**Resolution**: Provide sensible defaults (LWW for scalar attributes, Multi for set-valued,
Lattice for lifecycle/status). Emit warnings when resolution mode selection looks unusual
(e.g., LWW on a set-valued attribute). Consider a `ddis schema audit` command.

**What breaks**: INV-RESOLUTION-001 (algebraic law holds by construction, but semantic
correctness depends on correct mode selection).

---

### §15.2.1 ADRs

### ADR-UNCERTAINTY-001: Three-Axis Uncertainty Tensor

**Traces to**: SEED §6, ADRS UA-001
**Stage**: 1

#### Problem
How should the system represent uncertainty about entities? A scalar uncertainty value
(e.g., "confidence = 0.7") collapses fundamentally different kinds of uncertainty into
a single number, losing the information needed to decide what action reduces uncertainty.
An entity might be uncertain because we lack observations (epistemic), because its behavior
is inherently variable (aleatory), or because its downstream dependencies amplify small
errors (consequential). These require different responses: observe, model, or isolate.

#### Options
A) **Scalar uncertainty** — a single confidence value per entity. Simple but conflates
   epistemic, aleatory, and consequential uncertainty. An agent cannot distinguish "we
   don't know" from "it's inherently random" from "errors here cascade badly."
B) **Two-axis model** — separate epistemic and aleatory uncertainty (the classical
   distinction). Misses the consequential dimension: an entity can have low epistemic and
   aleatory uncertainty but high consequential risk if its dependents amplify small errors.
C) **Three-axis tensor** — `σ = (σ_e, σ_a, σ_c)` representing epistemic (reducible by
   observation), aleatory (inherent randomness, Shannon entropy), and consequential
   (downstream risk via DAG traversal). Each axis has distinct computation methods and
   distinct reduction strategies.

#### Decision
**Option C.** Uncertainty is represented as a three-axis tensor:

```
σ = (σ_e, σ_a, σ_c)

σ_e — epistemic uncertainty: reducible by observation or validation
      Computation: based on age since last validation, provenance quality,
      and observation coverage of the entity's attributes.

σ_a — aleatory uncertainty: inherent variability, measured as Shannon entropy
      Computation: entropy of the value distribution for the entity's attributes
      across transactions. High σ_a indicates an attribute that takes many
      different values (inherently variable).

σ_c — consequential uncertainty: downstream risk amplification
      Computation: propagated uncertainty through the entity's forward
      dependency DAG. σ_c(e) = f(Σ uncertainty of dependents of e).
      Self-referential exclusion required (ADR-UNCERTAINTY-004).
```

Scalar combination for contexts requiring a single number:

```
scalar = √(α·σ_e² + β·σ_a² + γ·σ_c²)

Default weights: α = 0.4, β = 0.4, γ = 0.2
```

Weights are stored as datoms, configurable per deployment.

#### Formal Justification
The three axes are algebraically independent — each measures a different source of
uncertainty:
- σ_e and σ_a are weighted equally (α = β = 0.4) because both represent actionable
  uncertainty dimensions. σ_e can be reduced by observation; σ_a informs modeling decisions.
- σ_c is weighted lower (γ = 0.2) because it depends on graph topology, which changes
  slowly. Overweighting σ_c causes excessive caution about well-understood entities that
  happen to have many dependents.

The quadratic combination (Euclidean norm with weights) ensures that high uncertainty
on any single axis produces a high scalar, while moderate uncertainty across all three
axes produces a moderate scalar. This matches intuition: a single severe uncertainty
source is more alarming than distributed mild uncertainty.

The tensor representation is essential for the delegation threshold (ADR-RESOLUTION-006),
which uses σ_c directly, and for the temporal decay model (ADR-UNCERTAINTY-002), which
applies only to σ_e. These computations are not possible with a scalar model.

#### Consequences
- Agents can take targeted action: high σ_e -> observe; high σ_a -> model or accept;
  high σ_c -> isolate dependents or stabilize upstream
- The delegation threshold formula (ADR-RESOLUTION-006) uses σ_c directly for risk
  assessment
- The scalar combination provides backward compatibility with systems expecting a
  single uncertainty value
- Weight calibration is empirical: Stage 0 uses defaults; later stages adjust based
  on which axis best predicts resolution outcomes

#### Falsification
The three-axis model is wrong if: (1) the three axes are not independent in practice
(e.g., σ_e and σ_a are always highly correlated, making the distinction unnecessary),
or (2) a significant source of uncertainty does not fit any of the three axes (e.g.,
social uncertainty — "will the stakeholder change their mind?" — requires a fourth axis),
or (3) the scalar combination formula consistently misjudges overall uncertainty (e.g.,
the quadratic norm over-smooths situations where one axis dominates).

---

### ADR-UNCERTAINTY-002: Epistemic Uncertainty Temporal Decay

**Traces to**: SEED §6, ADRS UA-002
**Stage**: 1

#### Problem
Epistemic uncertainty (σ_e) is reducible by observation — but observations become stale
over time. How should the system model the increasing uncertainty that comes from not
having recently validated an entity? The decay rate should differ by entity type: code
observations change frequently, architectural decisions rarely change, and invariants
(normative claims) do not decay at all.

#### Options
A) **No temporal decay** — σ_e remains constant until explicitly re-measured. Simple but
   ignores the reality that knowledge goes stale. An observation from a week ago about
   a rapidly-changing file has much higher effective uncertainty than the same observation
   made minutes ago.
B) **Uniform decay** — σ_e increases at the same rate for all entities. Uniform but
   incorrect: an architectural decision validated last month is still trustworthy, while
   a filesystem observation from last month is likely stale.
C) **Per-namespace exponential decay** — σ_e increases exponentially with time since last
   validation, with a per-namespace decay rate λ that reflects how quickly different types
   of knowledge go stale.

#### Decision
**Option C.** Epistemic uncertainty increases over time following an exponential form:

```
age_factor(e) = 1 - e^{-λ × time_since_last_validation(e)}

σ_e(e, t) = σ_e_base(e) + age_factor(e) × (1 - σ_e_base(e))
```

where `age_factor` ranges from 0 (just validated) to 1 (very stale), and `σ_e_base`
is the uncertainty at the time of last validation.

Per-namespace λ values (default, configurable as datoms):
- **Code observations** (`:filesystem`, `:shell`): λ = 0.1/hour — fast decay, code changes
  frequently during active development
- **Architectural decisions**: λ = 0.001/hour — slow decay, architecture is relatively
  stable
- **Invariants** (normative claims): λ = 0 — no decay. Invariants are prescriptive,
  not descriptive; their truth value does not change with time. An invariant is either
  satisfied or violated, never "stale."
- **Git observations**: λ = 0.01/hour — moderate decay, git history is append-only but
  the working tree changes

#### Formal Justification
The exponential form `1 - e^{-λt}` is the standard model for monotonically-increasing
probability of change. It has the properties:
- Starts at 0 when freshly validated (t = 0)
- Approaches 1 asymptotically (never exceeds maximum uncertainty)
- Rate of increase is highest immediately after validation (when the information is
  most trusted, each passing moment adds the most relative uncertainty)
- Per-namespace λ captures the empirical observation that different knowledge types
  have different half-lives

The invariant exception (λ = 0) is critical: invariants are normative ("the system SHALL
behave this way"), not descriptive ("the system currently behaves this way"). A violated
invariant is a defect to fix, not a stale observation to re-validate.

#### Consequences
- Seed assembly (§6) naturally surfaces recently-validated knowledge over stale knowledge
  (lower σ_e -> higher relevance score)
- Agents are incentivized to re-validate long-untouched entities (high σ_e creates work
  items via the guidance system)
- Invariants are stable anchors — they never become "uncertain" through neglect
- The λ values are stored as datoms (C3), enabling empirical calibration from validation
  frequency data

#### Falsification
The decay model is wrong if: (1) the exponential form does not match empirical staleness
patterns (e.g., knowledge is actually stable for a long period then suddenly becomes stale,
which would require a step function rather than exponential), or (2) the per-namespace λ
defaults are systematically miscalibrated (e.g., code observations at λ = 0.1/hour are
flagged as stale too aggressively for projects with slow development cadence), or (3) the
invariant exception (λ = 0) is wrong because invariants DO effectively become less
trustworthy when the system they govern has changed significantly since they were last
verified.

---

### ADR-UNCERTAINTY-003: Uncertainty Markers as First-Class Elements

**Traces to**: SEED §3, ADRS UA-006
**Stage**: 0

#### Problem
Specifications inevitably contain claims whose truth is uncertain — design choices that
may need revision, performance assumptions that have not been benchmarked, threshold
values that have not been calibrated. How should the specification handle these uncertain
claims? Traditional specifications either present everything with equal confidence (hiding
uncertainty) or use informal hedging language ("this might need adjustment") that agents
cannot act on.

#### Options
A) **Omit uncertain claims** — only include claims with high confidence. Produces a
   smaller, more trustworthy spec but hides important information. Agents cannot plan
   around known uncertainties if the uncertainties are not documented.
B) **Informal hedging** — use natural language to indicate uncertainty ("this value may
   need calibration"). Agents cannot programmatically distinguish uncertain from certain
   claims. The hedging language is invisible to automated verification.
C) **Structured uncertainty markers** — uncertain claims carry explicit metadata:
   confidence level (0.0–1.0), what is uncertain, why it matters, what would resolve it,
   and what breaks if the assumption is wrong. Markers are first-class specification
   elements, queryable and verifiable.

#### Decision
**Option C.** Specification uncertainty is marked explicitly using structured markers
with the following metadata:

```
Uncertainty marker schema:
  :uncertainty/id          — UNC-{NAMESPACE}-{NNN}
  :uncertainty/source      — reference to the spec element (INV, ADR) making the claim
  :uncertainty/confidence  — float 0.0–1.0
  :uncertainty/stage       — which stage is affected
  :uncertainty/claim       — the uncertain assertion
  :uncertainty/why         — why this claim is uncertain
  :uncertainty/impact      — what breaks if the assumption is wrong
  :uncertainty/resolution  — what evidence or action would resolve the uncertainty
  :uncertainty/breaks      — which invariants/ADRs are affected
```

Claims without explicit uncertainty markers are considered confidence 1.0 (settled).

The Uncertainty Register (§15) collects all markers, organized by resolution urgency.

#### Formal Justification
Structured uncertainty markers serve three functions:

1. **Agent safety**: An agent implementing a claim marked with confidence 0.6 can
   implement it provisionally and add validation tests, rather than treating it as gospel.
   Without markers, the agent has no signal about which claims to trust fully vs. hedge.

2. **Resolution tracking**: Each marker specifies what would resolve the uncertainty.
   This transforms uncertainty from a static annotation into an actionable work item.
   The guidance system (§12) can direct agents toward high-impact resolution activities.

3. **Risk assessment**: The `:uncertainty/breaks` field connects each uncertain claim to
   the invariants and ADRs that depend on it. This enables automated impact analysis:
   "if this assumption is wrong, which invariants are at risk?"

The confidence scale (0.0–1.0) is deliberately coarse-grained in practice. The meaningful
distinctions are: < 0.5 (more likely wrong than right — requires active resolution),
0.5–0.8 (plausible but unvalidated — monitor), > 0.8 (high confidence but not proven —
validate during implementation). Finer distinctions are false precision.

#### Consequences
- Every uncertain claim in the specification is queryable and trackable
- Agents can distinguish settled decisions from open questions
- The Uncertainty Register (§15) provides a prioritized list of uncertainties to resolve
- The fitness function (F(S)) includes uncertainty as a component: high aggregate
  uncertainty reduces the convergence score
- Uncertainty markers are datoms in the store (C3), enabling the same query and
  conflict resolution mechanisms as all other spec elements

#### Falsification
Uncertainty markers are wrong if: (1) agents ignore them in practice (the markers have
no behavioral effect — agents treat uncertain claims the same as certain ones), or
(2) the confidence values are systematically miscalibrated (claims marked 0.8 are wrong
as often as claims marked 0.5), or (3) the structured format is too burdensome (spec
authors skip marking uncertainties because the metadata requirements are too heavy).

---

### ADR-UNCERTAINTY-004: Self-Referential Measurement Exclusion

**Traces to**: SEED §6, ADRS UA-008
**Stage**: 1

#### Problem
Consequential uncertainty (σ_c) for entity e is computed by propagating uncertainty
through e's forward dependency DAG. But if entity e has uncertainty measurements that
target e itself (e.g., an uncertainty marker about e's own confidence level), including
these self-referential measurements in the σ_c computation creates a feedback loop:
e's uncertainty depends on its dependents' uncertainty, which depends on e's uncertainty.
This loop can cause the computation to diverge (oscillate or grow without bound).

#### Options
A) **Ignore the problem** — compute σ_c naively including self-references. In practice
   the feedback loop may converge to a fixed point (contractive mapping). But this is
   not guaranteed: the initial analysis (Transcript 02:819-858) showed that the claim
   "measurement is always contractive" is false in general. Self-correction to conditional
   contractivity required explicit exclusion.
B) **Cap the computation** — allow self-references but cap σ_c at a maximum value.
   Prevents divergence but produces incorrect values: the cap masks the true uncertainty
   structure and can cause σ_c to saturate at the cap value for entities with mild
   self-reference.
C) **Exclude self-referential measurements** — when computing σ_c(e), exclude from the
   dependent set any uncertainty measurements that target e itself. The computation
   becomes acyclic by construction.

#### Decision
**Option C.** The consequential uncertainty computation explicitly excludes self-referential
measurements:

```
σ_c(e) = f(dependents(e) \ {measurements targeting e})

where:
  dependents(e) = {e' | e' depends on e}
  measurements targeting e = {m | m is an uncertainty measurement AND m.target = e}
```

The exclusion set includes:
- Uncertainty markers (UNC-* entries) whose `:uncertainty/source` references e
- Derived uncertainty computations that include e in their input set
- Any entity whose sole relationship to e is measuring e's uncertainty

#### Formal Justification
The exclusion makes the σ_c computation well-defined by breaking all self-referential
cycles. Without exclusion, the computation graph can contain cycles:

```
e → dependent(e) → uncertainty_of(e) → e    (cycle!)
```

With exclusion, the `uncertainty_of(e)` node is removed from e's dependent set, breaking
the cycle. The resulting DAG is acyclic by construction, and σ_c can be computed in a
single bottom-up pass (topological sort of the dependency graph, excluding self-references).

This revision was self-corrected during the original design (Transcript 02:819-858):
the initial unconditional claim "measurement is always contractive" was analyzed and found
to be false in general. The correction to conditional contractivity with explicit
exclusion is more conservative and provably correct.

The exclusion does not lose information: the uncertainty measurements targeting e are
still visible via direct query. They simply do not participate in the recursive σ_c
computation for e. An agent can still ask "what uncertainty measurements exist for
entity e?" — the exclusion only affects the propagated consequential uncertainty.

#### Consequences
- σ_c computation is guaranteed to terminate (no infinite loops or oscillation)
- The computation can be performed in O(|V| + |E|) time via topological sort of the
  dependency DAG (excluding self-referential edges)
- Self-referential uncertainty measurements are preserved in the store — only excluded
  from the recursive computation
- The exclusion is conservative: it may slightly underestimate σ_c for entities with
  genuine self-referential risk, but never overestimates or diverges

#### Falsification
The exclusion is wrong if: (1) there exist entities where the self-referential uncertainty
is a significant component of their true consequential risk (the exclusion materially
underestimates σ_c), or (2) the exclusion criterion is too broad (it removes legitimate
dependent relationships that happen to involve measurement entities, reducing σ_c accuracy
for non-self-referential cases), or (3) a contractive fixed-point computation (Option A)
converges reliably in practice and produces more accurate results than the exclusion
approach for the entity sizes and graph topologies encountered in real Braid usage.

---

### §15.3 Summary

| ID | Confidence | Stage | Impact | Resolution Urgency |
|----|-----------|-------|--------|-------------------|
| UNC-BILATERAL-001 | 0.6 | 1+ | Misleading convergence signal | Medium — calibrate during Stage 0 |
| UNC-BILATERAL-002 | 0.5 | 1+ | Misguided remediation priority | Medium — calibrate during Stage 0 |
| UNC-STORE-001 | 0.95 | 0 | Silent data corruption (extremely unlikely) | Low — monitor during implementation |
| UNC-STORE-002 | 0.9 | 0 | Transaction ordering violation | Low — mitigated by single-VPS deployment |
| UNC-QUERY-001 | 0.8 | 1+ | Query timeout in interactive use | Medium — benchmark during Stage 0 |
| UNC-HARVEST-001 | 0.7 | 0 | Knowledge loss or wasted budget | High — calibrate during Stage 0 |
| UNC-GUIDANCE-001 | 0.7 | 0 | Insufficient or excessive drift correction | High — instrument during Stage 0 |
| UNC-DELIBERATION-001 | 0.7 | 2 | Premature or delayed decisions | Medium — simulate during Stage 2 |
| UNC-SCHEMA-001 | 0.85 | 0 | Missing bootstrap attribute | High — verify during Stage 0 |
| UNC-RESOLUTION-001 | 0.8 | 0 | Incorrect conflict resolution | Medium — provide defaults + warnings |
| UNC-LAYOUT-001 | 0.85 | 0+ | Filesystem perf degradation at scale | Medium — benchmark during Stage 0 |
| UNC-LAYOUT-002 | 0.90 | 0+ | Slow cold start from EDN parsing | Medium — benchmark during Stage 0 |
| UNC-LAYOUT-003 | 0.80 | 3+ | Git packfile inefficiency with small files | Low — benchmark at Stage 3 scale |

**By resolution urgency**:
- **High** (resolve during Stage 0): UNC-HARVEST-001, UNC-GUIDANCE-001, UNC-SCHEMA-001
- **Medium** (resolve during Stage 0–2): UNC-BILATERAL-001/002, UNC-QUERY-001, UNC-DELIBERATION-001, UNC-RESOLUTION-001, UNC-LAYOUT-001/002
- **Low** (monitor, resolve if observed): UNC-STORE-001/002, UNC-LAYOUT-003

---

