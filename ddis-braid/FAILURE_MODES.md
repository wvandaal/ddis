# FAILURE_MODES.md — Agentic Failure Mode Catalog

> **Purpose**: A live catalog of failure modes observed during the DDIS/Braid ideation,
> specification, and implementation process. Each entry documents a real failure that occurred
> when using AI agents for complex, long-running work — including cases where agents deviated
> from user intentions, lost knowledge across session boundaries, or produced subtly wrong
> artifacts.
>
> **Why this exists**: These failure modes serve as **test cases and acceptance criteria** for
> evaluating DDIS and Braid. For each entry, the question is: *Does the DDIS/Braid methodology
> (as designed in SEED.md) have a mechanism that would prevent or detect this failure?* If yes,
> the entry becomes a verification target with a measurable SLA. If no, it represents a design
> gap that the methodology must address.
>
> **What this is NOT**: This is not a task tracker for ad-hoc manual fixes. Marking an FM as
> "resolved" because a human manually patched a document misses the point. The only resolutions
> that matter are mechanisms incorporated into the DDIS/Braid methodology itself.

---

## How to Use This Document

### Recording a Failure Mode

When you discover a failure mode during any session:

1. **Assign an ID**: `FM-NNN` (sequential, never reused)
2. **Classify** the divergence type using the reconciliation taxonomy (SEED.md §6)
3. **Document** what happened, why it matters, and the root cause
4. **Map** to DDIS/Braid mechanisms: which part of the design addresses this class of failure?
5. **Define** an acceptance criterion: how would you verify the mechanism works? What SLA?

### Triage Protocol

At the start of any session:

1. Read open failure modes — check whether your current work might trigger the same class of failure
2. If you observe a new instance of an existing FM, add it as evidence (strengthens the test case)
3. If you discover a new FM, add it immediately
4. If you realize a DDIS/Braid mechanism doesn't cover an FM, flag it as UNMAPPED (design gap)

### Lifecycle

```
OBSERVED → MAPPED → TESTABLE → VERIFIED
                  → UNMAPPED (design gap — methodology must be extended)
```

- **OBSERVED**: Failure occurred and is documented, but not yet analyzed against DDIS mechanisms
- **MAPPED**: A DDIS/Braid mechanism has been identified that should address this failure class
- **TESTABLE**: An acceptance criterion with measurable SLA has been defined
- **VERIFIED**: The mechanism has been tested against an implementation and the SLA is met
- **UNMAPPED**: No DDIS/Braid mechanism covers this failure class — represents a design gap

### Severity Levels

| Level | Meaning |
|-------|---------|
| **S0 — Structural** | The failure produces artifacts that are silently wrong. Downstream work inherits the error with no detection signal. |
| **S1 — Methodological** | The methodology itself failed to fire — a mechanism that should have caught the problem didn't. |
| **S2 — Operational** | A process step was skipped or executed incorrectly. The mechanism exists but wasn't followed. |
| **S3 — Cosmetic** | Surface-level inconsistency with no impact on correctness. |

### Divergence Types (from SEED.md §6 Reconciliation Taxonomy)

| Code | Type | Boundary | DDIS Detection Mechanism |
|------|------|----------|--------------------------|
| **EP** | Epistemic | Store vs. agent knowledge | Harvest gap detection |
| **ST** | Structural | Implementation vs. spec | Bilateral scan / drift |
| **CO** | Consequential | Current state vs. future risk | Uncertainty tensor |
| **AL** | Aleatory | Agent vs. agent | Merge conflict detection |
| **LO** | Logical | Invariant vs. invariant | Contradiction detection |
| **AX** | Axiological | Implementation vs. goals | Fitness function, goal-drift |
| **TE** | Temporal | Agent frontier vs. agent frontier | Frontier comparison |
| **PR** | Procedural | Agent behavior vs. methodology | Drift detection |

---

## Failure Mode Registry

