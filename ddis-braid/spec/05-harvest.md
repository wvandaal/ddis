> **Namespace**: HARVEST | **Wave**: 2 (Lifecycle) | **Stage**: 0
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §5. HARVEST — End-of-Session Extraction

### §5.0 Overview

Harvest is the mechanism by which knowledge survives conversation boundaries. At the end
of a conversation (or when context budget is critically low), the agent extracts durable
knowledge — observations, decisions, dependencies, uncertainties — from the ephemeral
conversation into the permanent datom store.

The fundamental insight: **conversations are disposable; knowledge is durable.** Harvest
transforms the workflow from "fight to keep conversations alive" to "ride bounded context
waves, extracting knowledge at each crest."

**Traces to**: SEED.md §5
**ADRS.md sources**: LM-005–006, LM-011–013, IB-012, CR-005, UA-007

---

### §5.1 Level 0: Algebraic Specification

#### Epistemic Gap

```
Let K_agent(t) = knowledge held by the agent at time t (in conversation context)
Let K_store(t) = knowledge in the datom store at time t

Epistemic gap: Δ(t) = K_agent(t) \ K_store(t)
  (knowledge the agent has that the store does not)

Harvest: HARVEST(Δ(t)) → K_store(t') where K_store(t') ⊇ K_store(t) ∪ Δ(t)
  The store grows to include the agent's un-transacted knowledge.

Perfect harvest: Δ(t') = ∅
  (all agent knowledge is in the store after harvest)

Practical harvest: |Δ(t')| ≤ ε
  (residual gap below acceptable threshold)
```

#### Harvest as Monotonic Extension

```
∀ harvest operations H:
  K_store(pre) ⊆ K_store(post)          — store grows (C1)
  |K_store(post)| ≥ |K_store(pre)|      — never shrinks (L5)

Harvest does not modify existing datoms. It only adds new datoms
representing the agent's un-transacted observations and decisions.
```

#### Harvest Quality Metrics

```
false_positive_rate = |{candidates committed then later retracted}| / |{committed}|
false_negative_rate = |{candidates rejected then later re-discovered}| / |{rejected}|
drift_score = |Δ(t)| at session end before harvest

Calibration: High FP → raise thresholds. High FN → lower thresholds.
Both high → improve extractor.

Quality bands:
  0–2 uncommitted observations at harvest time = excellent
  3–5 = minor drift
  6+  = significant drift (methodology not followed)
```

---

### §5.2 Level 1: State Machine Specification

#### Harvest Pipeline

```
HARVEST(S, agent, transcript_context) → S'

PIPELINE:
  1. DETECT: Scan agent's recent transactions. Identify:
     - Observations made but not transacted (implicit knowledge)
     - Decisions made but not recorded as ADR datoms
     - Dependencies discovered but not linked
     - Uncertainties encountered but not marked

  2. PROPOSE: Generate harvest candidates.
     Each candidate c has:
       c.datom_spec    — the datom(s) to transact
       c.category      — observation | decision | dependency | uncertainty
       c.confidence    — extraction confidence (0.0–1.0)
       c.weight        — commitment weight estimate

  3. REVIEW: Agent/human confirms or rejects each candidate.
     Review topology (LM-012) determines who reviews:
       single-agent self-review (default)
       bilateral peer review
       swarm broadcast + voting
       hierarchical specialist delegation
       human review

  4. COMMIT: Confirmed candidates transacted as datoms.
     Each committed candidate becomes a Transaction with:
       provenance = :observed or :derived
       rationale = harvest extraction context
       causal_predecessors = session's transaction chain

  5. RECORD: Harvest session entity created.
     Records: session_id, agent, topology, candidate_count,
     committed_count, rejected_count, drift_score, timestamp.

POST:
  S'.datoms ⊇ S.datoms                          — monotonic (C1)
  harvest_session entity in S'                    — provenance trail
  ∀ committed candidates: datoms in S'            — knowledge captured
  drift_score(S') recorded for calibration        — learning signal
```

#### Proactive Harvest Warnings

```
When Q(t) < 0.15 (~75% context consumed):
  Every CLI response includes harvest warning.
  "Context budget low. Run `braid harvest` to preserve session knowledge."

When Q(t) < 0.05 (~85% context consumed):
  CLI emits ONLY the harvest imperative.
  "HARVEST NOW. Run `braid harvest`. Further work will degrade."

Continuing past harvest threshold produces diminishing returns —
outputs become parasitic (consuming budget without producing value).
```

#### Crystallization Stability Guard

```
Harvest candidates with high commitment weight require stability check:

crystallizable(candidate) =
  candidate.status = :refined ∧
  candidate.confidence ≥ 0.6 ∧
  candidate.coherence ≥ 0.6 ∧
  no_unresolved_conflicts(candidate) ∧
  stability_score(candidate) ≥ stability_min (default 0.7)

Candidates below stability threshold remain as :proposed in the harvest
session, not committed. They surface in the next session's seed as
"pending crystallization."
```

