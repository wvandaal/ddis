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