| ID | Status | Severity | Div. Type | Failure Class | DDIS/Braid Mechanism | Acceptance Criterion |
|----|--------|----------|-----------|---------------|----------------------|----------------------|
| FM-001 | TESTABLE | S0 | EP | **Knowledge loss across session boundaries.** Confirmed design decisions are silently dropped when summarizing long conversations into durable documents. | Harvest gap detection (§5): measures delta between agent knowledge and store knowledge. FP/FN calibration (LM-006) tracks extraction quality. | Harvest detects ≥99% of confirmed decisions made during a session. Measured by: inject N known decisions, harvest, verify N appear in store. |
| FM-002 | TESTABLE | S1 | EP | **Provenance fabrication.** Agent uses a concept and attributes it to a plausible but incorrect source, or labels it with a term not used in the original discussion. | Causal traceability (§4): every datom records provenance (who, when, why, causal predecessors). Constraint C5 (traceability): every artifact traces to a specification element. | Every assertion in the store traces to a verifiable source. Query: `?datom :provenance/source ?src` returns a real, checkable reference for 100% of datoms. No fabricated attributions survive harvest. |
| FM-003 | MAPPED | S2 | PR | **Anchoring bias in analysis scope.** When analyzing a codebase against a specification, the agent anchors on one document (SEED.md) and misses decisions recorded elsewhere (transcripts, ADRs). 46% miss rate observed. | The datom store eliminates the "compressed document" failure mode: all decisions are datoms, queryable from a single substrate. Associate/Assemble (§6) surfaces relevant prior knowledge from the full store, not from a single document. | Gap analysis against datom store achieves ≥95% coverage of confirmed decisions. Measured by: compare analysis output against known decision set. |
| FM-004 | TESTABLE | S0 | ST | **Cascading incompleteness.** A foundational document (SEED.md) is missing confirmed decisions. Everything downstream — spec, implementation guide, code — silently inherits the gaps. No detection signal until a human notices. | Bilateral loop (§6): continuously checks alignment between spec and implementation in both directions. Fitness function (§3): quantifies coverage (every goal traces to invariants and back). Harvest (§5): ensures session-produced knowledge enters the store. | F(S) coverage component detects ≥99% of spec gaps within one bilateral cycle. Measured by: introduce a known gap, run bilateral scan, verify detection. |

---

## Failure Mode Detail

### FM-001: Knowledge Loss Across Session Boundaries

**Discovered**: Session 002 (2026-03-02)
**Trigger**: User asked about "private datoms" — a design decision confirmed in Transcript 04
(PQ1, Option B, line 373) and elaborated into a full tool spec in Transcript 05 (lines 849–861).
The decision was entirely absent from SEED.md, GAP_ANALYSIS.md, and the Session 001 harvest entry.

**What happened**: During 7 design sessions (~50,000 words), the agent produced SEED.md as the
durable artifact. The manual harvest (HARVEST.md Session 001) captured high-level abstractions
but missed a confirmed, architecturally significant protocol decision (W_α working set / patch
branches). The agent that produced the SEED.md summarized at the abstraction level, not the
decision level. The compression ratio (~10:1) guaranteed information loss, but there was no
mechanism to detect *which* information was lost.

**Why this is a hard problem for agents**: LLM agents have no durable memory across sessions.
When a conversation ends, everything not explicitly externalized is gone. The harvest step
(writing a summary) is itself an LLM generation — subject to the same attention degradation,
recency bias, and abstraction-level mismatch that produced the gap in the first place. The
agent doesn't know what it forgot, because forgetting is invisible to the forgetter.

**Root cause**: No automated mechanism to compare "what the agent knew" against "what the agent
externalized." The harvest was a manual, best-effort summary — not a gap-detecting operation.

**DDIS/Braid mechanism**: The harvest operation (SEED.md §5) is designed to detect exactly this.
It measures the delta between the agent's working knowledge (datoms asserted during the session)
and the store (datoms that were actually transacted). The harvest FP/FN calibration metric
(LM-006) tracks extraction quality over time, enabling systematic improvement. Semi-automated
harvest (LM-005) presents candidates for confirmation rather than relying on the agent's
summary generation.

**Acceptance criterion**: Given a session where N decisions are made, the harvest mechanism
detects and extracts ≥99% of them into the store. Measured by: seed a session with known
decisions (some explicit, some implicit in code changes), harvest, compare store contents
against ground truth. Repeated across 100 sessions, the aggregate FN rate must be <1%.

