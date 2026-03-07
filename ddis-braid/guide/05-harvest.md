# §5. HARVEST — Build Plan

> **Spec reference**: [spec/05-harvest.md](../spec/05-harvest.md) — read FIRST
> **Stage 0 elements**: INV-HARVEST-001–003, 005, 007 (5 INV), ADR-HARVEST-001–004, NEG-HARVEST-001–003
> **Later stages**: INV-HARVEST-004 (Stage 1), 006 (Stage 1), 008 (Stage 2), 009 (Stage 2)
> **Dependencies**: STORE (§1), SCHEMA (§2), QUERY (§3), RESOLUTION (§4)
> **Cognitive mode**: Information-theoretic — epistemic gaps, information gain, pipeline

---

## §5.1 Module Structure

```
braid-kernel/src/
└── harvest.rs    ← HarvestCandidate, HarvestPipeline, gap detection, quality metrics
```

### Public API Surface

```rust
/// A candidate for harvest: knowledge to potentially transact.
/// All fields match spec/05-harvest.md §5.3 Level 2 (R6.7b alignment).
pub struct HarvestCandidate {
    pub id:                  usize,                  // Index for accept/reject referencing in CLI
    pub datom_spec:          Vec<Datom>,
    pub category:            HarvestCategory,
    pub confidence:          f64,                     // 0.0-1.0
    pub weight:              f64,                     // Estimated commitment weight
    pub status:              CandidateStatus,         // Lattice: proposed < under-review < committed < rejected
    pub extraction_context:  String,                  // Why this was extracted
    pub reconciliation_type: ReconciliationType,      // Traces to reconciliation taxonomy (spec section 15)
}

/// Candidate status lattice (spec/05-harvest.md §5.3).
/// Follows the spec's four-step lattice: proposed < under-review < committed < rejected.
pub enum CandidateStatus {
    Proposed,               // Initial state after detection
    UnderReview,            // Being reviewed by topology
    Committed,              // Approved and transacted
    Rejected(String),       // Rejected with reason
}

pub enum HarvestCategory {
    Observation,   // Something noticed but not transacted
    Decision,      // A choice made but not recorded as ADR
    Dependency,    // A link discovered but not formalized
    Uncertainty,   // Something uncertain but not marked
}

pub enum ReconciliationType {
    Epistemic,      // Store vs. agent knowledge
    Structural,     // Implementation vs. spec
    Consequential,  // Current state vs. future risk
}

/// Run the harvest pipeline on a store + session context.
pub fn harvest_pipeline(
    store: &Store,
    session_context: &SessionContext,
) -> HarvestResult;

pub struct HarvestResult {
    pub candidates: Vec<HarvestCandidate>,
    pub drift_score: f64,          // |Δ(t)| at session end
    pub quality: HarvestQuality,
}

pub struct HarvestQuality {
    pub candidate_count:  usize,
    pub high_confidence:  usize,   // confidence > 0.8
    pub medium_confidence: usize,  // 0.5–0.8
    pub low_confidence:   usize,   // < 0.5
}

/// Accept a candidate: produce the transaction to commit it.
pub fn accept_candidate(
    candidate: &HarvestCandidate,
    agent: AgentId,
) -> Transaction<Building>;

/// Record harvest session metadata.
pub fn harvest_session_entity(
    result: &HarvestResult,
    committed: &[usize],  // IDs of accepted candidates
    rejected: &[usize],   // IDs of rejected candidates
    agent: AgentId,
) -> Transaction<Building>;

pub struct SessionContext {
    pub agent:              AgentId,
    pub session_start_tx:   TxId,
    pub recent_transactions: Vec<TxId>,
    pub task_description:   String,
}

/// Micro-transaction annotation from continuous externalization (INV-HARVEST-009).
/// These are structured hints that feed into the harvest pipeline as pre-candidates.
/// Stage 2+: generated via externalization obligations in dynamic CLAUDE.md.
pub struct ExternalizationAnnotation {
    pub category:    HarvestCategory,       // observation | decision | dependency | uncertainty
    pub description: String,                // what was learned
    pub response_id: usize,                 // which response produced this
    pub confidence:  f64,                   // self-assessed confidence (0.0-1.0)
}

/// Ingest externalization annotations into harvest pipeline (INV-HARVEST-009).
/// Filters annotations already in store, boosts confidence of remaining to ≥ 0.7.
pub fn ingest_annotations(
    store: &Store,
    annotations: &[ExternalizationAnnotation],
) -> Vec<HarvestCandidate>;

/// Harvest session entity — records metadata for a completed harvest (INV-HARVEST-002).
/// Created by harvest_session_entity() and transacted into the store.
pub struct HarvestSession {
    pub session_id:       EntityId,
    pub agent:            AgentId,
    pub review_topology:  ReviewTopology,
    pub candidates:       Vec<HarvestCandidate>,
    pub drift_score:      u32,           // count of uncommitted observations at harvest time
    pub timestamp:        Instant,
}

/// Review topology for harvest candidates (INV-HARVEST-008).
/// Stage 0: only SelfReview is used. Other variants defined for forward compatibility.
pub enum ReviewTopology {
    SelfReview,                                    // Stage 0: single agent reviews own work
    PeerReview { reviewer: AgentId },              // Stage 2: bilateral peer review
    SwarmVote { quorum: u32 },                     // Stage 3: multi-agent voting
    HierarchicalDelegation { specialist: AgentId },// Stage 3: route to domain expert
    HumanReview,                                   // Stage 2: human in the loop
}
```