#### Observation Staleness Model

```
Observation datoms carry freshness metadata:
  :observation/source    — :filesystem | :shell | :network | :git | :process
  :observation/timestamp — when observed
  :observation/hash      — content hash at observation time
  :observation/stale-after — TTL (source-dependent)

Freshness check during harvest:
  if now - observation.timestamp > stale_after:
    flag as potentially stale
    ASSEMBLE applies freshness-mode: :warn (default) | :refresh | :accept
```

---

### §5.3 Level 2: Interface Specification

```rust
/// Harvest candidate — proposed datom extraction from conversation.
pub struct HarvestCandidate {
    pub id:                  usize,                  // Index for accept/reject referencing in CLI
    pub datom_spec:          Vec<Datom>,
    pub category:            HarvestCategory,
    pub confidence:          f64,                    // 0.0–1.0
    pub weight:              f64,                    // estimated commitment weight
    pub status:              CandidateStatus,        // lattice: :proposed < :under-review < :committed < :rejected
    pub extraction_context:  String,                 // why this was extracted
    pub reconciliation_type: ReconciliationType,     // Traces to reconciliation taxonomy (§15)
}

pub enum HarvestCategory {
    Observation,     // fact observed but not transacted
    Decision,        // choice made but not recorded as ADR
    Dependency,      // link discovered but not asserted
    Uncertainty,     // unknown encountered but not marked
}

/// Candidate lifecycle lattice: proposed < under-review < committed | rejected.
/// The lattice ordering ensures forward-only progress through the review pipeline.
/// Motivating invariant: INV-HARVEST-001 (harvest monotonicity — no status reversal).
pub enum CandidateStatus {
    Proposed,                // Initial state after detection
    UnderReview,             // Being reviewed by the selected topology
    Committed,               // Approved and transacted into the store
    Rejected(String),        // Rejected with reason (terminal state)
}

/// Harvest session entity.
pub struct HarvestSession {
    pub session_id: EntityId,
    pub agent: AgentId,
    pub review_topology: ReviewTopology,
    pub candidates: Vec<HarvestCandidate>,
    pub drift_score: u32,           // count of uncommitted observations
    pub timestamp: Instant,
}

pub enum ReviewTopology {
    SelfReview,                     // single agent reviews own work
    PeerReview { reviewer: AgentId },
    SwarmVote { quorum: u32 },
    HierarchicalDelegation { specialist: AgentId },
    HumanReview,
}

// --- Free functions (ADR-ARCHITECTURE-001) ---

/// Harvest detection pipeline: scans agent's recent transactions and proposes candidates.
/// Free function: harvest detection is a read-only pipeline that queries the store
/// for un-transacted knowledge. See guide/05-harvest.md for the decomposed pipeline.
pub fn harvest_pipeline(store: &Store, session_context: &SessionContext) -> HarvestResult;

/// Accept a harvest candidate by building a transaction for commitment.
/// The caller commits via Store::transact(). This reuses the core mutation path
/// rather than creating a parallel mutation path inside Store.
pub fn accept_candidate(candidate: &HarvestCandidate, agent: AgentId) -> Transaction<Building>;

/// Create a HarvestSession entity recording the harvest metadata.
/// The caller commits via Store::transact().
pub fn harvest_session_entity(
    result: &HarvestResult,
    agent: AgentId,
    topology: ReviewTopology,
) -> Transaction<Building>;
```

#### CLI Commands

```
braid harvest                       # Interactive: detect, propose, review, commit
braid harvest --auto                # Auto-commit candidates above confidence threshold
braid harvest --dry-run             # Show candidates without committing
braid harvest --topology peer       # Use peer review topology
braid harvest --stats               # Show harvest quality metrics (FP/FN rates)
```

---

### §5.4 Invariants

### INV-HARVEST-001: Harvest Monotonicity

**Traces to**: SEED §5, C1
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ harvest operations H on store S:
  S ⊆ HARVEST(S)
  (harvest only adds datoms, never removes)
```

#### Level 1 (State Invariant)
Every harvest commit is a TRANSACT operation, inheriting all STORE invariants.
No existing datom is modified or removed during harvest.

#### Level 2 (Implementation Contract)
```rust
// Free function (ADR-ARCHITECTURE-001): builds a transaction from confirmed candidates.
// Caller commits via Store::transact(), which preserves INV-STORE-001.
pub fn accept_candidate(candidate: &HarvestCandidate, agent: AgentId) -> Transaction<Building> {
    let mut tx = Transaction::<Building>::new(agent);
    for datom in &candidate.datom_spec {
        tx = tx.assert_datom(datom.entity, datom.attribute.clone(), datom.value.clone());
    }
    tx
}

