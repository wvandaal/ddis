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

**By resolution urgency**:
- **High** (resolve during Stage 0): UNC-HARVEST-001, UNC-GUIDANCE-001, UNC-SCHEMA-001
- **Medium** (resolve during Stage 0–2): UNC-BILATERAL-001/002, UNC-QUERY-001, UNC-DELIBERATION-001, UNC-RESOLUTION-001
- **Low** (monitor, resolve if observed): UNC-STORE-001/002

---