**Observations**:
- Session 002: 4 confirmed decisions from a single transcript were missing (W_α, provenance lattice, crash-recovery, delivery semantics)
- Session 004/005: Systematic audit revealed 59 HIGH-priority items across 125 ADRs were absent from SEED.md (47% miss rate on the manual harvest)

---

### FM-002: Provenance Fabrication

**Discovered**: Session 002 (2026-03-02)
**Trigger**: User challenged "where does frontier tracking come from?" after reading the gap
analysis.

**What happened**: The gap analysis used the term "frontier tracking" and attributed it as a
concept from the design sessions. When the user asked for the exact source, investigation
revealed the concept originated from Transcript 01 (lines 328–337) where Claude introduced it
as a consequence of multi-writer partial orders. The term "frontier tracking" was the gap
analyst's label — not a term used in the original discussion. The attribution was
directionally correct (the concept does originate from Transcript 01) but the label was
fabricated by the agent, not sourced from the transcripts.

**Why this is a hard problem for agents**: LLM agents interpolate freely between source material
and their own generation. When summarizing or analyzing, they introduce their own terminology
and framing without flagging which parts are sourced and which are generated. This is invisible
to the reader — everything reads with equal confidence. In a system where provenance matters
(DDIS), this is a corruption vector.

**Root cause**: No structural separation between "fact sourced from document X at line Y" and
"agent's synthesis/label for a cluster of facts." The gap analysis format treated both as
equivalent prose.

**DDIS/Braid mechanism**: Every datom carries provenance as a structural property (§4, Axiom 1).
The transaction entity records who asserted the fact, when, with what causal predecessors. The
provenance typing lattice (PD-002: `:observed < :derived < :inferred < :hypothesized`) would
distinguish a directly-sourced fact from an agent's synthesis. Constraint C5 (traceability)
requires every artifact to trace to a specification element.

**Acceptance criterion**: Every datom in the store has a verifiable provenance chain. Agent-
introduced terminology is tagged with provenance type `:derived` or `:inferred`, never
`:observed`. Query: "show me all `:observed` datoms whose provenance chain doesn't terminate
at a verifiable external source" returns empty set.

---

### FM-003: Anchoring Bias in Analysis Scope

**Discovered**: Session 002 (2026-03-02)
**Trigger**: FM-001 investigation revealed the gap analysis methodology (GAP_ANALYSIS.md §1)
defined its scope as "comprehensive analysis of `../ddis-cli/` against SEED.md §1–§11" — but
SEED.md was an incomplete capture of all design decisions.

**What happened**: The gap analysis anchored on SEED.md as the sole specification source. Design
decisions confirmed in transcripts but not propagated to SEED.md were invisible. Session 004
quantified this: 58 of 125 ADRs (46%) were not covered by the gap analysis. The miss rate
was not a fluke — it was structural, caused by the 10:1 compression ratio from transcripts to
SEED.md.

**Why this is a hard problem for agents**: Agents follow their instructions literally. If told
"analyze against SEED.md," they analyze against SEED.md — they don't independently identify
that SEED.md might be incomplete and seek additional sources. The anchoring bias compounds:
each downstream artifact (gap analysis, spec, implementation) inherits and amplifies the
original document's gaps, with no mechanism to detect the inheritance.

**Root cause**: The specification existed in multiple documents at different levels of
abstraction (transcripts, SEED.md, ADRS.md), with no single authoritative source and no
automated consistency check between them.

**DDIS/Braid mechanism**: The datom store IS the single authoritative source. There are no
"multiple documents at different abstraction levels" — there are datoms. Associate (§6) queries
the full store, not a single document. The bilateral loop checks alignment between spec and
implementation across the full knowledge base. The fitness function's coverage component
detects gaps where goals exist without corresponding invariants.

**Acceptance criterion**: An analysis performed using DDIS tools (associate, query, coverage)
achieves ≥95% coverage of confirmed decisions, compared to the 54% achieved by the manual
SEED.md-anchored approach. Measured by: run both approaches against the same decision set,
compare recall.