// Usage in CLI layer:
// for candidate in confirmed_candidates {
//     let tx = accept_candidate(&candidate, agent).commit(&store.schema())?;
//     store.transact(tx)?;
// }
```

**Falsification**: A harvest operation that reduces the datom count or removes existing datoms.

---

### INV-HARVEST-002: Harvest Provenance Trail

**Traces to**: SEED §5, ADRS FD-012
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ harvest operations:
  ∃ HarvestSession entity in S' recording:
    agent, timestamp, candidate_count, drift_score, topology
  ∀ committed candidates:
    ∃ transaction with provenance tracing to the harvest session
```

#### Level 1 (State Invariant)
Every harvest creates a HarvestSession entity. Every committed candidate has a
transaction whose causal predecessors include the harvest session entity.

**Falsification**: A harvest that commits candidates without creating a HarvestSession
entity, or candidates whose transactions have no provenance link to the session.

---

### INV-HARVEST-003: Drift Score Recording

**Traces to**: ADRS LM-006
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ harvest sessions:
  drift_score = |uncommitted observations| at harvest time
  drift_score is stored as a datom on the HarvestSession entity
```

#### Level 1 (State Invariant)
The drift score is recorded per session, enabling longitudinal tracking of
harvest discipline. Quality bands: 0–2 = excellent, 3–5 = minor, 6+ = significant.

**Falsification**: A harvest session entity without a drift_score attribute.

---

### INV-HARVEST-004: FP/FN Calibration

**Traces to**: ADRS LM-006
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ committed candidates c:
  if c is later retracted: FP_count += 1
∀ rejected candidates c:
  if c's knowledge is later re-discovered: FN_count += 1

Calibration rule:
  FP_rate > threshold → raise extraction confidence threshold
  FN_rate > threshold → lower extraction confidence threshold
  Both high → improve extractor (not just thresholds)
```

#### Level 1 (State Invariant)
The harvest system tracks empirical quality and adjusts thresholds.
False positives and false negatives are both measurable from the store.

**Falsification**: Harvest thresholds that never adjust despite persistent FP/FN rates.

---

### INV-HARVEST-005: Proactive Warning

**Traces to**: ADRS IB-012
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ CLI responses when Q(t) < 0.15:
  response includes harvest warning
∀ CLI responses when Q(t) < 0.05:
  response = ONLY harvest imperative (no other content)
```

#### Level 1 (State Invariant)
The budget system triggers harvest warnings at context consumption thresholds.
Below the critical threshold, all output is suppressed except the harvest command.

**Stage 0 simplification**: At Stage 0, Q(t) is not yet available (BUDGET is Stage 1).
The Stage 0 implementation uses a turn-count heuristic as a proxy: warn at turn 20,
imperative at turn 40. Stage 1 replaces this with the formal Q(t) formula.

**Falsification**: A CLI response at Q(t) < 0.05 that contains content other than
the harvest imperative.

---

### INV-HARVEST-006: Crystallization Guard

**Traces to**: ADRS CR-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ harvest candidates c with commitment_weight(c) > crystallization_threshold:
  c MUST NOT be committed unless:
    c.confidence ≥ 0.6 ∧
    c.coherence ≥ 0.6 ∧
    no_unresolved_conflicts(c) ∧
    stability_score(c) ≥ stability_min
```

#### Level 1 (State Invariant)
High-weight candidates require stability verification before commitment.
This prevents premature crystallization of uncertain knowledge into
load-bearing datoms.

**Falsification**: A high-weight candidate committed with stability_score below threshold.

**proptest strategy**: Generate candidates with varying weights and stability scores.
Verify that only stable, high-confidence candidates with weights above threshold
pass the crystallization guard.

---

### INV-HARVEST-007: Bounded Conversation Lifecycle

**Traces to**: ADRS LM-011
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Agent lifecycle is a bounded cycle:
  SEED → work(20–30 turns) → HARVEST → conversation_end → SEED → ...

Each conversation is a bounded trajectory:
  high-quality reasoning for a limited window before attention degrades.
  Produces: durable knowledge (datoms) + ephemeral reasoning (conversation).
  At end: ephemeral released, durable persists.
```

#### Level 1 (State Invariant)
The system enforces a bounded lifecycle through proactive warnings (INV-HARVEST-005)
and budget monitoring. Conversations that exceed the attention degradation threshold
without harvesting produce lower-quality output.

**Falsification**: An agent operating for 50+ turns without a harvest or harvest warning.

---

### INV-HARVEST-008: Delegation Topology Support

**Traces to**: ADRS LM-012
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ harvest sessions: topology ∈ {self, peer, swarm, hierarchical, human}

Topology selection based on commitment weight:
  auto_threshold = 0.15: self-review sufficient
  peer_threshold = 0.40: peer review recommended
  human_threshold = 0.70: human review required

harvest_weight(candidate) = intrinsic_weight(candidate) × confidence(extraction)
```