---

## §5.1a Harvest Detection Epistemology (from D4-harvest-epistemology.md)

The formal model defines `Delta(t) = K_agent(t) \ K_store(t)` as the epistemic gap.
The operational challenge is that **K_agent(t) is not directly observable**.

### What K_agent(t) Contains (Five Categories)

| Category | Observable? | Source | Detection Method |
|----------|------------|--------|-----------------|
| 1. Seed knowledge | YES (fully) | Loaded at session start from store | Known by construction |
| 2. Transaction knowledge | YES (fully) | Explicit `braid transact` calls | In K_store by definition |
| 3. Query knowledge | YES (fully) | `braid query` results | Projections of K_store |
| 4. Tool output knowledge | PARTIAL | Non-braid tool calls (bash, file reads, web) | Tool call log matching |
| 5. Reasoning knowledge | NO | Implicit conclusions, trade-off assessments | LLM-assisted only |

**The epistemic gap lives primarily in categories 4 and 5.** Category 4 is the highest-value
target for mechanical detection. Category 5 requires the agent to self-report its knowledge.

### Stage 0 Detection: Two-Layer Architecture

**Layer 1 — Tool Call Log Matcher (mechanical)**:
```
1. Read session context (agent ID, task description, start TX)
2. Query store for all transactions since session start
3. Query tool call log for all tool calls since session start
4. For each tool call result:
   a. Check if a corresponding observation datom exists in store
   b. If not: generate a harvest candidate with category=Observation
```

Mechanical signals with detection confidence:

| Signal | Detection Method | Confidence |
|--------|-----------------|------------|
| File read without observation datom | Compare tool log to store | High |
| Error encountered without uncertainty datom | Check tool exit codes | High |
| Test run without result datom | Check test tool calls | High |
| Dependency discovered without link | Compare entity references | Medium |
| Decision language in conversation | Parse for decision patterns | Medium |

**Layer 2 — LLM-Assisted Detection (semi-automated)**:

Steps 1-4 above are mechanical. Steps 5-7 require the agent:
```
5. Present mechanical candidates to the agent for categorization and refinement
6. Agent adds reasoning-derived candidates (decisions, uncertainties)
7. Agent reviews and accepts/rejects each candidate
```

### The Harvest Prompt Template

To structure LLM-assisted detection and reduce false negatives, `braid harvest` presents
structured questions:

- "What files did you read that aren't recorded as observations?"
- "What decisions did you make that aren't recorded as ADRs?"
- "What uncertainties did you discover?"
- "What dependencies exist that aren't linked?"

This template activates specific knowledge categories rather than asking the open-ended
"what do you know?" which is subject to the same attention degradation harvest counteracts.

### Recursive Epistemic Uncertainty

The agent does not know what it does not know. An empty harvest (0 candidates) may mean
either (a) all knowledge is transacted, or (b) detection missed gaps. The **drift score
is the key cross-check**: if drift_score > 0 but candidates = 0, the detector is broken.
Over time, FP/FN tracking (INV-HARVEST-004, Stage 1) calibrates detection thresholds.