---

### FM-004: Cascading Incompleteness

**Discovered**: Session 002 (2026-03-02)
**Trigger**: FM-001 investigation revealed that SEED.md was missing at least 4 confirmed
decisions from Transcript 04 alone. Subsequent audit (Session 004/005) found 59 HIGH-priority
items missing across all transcripts — decisions where an implementing agent would build the
wrong thing.

**What happened**: SEED.md was described as "the seed from which the formal specification, the
implementation, and the system itself will grow." But it was incomplete: 47% of confirmed
design decisions were absent. Every downstream artifact (GAP_ANALYSIS.md, the planned SPEC.md,
the eventual implementation) would silently inherit these gaps. There was no detection signal —
the document read as complete. Only systematic cross-referencing against the transcripts (via
ADRS.md) revealed the gaps.

**Why this is a hard problem for agents**: An incomplete document doesn't *look* incomplete.
It reads fluently, covers the major topics, and passes surface-level review. Agents working
from it produce artifacts that are internally consistent but externally incomplete — they
satisfy the document they were given, but not the full intent behind it. The incompleteness
is invisible until something downstream fails or a human asks the right question.

**Root cause**: No automated check for completeness. The manual harvest process had no
mechanism to verify that every confirmed decision from the source material (transcripts)
appeared in the output (SEED.md). The fitness function's coverage component exists in the
design but was not yet implemented to catch this.

**DDIS/Braid mechanism**: The fitness function (§3) quantifies coverage — every goal must trace
to invariants and back. The bilateral loop (§6) checks both directions: does the implementation
satisfy the spec, AND does the spec accurately describe the implementation? The harvest (§5)
detects the gap between agent knowledge and store knowledge. Together, these three mechanisms
form a closed detection loop: if a decision exists in the transcripts (source), it should
appear as a datom (store), which should trace to an invariant (spec), which should trace to
code (implementation). Any break in this chain is a measurable gap.

**Acceptance criterion**: After a bilateral scan, the fitness function's coverage component
reports ≥99% of confirmed decisions. Measured by: inject N known decisions into the source
material, run the full pipeline (harvest → store → bilateral scan → coverage), verify N
appear in the coverage report. The 47% miss rate observed with the manual process must drop
to <1% with the automated process.

**Observations**:
- Session 002: 4 confirmed decisions from Transcript 04 identified as missing
- Session 004: 58 of 125 ADRs (46%) found absent from GAP_ANALYSIS.md
- Session 005: 59 of 125 ADRs (47%) found to contain HIGH-priority information absent from SEED.md
- The convergence of 46% and 47% across independent analyses suggests this is the true incompleteness rate of manual harvest at ~10:1 compression

---

## Statistics

| Metric | Value |
|--------|-------|
| Total failure modes | 4 |
| OBSERVED | 0 |
| MAPPED | 1 (FM-003) |
| TESTABLE | 3 (FM-001, FM-002, FM-004) |
| VERIFIED | 0 |
| UNMAPPED (design gaps) | 0 |
| S0 (Structural) | 2 (FM-001, FM-004) |
| S1 (Methodological) | 1 (FM-002) |
| S2 (Operational) | 1 (FM-003) |

### Coverage Summary

All 4 observed failure modes map to DDIS/Braid mechanisms. No design gaps identified yet.
This is expected at the current stage (specification, pre-implementation) — design gaps are
more likely to surface during implementation and real-world usage. The true test is whether
the mechanisms achieve their stated SLAs when implemented.

| FM | Mechanism | Target SLA | Current Manual Rate |
|----|-----------|------------|---------------------|
| FM-001 | Harvest gap detection + FP/FN calibration | ≥99% decision capture | ~53% (47% miss) |
| FM-002 | Provenance typing lattice + traceability constraint | 100% verifiable provenance | Unknown (1 observed fabrication) |
| FM-003 | Single-substrate store + Associate/Assemble | ≥95% analysis coverage | ~54% (46% miss) |
| FM-004 | Fitness function coverage + bilateral loop | ≥99% completeness detection | ~53% (47% miss) |