#### Level 1 (State Invariant)
High-weight harvest candidates are routed to higher-authority review topologies.
The topology is recorded on the HarvestSession entity.

**Falsification**: A harvest session with high-weight candidates using self-review topology.

---

### INV-HARVEST-009: Continuous Externalization Protocol

**Traces to**: SEED §5, ADRS LM-005, LM-013, INV-GUIDANCE-001, INV-GUIDANCE-007
**Verification**: `V:PROP`
**Stage**: 2 (partial at Stage 0 via structured harvest prompt; full protocol at Stage 2)

#### Problem: The Category 5 Ceiling

Harvest detection faces a fundamental observability problem with **Category 5 knowledge**
(reasoning conclusions, trade-off assessments, confidence levels). This knowledge exists only
in the agent's internal reasoning and is never surfaced through tool calls or transactions.
Heuristic detection hits a ceiling: the agent cannot reliably self-report knowledge it doesn't
know it has, and LLM-assisted detection at session end suffers from the same attention
degradation that harvest is designed to counteract.

The architectural insight: **don't detect — prevent.** Instead of trying to find Category 5
knowledge after the fact, restructure the workflow so knowledge is externalized at the moment
of discovery, converting Category 5 → Category 2 in real time.

#### Level 0 (Algebraic Law)

```
Continuous externalization: E: Response → MicroTransaction*

For every agent response r that produces implicit knowledge k:
  E(r) generates micro-transaction annotations:
    ↳ Learned: [category] description

where category ∈ {observation, decision, dependency, uncertainty}

The externalization function satisfies:
  1. Timeliness: knowledge externalized within the same response that produced it
     (not deferred to session end)
  2. Completeness: E(r) covers all four HarvestCategory variants
  3. Low overhead: |E(r)| ≤ 3 annotations per response (avoid annotation fatigue)
  4. Composability: annotations compose into harvest candidates via
     DETECT: ∀ annotation a ∈ session: ¬transacted(a) → HarvestCandidate

Category 5 shrinkage:
  Without externalization: |Cat5(t)| grows with session length
  With externalization:    |Cat5(t)| ≤ ε (bounded residual)

  The residual ε represents genuinely tacit knowledge — the rate-distortion
  limit below which externalization cannot reach (~2-5% of total knowledge).
```

#### Level 1 (State Invariant)

The continuous externalization protocol operates as an overlay on the guidance injection
system (INV-GUIDANCE-001). The dynamic CLAUDE.md (INV-GUIDANCE-007) includes externalization
obligations that prompt the agent to annotate responses with micro-transaction markers.

Pipeline integration:

```
Normal tool response flow:
  Tool Output → Guidance Footer (INV-GUIDANCE-001) → Agent

Externalization-augmented flow:
  Tool Output → Guidance Footer → Agent → Response with ↳ Learned annotations
  ↓
  Next harvest: annotations become pre-populated candidates (confidence boost)
```

The externalization annotations are NOT automatically transacted. They are structured hints
that the harvest pipeline (§5.2 DETECT stage) uses to generate higher-confidence candidates.
This preserves semi-automated review (ADR-HARVEST-001) while dramatically reducing false
negatives for Category 5 knowledge.

**Stage-by-stage activation**:
- **Stage 0**: Structured harvest prompt template (§5.1a) asks targeted questions. Category 5
  capture: ~60-70%.
- **Stage 1**: Dynamic CLAUDE.md includes externalization obligations. Harvest pipeline
  ingests `↳ Learned:` annotations as pre-candidates. Category 5 capture: ~80-85%.
- **Stage 2**: Full protocol with fresh-agent review (ADR-HARVEST-004) specifically targeting
  Category 5 residuals. Continuous externalization + dual-process review. Category 5
  capture: ~90-95%.
- **Stage 3+**: Cross-agent externalization, transcript analysis, pattern learning from
  historical FP/FN data. Category 5 capture: ~95-98%.

The ~2-5% residual at convergence represents the **rate-distortion limit**: genuinely tacit
knowledge that the agent cannot articulate regardless of prompting strategy.

#### Level 2 (Implementation Contract)

```rust
/// Micro-transaction annotation from continuous externalization.
/// These are structured hints, not committed datoms — they feed into
/// the harvest pipeline as pre-candidates with elevated confidence.
pub struct ExternalizationAnnotation {
    pub category:    HarvestCategory,       // observation | decision | dependency | uncertainty
    pub description: String,                // what was learned
    pub response_id: usize,                 // which response produced this
    pub confidence:  f64,                    // self-assessed confidence (0.0-1.0)
}

/// Ingest externalization annotations into the harvest pipeline.
/// Annotations matching existing store datoms are filtered out (already transacted).
/// Remaining annotations become HarvestCandidates with boosted confidence:
///   candidate.confidence = max(annotation.confidence, 0.7)
/// The 0.7 floor reflects that explicitly externalized knowledge is higher-quality
/// than heuristically detected knowledge.
pub fn ingest_annotations(
    store: &Store,
    annotations: &[ExternalizationAnnotation],
) -> Vec<HarvestCandidate>;
```