### Verification Against spec/05-harvest.md

The D4 analysis confirms alignment with the spec's formal model:

- **Epistemic gap definition** (spec section 5.1 Level 0): mathematically sound, correctly
  formulated as set difference. D4 confirms no corrections needed.
- **Quality metrics** (spec section 5.1 Level 0): FP/FN rates and drift_score are the right
  diagnostics. D4 adds that **data collection should start at Stage 0** even though
  INV-HARVEST-004 (FP/FN calibration) is Stage 1.
- **Pipeline stages** (spec section 5.2 Level 1): Five-stage pipeline (DETECT/PROPOSE/REVIEW/
  COMMIT/RECORD) is correctly structured. D4 clarifies that DETECT is two-layered at Stage 0.
- **Bounded conversation lifecycle** (INV-HARVEST-007): Forces regular harvests, preventing
  unbounded epistemic gap accumulation. Confirmed as essential.

### Category 5 Knowledge Capture Strategy

Category 5 knowledge (reasoning conclusions, trade-off assessments, confidence levels) is
**not directly observable** — it exists only in the agent's internal reasoning, never surfaced
through tool calls or transactions. Rather than improving heuristic detection (which hits a
fundamental ceiling), the architectural approach is to **shrink Category 5 by converting it
to other categories in real time**.

#### The Three Mechanisms

**Mechanism 1: Structured Harvest Prompt (Stage 0)**

The harvest prompt template (above) asks targeted questions that activate specific knowledge
categories: "What files did you read?", "What decisions did you make?", "What uncertainties
did you discover?". This converts some Category 5 → Category 2 by prompting the agent to
articulate implicit knowledge while attention is still partially available.

**Limitation**: At session end, attention budget is degraded — the agent cannot reliably
self-report knowledge it doesn't know it has. Capture rate: ~60-70%.

**Mechanism 2: Continuous Externalization Protocol (Stage 1-2, INV-HARVEST-009)**

Instead of detecting Category 5 knowledge after the fact, the dynamic CLAUDE.md
(INV-GUIDANCE-007) injects **externalization obligations** that prompt the agent to annotate
every response with micro-transaction markers at the moment of discovery:

```
↳ Learned: [decision] Chose redb over rocksdb for single-process constraint
↳ Learned: [observation] BTreeSet ordering matches INV-STORE-003 requirements
↳ Learned: [uncertainty] HLC clock accuracy under high write load untested
```

Key insight: externalize while attention is *high* (during the response that produced the
conclusion), not at session end when attention is *degraded*. These annotations are NOT
auto-committed — they feed into the harvest pipeline as pre-candidates with boosted
confidence (≥ 0.7 floor).

**Mechanism 3: Fresh-Agent Category 5 Review (Stage 2, ADR-HARVEST-004)**

The depleted agent proposes harvest candidates. A **fresh agent** with full attention budget
reviews the conversation transcript (not just the tool call log) and specifically hunts for
Category 5 residuals:

1. Implicit conclusions not captured by externalization annotations
2. Trade-off assessments where the reasoning wasn't recorded
3. Confidence levels expressed in conversation but not as uncertainty datoms
4. Behavioral changes suggesting the agent learned something unrecorded

This exploits **maximum context asymmetry**: the fresh agent sees the transcript with full
attention and no knowledge of what "should" have been externalized, making it an effective
auditor of the depleted agent's blind spots.

#### Stage-by-Stage Capture Trajectory

| Stage | Category 5 Capture | Mechanisms Active |
|-------|-------------------|-------------------|
| 0 | ~60-70% | Structured harvest prompt only |
| 1 | ~80-85% | + Continuous externalization via CLAUDE.md |
| 2 | ~90-95% | + Fresh-agent review targeting Category 5 |
| 3+ | ~95-98% | + Cross-agent externalization, transcript analysis, FP/FN learning |

The ~2-5% residual at convergence represents the **rate-distortion limit**: genuinely tacit
knowledge that the agent cannot articulate regardless of prompting strategy. This is not a
failure of the system — it is the theoretical floor.

#### Transcript Analysis as Detection Source