**Falsification**: An agent operates for 10+ responses producing reasoning conclusions
without any externalization annotations, AND the session's harvest detects 0 Category 5
candidates (indicating both externalization and detection failed), OR the harvest pipeline
ignores externalization annotations (does not boost confidence), OR annotations are
auto-committed without review (violating ADR-HARVEST-001).

---

### §5.5 ADRs

### ADR-HARVEST-001: Semi-Automated Over Fully Automatic

**Traces to**: ADRS LM-005
**Stage**: 0

#### Problem
Should harvest be fully automatic or require agent/human confirmation?

#### Options
A) **Fully automatic** — system extracts and commits without review.
B) **Semi-automated** — system proposes candidates; agent/human confirms.
C) **Fully manual** — agent must explicitly identify all harvestable knowledge.

#### Decision
**Option B.** The system detects harvestable knowledge from transaction analysis and
presents candidates for confirmation. This balances extraction coverage (higher than C)
with precision (lower FP rate than A).

#### Formal Justification
Fully automatic harvest (Option A) risks high false positive rates — committing
speculative observations as established facts. Fully manual (Option C) risks high
false negative rates — agents forgetting to harvest key decisions. Semi-automated
balances both failure modes and provides calibration data (FP/FN rates) for improvement.

---

### ADR-HARVEST-002: Conversations Disposable, Knowledge Durable

**Traces to**: SEED §5, ADRS LM-003
**Stage**: 0

#### Problem
What is the relationship between conversations and durable state?

#### Options
A) **Conversations are disposable** — knowledge extracted to store; conversation discarded.
B) **Conversations are archival** — full transcripts preserved alongside store.
C) **Conversations are primary** — store is an index into conversations.

#### Decision
**Option A.** Conversations are bounded reasoning trajectories. Knowledge lives in the
store. Conversations are lightweight and replaceable — start one, work 20–30 turns,
harvest, discard, start fresh. The agent never loses anything.

#### Formal Justification
Option B preserves too much — conversation transcripts are voluminous and mostly
redundant with the extracted datoms. Option C inverts the architecture — makes the
store dependent on ephemeral artifacts. Option A aligns with the harvest/seed lifecycle:
knowledge survives; reasoning sessions do not.

---

### ADR-HARVEST-003: FP/FN Tracking for Calibration

**Traces to**: ADRS LM-006
**Stage**: 1

#### Problem
How to improve harvest quality over time?

#### Decision
Track empirical FP/FN rates per agent and per category. A committed candidate later
retracted is a false positive. A rejected candidate whose knowledge is later re-discovered
is a false negative. Rates feed back into threshold adjustment.

#### Formal Justification
Harvest quality is measurable from the store: retractions of harvest-committed datoms
and re-discoveries of rejected candidates are both detectable by querying transaction
history. This makes harvest improvement a data-driven process.

---

### ADR-HARVEST-004: Five Review Topologies

**Traces to**: ADRS LM-012
**Stage**: 2

#### Problem
Who reviews harvest candidates?

#### Decision
Five topologies: (1) self-review (default — agent reviews own harvest), (2) bilateral
peer review (a second agent reviews), (3) swarm broadcast + voting (multiple agents vote),
(4) hierarchical specialist delegation (route to domain expert), (5) human review.

The "Fresh-Agent Self-Review" pattern exploits maximum context asymmetry: the depleted
agent proposes candidates, a fresh session reviews them with full attention budget.

#### Category 5 Targeting in Fresh-Agent Review

Fresh-agent reviewers are specifically directed to hunt for **Category 5 knowledge** —
reasoning conclusions that the depleted agent holds implicitly but did not externalize.
The fresh agent, with full attention budget, reviews the session's conversation transcript
(not just the tool call log) and specifically looks for:

1. **Implicit conclusions**: "The agent decided X over Y but didn't record why"
2. **Trade-off assessments**: "The agent weighed options A, B, C and chose B — was the
   reasoning captured?"
3. **Confidence levels**: "The agent expressed uncertainty about X but didn't create an
   uncertainty datom"
4. **Emergent understanding**: "The agent's behavior changed after reading file F, suggesting
   it learned something — was that observation recorded?"

The fresh-agent review prompt template includes these four Category 5 detection patterns
as explicit search directives, not generic review guidance.

**Conversation transcript analysis** is the key mechanism: tool call logs capture *what*
the agent did, but conversation transcripts capture *what the agent concluded*. The harvest
pipeline's Layer 2 (LLM-assisted detection) at Stage 2+ operates on the conversation
transcript, not just the tool call log.

### ADR-HARVEST-005: Observation Staleness Model

**Traces to**: SEED §5, ADRS UA-007
**Stage**: 0

#### Problem
Observation datoms represent facts about the external world (filesystem state, process
output, network responses) at a specific point in time. These observations become stale
as the external world changes. How should the system track and handle observation staleness
during harvest and seed assembly?

#### Options
A) **No staleness tracking** — treat all observations as current. Simple but dangerous:
   an observation from 3 days ago about a file's content may be completely wrong.
B) **Time-based TTL only** — observations expire after a fixed duration. Uniform but
   ignores that different observation sources have different staleness characteristics
   (filesystem changes frequently; architectural decisions rarely).
C) **Source-aware staleness model** — observations carry source-specific metadata including
   per-source TTLs and freshness modes, with configurable behavior when stale observations
   are encountered during assembly.

#### Decision
**Option C.** Observation datoms carry the following freshness metadata:

```
:observation/source      — keyword: :filesystem | :shell | :network | :git | :process
:observation/path        — string: path or identifier of observed resource
:observation/timestamp   — instant: when the observation was made
:observation/hash        — bytes: content hash at observation time
:observation/stale-after — duration: TTL (source-dependent default)
```

Default TTLs by source:
- `:filesystem` — 1 hour (files change frequently during active development)
- `:shell` — 30 minutes (command output reflects transient state)
- `:network` — 15 minutes (API responses may change rapidly)
- `:git` — 24 hours (commit history is append-only; recent state is relatively stable)
- `:process` — 5 minutes (process state is highly volatile)

During ASSEMBLE (seed assembly), the system checks observation freshness:

```
if now - observation.timestamp > stale_after:
  apply freshness_mode:
    :warn    (default) — include observation with staleness warning
    :refresh — re-observe before including (requires tool access)
    :accept  — include without warning (for explicitly stable observations)
```

#### Formal Justification
Source-aware staleness captures the empirical reality that different data sources have
different change rates. The `:observation/hash` field enables efficient staleness detection:
if the hash still matches the current content, the observation is fresh regardless of
elapsed time. The freshness mode is configurable per observation, allowing agents to
override defaults for known-stable or known-volatile resources.

The model integrates with the harvest pipeline (§5.2): during DETECT, observations whose
staleness has expired are flagged as re-observation candidates. During ASSEMBLE (§6.2),
stale observations are annotated with warnings so the agent knows to re-verify before
relying on them.

#### Consequences
- Agents can trust the temporal validity of observation datoms in their seed
- Stale observations are surfaced as warnings rather than silently included
- The `:refresh` mode enables proactive re-observation but requires tool access
  (not always available during seed assembly)
- TTL defaults are stored as datoms (C3), enabling per-project calibration

#### Falsification
The staleness model is wrong if: (1) the default TTLs are consistently too aggressive
(flagging observations as stale that have not actually changed), wasting re-observation
effort, or (2) too permissive (including observations that have changed, leading agents
to act on outdated information). Track stale-flagged observations vs. actual content
change to calibrate TTLs.

---

### ADR-HARVEST-006: DDR Feedback Loop

**Traces to**: SEED §7, ADRS LM-014
**Stage**: 0

#### Problem
When practical usage of Braid reveals gaps in the specification — missing invariants,
inadequate ADRs, underspecified behavior — how should these discoveries feed back into
the specification? Without a structured feedback mechanism, spec gaps accumulate silently
and diverge from the evolving implementation.

#### Options
A) **Ad-hoc spec updates** — agents directly modify spec files when they notice gaps.
   Fast but produces untracked changes with no provenance, no impact analysis, and no
   review process. Violates the bilateral principle (spec changes should flow through
   the same protocol as implementation changes).
B) **Issue tracker** — create issues for spec gaps and resolve them in separate sessions.
   Provides tracking but disconnects the discovery context from the resolution. By the
   time the issue is addressed, the original observation context may be lost.
C) **DDR (DDIS Decision Record) as datom** — when usage reveals a spec gap, record it
   as a structured DDR entity in the store with sections for Observation, Impact on Spec,
   Resolution Options, Decision, and Spec Update. DDRs flow through the standard harvest
   pipeline and are datoms themselves.

#### Decision
**Option C.** Spec gaps discovered during usage are recorded as DDR entities with the
following structure:

```
DDR entity attributes:
  :ddr/observation      — what was observed (the gap or inadequacy)
  :ddr/impact           — which spec elements are affected
  :ddr/options          — resolution options considered
  :ddr/decision         — which option was chosen and why
  :ddr/spec-update      — reference to the spec elements modified
  :ddr/session          — reference to the session that discovered the gap
  :ddr/status           — lattice: :observed < :analyzed < :resolved < :verified
```