The harvest pipeline's Layer 2 (LLM-assisted detection) should analyze the **conversation
transcript**, not just the tool call log. Tool call logs capture *what the agent did*;
conversation transcripts capture *what the agent concluded*. At Stage 0, this is implicit
in the structured harvest prompt (the agent reviews its own reasoning). At Stage 2+, the
fresh-agent reviewer explicitly analyzes the transcript for un-externalized knowledge.

---

## §5.2 Three-Box Decomposition

### HarvestPipeline

**Black box** (contract):
- INV-HARVEST-001: Harvest Monotonicity — harvest only adds datoms, never removes (C1).
- INV-HARVEST-002: Harvest Provenance Trail — every harvest creates a HarvestSession entity
  with provenance linking committed candidates to the session.
- INV-HARVEST-003: Drift Score Recording — drift_score (count of uncommitted observations)
  stored as a datom on the HarvestSession entity.
- INV-HARVEST-004: FP/FN Calibration — false positive and false negative rates are tracked
  for threshold adjustment (Stage 1).
- INV-HARVEST-005: Proactive Warning — Q(t) triggers are monitored; harvest warnings at
  Q(t) < 0.15, harvest-only imperative at Q(t) < 0.05.
- INV-HARVEST-006: Crystallization Guard — high-weight candidates require stability check
  before commitment (Stage 1).
- INV-HARVEST-007: Bounded Conversation Lifecycle — SEED → work → HARVEST → end cycle;
  conversations are bounded reasoning trajectories.
- INV-HARVEST-008: Delegation Topology Support — harvest review topology selected by
  commitment weight (self/peer/swarm/hierarchical/human) (Stage 2).
- INV-HARVEST-009: Continuous Externalization Protocol — agents annotate responses with
  micro-transaction markers; annotations become pre-candidates with boosted confidence (Stage 2).

**State box** (internal design):
- Pipeline is a pure function: `(Store, SessionContext) → HarvestResult`.
- No mutation during detect/propose — candidates are proposals, not facts.
- Mutation happens only at commit stage (via `Store::transact`).
- Drift score computed from gap analysis: count of session observations not yet in store.

**Clear box** (implementation):
- **DETECT**: Compare session transactions against store state. For each tx in session,
  check: are all implied observations transacted? Are decisions recorded as ADR entities?
  Are discovered dependencies linked? Are uncertainties marked?
  For Stage 0: detection is LLM-assisted. The harvest command presents the session's
  transaction log and asks the agent to identify gaps. As the system matures, detection
  becomes increasingly automated.
- **PROPOSE**: Each detected gap → HarvestCandidate. Confidence based on:
  - Explicitly stated decision → 0.9+
  - Implicit observation (inferred from behavior) → 0.5–0.7
  - Dependency suggested by co-occurrence → 0.3–0.5
- **REVIEW**: Stage 0 = single-agent self-review. Present candidates to agent for accept/reject.
- **COMMIT**: Accepted candidates → `Transaction<Building>` → commit → transact.
- **RECORD**: Create harvest session entity with metadata → transact.

---

## §5.3 LLM-Facing Outputs

### Agent-Mode Output — `braid harvest`

```
[HARVEST] 7 candidates detected (4 high, 2 medium, 1 low confidence). Drift: 3.2.

  1. [0.95] DECISION: ADR on redb table layout (Epistemic)
     Datoms: {:spec/type "adr", :spec/id "ADR-IMPL-001", :adr/decision "redb over rocksdb"}
     Accept? [Y/n]

  2. [0.82] OBSERVATION: BTreeSet ordering verified correct (Epistemic)
     Datoms: {:test/result "pass", :test/covers "INV-STORE-003"}
     Accept? [Y/n]

  3. [0.45] UNCERTAINTY: HLC clock accuracy under load (Consequential)
     Datoms: {:uncertainty/id "UNC-IMPL-001", :uncertainty/confidence 0.7}
     Accept? [Y/n]
---
↳ Harvest quality: 4 high-confidence candidates. Commit these first.
  Low-confidence candidates may indicate incomplete understanding — investigate before accepting.
```

### Error Messages

- **Empty harvest**: `[HARVEST] 0 candidates. Either all knowledge is already transacted (ideal) or detection missed gaps. Run `braid status` to check drift score. See: INV-HARVEST-003`
- **Session context missing**: `Harvest error: no session context — run `braid seed --task "..."` at session start — See: INV-HARVEST-007`