DDR frequency adapts to project maturity:
- **Stage 0**: Every session produces DDRs (high discovery rate, spec is fresh)
- **Stage 1**: Every few sessions (spec stabilizing, fewer gaps discovered)
- **Stage 2+**: Weekly review (spec mature, DDRs primarily for edge cases)

#### Formal Justification
DDRs close the bilateral loop between implementation experience and specification quality.
They are the backward-flow counterpart to the forward-flow ADR: where ADRs flow from
spec to implementation ("here is what to build"), DDRs flow from implementation to spec
("here is what we learned by building"). Together they form the bilateral specification
cycle described in SEED.md §7.

DDRs are datoms in the store (not external artifacts), which means they:
- Have provenance: which agent, which session, what was the observation context
- Are queryable: "show me all unresolved DDRs affecting the HARVEST namespace"
- Participate in conflict resolution: competing DDR resolutions for the same gap are
  handled by the standard conflict pipeline
- Feed into the fitness function: unresolved DDRs reduce F(S) completeness score

The decreasing frequency schedule reflects the natural convergence of a specification:
early stages have many gaps (high DDR rate), mature stages have few (low DDR rate).
The DDR rate itself is a convergence signal.

#### Consequences
- Spec gaps are never lost — they are captured as datoms at the moment of discovery
- The bilateral cycle is explicit: implementation informs spec via DDRs, spec informs
  implementation via ADRs
- DDR frequency serves as a convergence metric: decreasing DDR rate indicates spec maturity
- DDRs are reviewable: the harvest pipeline applies the same review topology
  (ADR-HARVEST-004) to DDR candidates as to other harvest candidates

#### Falsification
The DDR feedback loop is wrong if: (1) spec gaps consistently go unrecorded despite being
observed (DDR discipline breaks down — the same failure mode as unharvested sessions,
NEG-HARVEST-001), or (2) DDRs accumulate without resolution (the analysis/resolution
pipeline is too slow or too burdensome), or (3) the frequency schedule is wrong (Stage 0
produces too few DDRs to capture the rapid early-stage gap discovery rate).

---

### ADR-HARVEST-007: Turn-Count Proxy for Context Budget at Stage 0

**Traces to**: SEED §10 (staged roadmap), INV-HARVEST-005, INV-HARVEST-007
**Stage**: 0

#### Problem
INV-HARVEST-005 specifies proactive harvest warnings when the context quality function
Q(t) falls below threshold: warn at Q(t) < 0.15, harvest-imperative at Q(t) < 0.05. The
formal definition of Q(t) is `Q(t) = k*_eff × attention_decay(k*_eff)`, which requires
measured context consumption from the BUDGET namespace (§13). BUDGET is a Stage 1
deliverable. Without a budget system, Q(t) literally cannot be computed, yet the safety
property of INV-HARVEST-005 — "never let an agent continue past harvest-without-warning" —
must hold from Stage 0 onward, because knowledge loss from unharvested sessions is the
single most damaging failure mode at any stage.

This ADR resolves the cross-stage dependency: INV-HARVEST-005 (Stage 0) depends on BUDGET
(Stage 1) for its formal trigger mechanism.

#### Options
A) **Pull BUDGET into Stage 0** — implement the full context budget system (k*_eff
   computation, attention decay model, token consumption tracking) at Stage 0 so Q(t)
   is available. This adds a significant dependency chain: BUDGET requires token counting,
   model-specific attention decay curves, and the k* effective-window calculation — all
   substantial subsystems that are not prerequisites for the core Stage 0 deliverables
   (store, transact, query, harvest, seed).

B) **Turn-count heuristic as proxy** — replace Q(t) with a turn-count proxy: warn at
   turn 20, harvest-imperative at turn 40. Turn count is a crude but conservative
   estimate of context consumption. The thresholds are stored as configurable datoms
   (per ADR-INTERFACE-005), not hard-coded constants.

C) **Defer INV-HARVEST-005 to Stage 1** — delay the proactive warning system entirely
   until BUDGET is available. This means Stage 0 agents operate without harvest warnings,
   relying entirely on manual discipline to harvest before context degradation.

D) **File-size-based proxy** — sum the byte sizes of tool results returned during the
   session as a proxy for token consumption. More granular than turn count: a turn that
   reads 50 files produces a larger byte total than a turn doing arithmetic. However,
   bytes-to-tokens conversion is model-dependent and imprecise, and the proxy still
   requires instrumenting all tool responses.

#### Decision
**Option B.** At Stage 0, the proactive harvest warning system uses a turn-count
heuristic as a proxy for Q(t):

```
turn_count_warning(turn) =
  if turn >= imperative_threshold (default 40): HARVEST_IMPERATIVE
  if turn >= warning_threshold (default 20):    HARVEST_WARNING
  else:                                         NO_WARNING
```

The thresholds (20 and 40) are stored as datoms in the store:
```
[:db/ident :config/harvest-warning-threshold, :db/valueType :long, value: 20]
[:db/ident :config/harvest-imperative-threshold, :db/valueType :long, value: 40]
```

Stage 1 replaces this function with the formal Q(t) computation. The replacement is
transparent to callers: the warning function's return type (`WarningLevel`) is identical
regardless of whether the trigger is turn-count or Q(t).

#### Formal Justification
The L0 invariant of INV-HARVEST-005 is a safety property:

```
□ ¬(∃ CLI response at Q(t) < 0.05 that contains non-harvest content)
```

Turn count preserves this safety property because the failure mode is **asymmetric**:

- **Too-early warning** (turn count triggers before Q(t) would): The agent sees a
  harvest warning when it still has remaining context budget. This is annoying but
  **safe** — it may cause premature harvesting, but no knowledge is lost.
- **Too-late warning** (turn count triggers after Q(t) would): The agent continues
  working past the point of attention degradation without warning. This causes
  **knowledge loss** — the critical failure that INV-HARVEST-005 exists to prevent.

Turn count is a **conservative** proxy: a turn count of 20 is reached before Q(t)
drops to 0.15 in the vast majority of sessions, because most turns consume meaningful
context (reading files, analyzing code, making decisions). The edge case where turn
count is too late — turns that consume very large amounts of context (e.g., reading
entire codebases) — is rare at Stage 0 where sessions are typically single-task.

The safety property of INV-HARVEST-007 (bounded conversation lifecycle) is also
preserved: the imperative threshold at turn 40 ensures no session exceeds 40 turns
without a harvest imperative, well within the "20-30 turn" bounded lifecycle window.

#### Consequences
- **Lost signal quality**: Turn count is a poor proxy for actual context consumption.
  A turn that reads 50 files consumes ~100x more context than a turn doing simple
  arithmetic, yet both count as 1 turn. The warning system cannot distinguish between
  lightweight and heavyweight turns.
- **Conservative bias**: Agents will see harvest warnings earlier than necessary in
  sessions with many lightweight turns (e.g., a series of small queries). This trades
  efficiency for safety — an acceptable tradeoff at Stage 0 where the methodology is
  being validated, not optimized.
- **Configurable thresholds**: The 20/40 defaults are arbitrary initial values. They
  are stored as datoms (C3: schema-as-data), enabling per-project calibration based on
  observed session patterns. Projects with heavier per-turn context consumption should
  lower the thresholds; projects with lightweight turns may raise them.
- **Full behavior activates at Stage 1**: When BUDGET is implemented, the turn-count
  proxy is replaced by the formal Q(t) computation. The `WarningLevel` return type is
  unchanged, so all downstream consumers (CLI formatting, guidance injection) are
  unaffected by the switch.
- **Risk: heavyweight-turn knowledge loss**: If a Stage 0 session has 15 turns, each
  reading a large file, context may be critically depleted before the turn-20 warning.
  This is the residual risk accepted by choosing turn-count over file-size proxy
  (Option D). Mitigated by the manual harvest discipline (CLAUDE.md harvest checklist)
  and the bounded lifecycle expectation (INV-HARVEST-007).

#### Falsification
This simplification is inadequate if: a Stage 0 session demonstrably suffers knowledge
loss due to context degradation that occurred before the turn-count warning threshold —
specifically, if an agent produces observably lower-quality output (hallucinations,
contradictions, forgotten context) at turn 15 in a heavy-context session, and the
turn-count system has not yet issued a warning. If this pattern is observed in 3+ sessions,
the turn-count proxy must be replaced or supplemented (Option D) before Stage 1.

---

### §5.6 Negative Cases

### NEG-HARVEST-001: No Unharvested Session Termination

**Traces to**: SEED §5, ADRS IB-012
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ session termination with drift_score > 0 and no harvest warning issued)`

**Formal statement**: Every session that ends with uncommitted observations MUST have
issued at least one harvest warning before termination. The warning is triggered by
Q(t) threshold crossing, not by session end detection.

**proptest strategy**: Simulate sessions with varying transaction/observation ratios.
Verify that all sessions with uncommitted observations receive harvest warnings.

---

### NEG-HARVEST-002: No Harvest Data Loss

**Traces to**: C1
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(∃ committed harvest candidate whose datoms are not in the store post-harvest)`

**Formal statement**: Every candidate with `status = :committed` has its datom_spec
present in `S'.datoms` after the harvest transaction completes.

**Kani harness**: Bounded check that for any set of committed candidates, all specified
datoms appear in the post-harvest store.

---

### NEG-HARVEST-003: No Premature Crystallization

**Traces to**: ADRS CR-005
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ high-weight candidate committed with stability < stability_min)`

**proptest strategy**: Generate harvest sessions with candidates of varying weight and
stability. Verify that no high-weight candidate bypasses the crystallization guard.

---