---

## §5.4 Verification

### Key Properties

```rust
proptest! {
    // INV-HARVEST-001: Harvest Monotonicity (harvest only adds, never removes)
    fn inv_harvest_001(store in arb_store(5), context in arb_session_context()) {
        let pre_count = store.len();
        let result = harvest_pipeline(&store, &context);
        // Harvest result does not reduce store size
        prop_assert!(store.len() >= pre_count);
    }

    // INV-HARVEST-001 (commit path): No data loss on commit
    fn inv_harvest_001_commit(store in arb_store(5), candidate in arb_harvest_candidate()) {
        let mut store = store;
        let tx = accept_candidate(&candidate, AgentId::test());
        let committed = tx.commit(&store.schema()).unwrap();
        let pre_datoms = store.len();
        store.transact(committed).unwrap();
        prop_assert!(store.len() > pre_datoms);
        // All candidate datoms present
        for d in &candidate.datom_spec {
            prop_assert!(store.contains(d));
        }
    }

    // INV-HARVEST-002: Provenance Trail — harvest session entity created
    fn inv_harvest_002(store in arb_store(5), context in arb_session_context()) {
        let mut s = store;
        let result = harvest_pipeline(&s, &context);
        let accepted: Vec<_> = result.candidates.iter().map(|c| c.id).collect();
        let tx = harvest_session_entity(&result, &accepted, &[], context.agent);
        let committed = tx.commit(&s.schema()).unwrap();
        s.transact(committed).unwrap();
        // A HarvestSession entity exists with provenance
        let sessions = s.query_by_type(":harvest/session");
        prop_assert!(!sessions.is_empty());
    }

    // INV-HARVEST-003: Drift Score Recording — drift_score stored as datom
    fn inv_harvest_003(store in arb_store(5), context in arb_session_context()) {
        let mut s = store;
        let result = harvest_pipeline(&s, &context);
        let tx = harvest_session_entity(&result, &[], &[], context.agent);
        let committed = tx.commit(&s.schema()).unwrap();
        s.transact(committed).unwrap();
        // Drift score is a datom on the session entity
        let sessions = s.query_by_type(":harvest/session");
        for session in sessions {
            let drift = s.entity_attr(session, ":harvest/drift-score");
            prop_assert!(drift.is_some());
        }
    }

    // INV-HARVEST-007: Proactive Harvest Schedule — harvest frequency scales with delta
    fn inv_harvest_007(
        delta_size in 1usize..100,
        session_turns in 1usize..50,
    ) {
        let schedule = compute_harvest_schedule(delta_size, session_turns);
        // Higher delta accumulation → shorter intervals between harvest warnings
        let schedule_tight = compute_harvest_schedule(delta_size * 2, session_turns);
        prop_assert!(schedule_tight.interval <= schedule.interval);
        // Harvest is always recommended before the delta would exceed the budget
        prop_assert!(schedule.next_harvest_turn <= session_turns);
    }

    // INV-HARVEST-005: Proactive Warning — Q(t) < 0.15 triggers warning
    fn inv_harvest_005(q_t in 0.0..0.2f64) {
        let should_warn = q_t < 0.15;
        let should_imperative = q_t < 0.05;
        let warning = harvest_warning(q_t);
        if should_imperative {
            prop_assert!(warning.is_some());
            prop_assert!(warning.unwrap().severity == WarningSeverity::Imperative);
        } else if should_warn {
            prop_assert!(warning.is_some());
        }
    }
}
```

---

## §5.5 Implementation Checklist

- [ ] `HarvestCandidate`, `HarvestCategory`, `CandidateStatus`, `ReconciliationType` types defined
- [ ] `harvest_pipeline()` implements five-stage pipeline
- [ ] Epistemic gap detection identifies untransacted knowledge
- [ ] Candidate confidence scoring
- [ ] `accept_candidate()` produces valid transaction
- [ ] `harvest_session_entity()` records metadata
- [ ] Quality metrics (FP/FN tracking) computed
- [ ] Drift score computation
- [ ] Integration with STORE: committed candidates become permanent datoms
- [ ] `ExternalizationAnnotation` type defined (INV-HARVEST-009)
- [ ] `ingest_annotations()` converts annotations to boosted candidates
- [ ] All proptest properties pass

---
