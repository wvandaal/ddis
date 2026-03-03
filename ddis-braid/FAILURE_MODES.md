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
| FM-005 | TESTABLE | S0 | ST | **Semantic ID collision (phantom alignment).** Two documents reference the same invariant ID but mean different things. ID-level grep shows alignment; only semantic comparison reveals the mismatch. 26 of 39 Stage 0 guide INV references map to wrong spec content. | Bilateral scan (§6): compares not just ID presence but semantic content. Datom identity (INV-STORE-002, C2): content-addressed identity means the same ID always maps to the same content — renumbering produces a new entity, not a silent mutation. | Bilateral scan detects 100% of ID↔content mismatches between any two documents. Measured by: introduce 5 phantom-aligned IDs (same ID, different content), run bilateral scan, verify all 5 flagged. |
| FM-006 | TESTABLE | S0 | ST | **Undetected cross-document drift.** A source document evolves (new elements added, IDs renumbered, counts changed) but dependent documents are not updated. No mechanism flags the inconsistency. Observed: 3 new INVs added to spec/, 0 of 13 guide files updated. | Drift detection (§7, INV-GUIDANCE-004): monitors for structural divergence between spec and implementation artifacts. Frontier comparison (§6): each document's "frontier" (latest known state) is tracked; stale frontiers trigger sync signals. | Adding an INV to spec/ triggers a drift signal within 1 bilateral cycle for every dependent document. Measured by: add INV-TEST-001 to spec, run drift scan, verify all guide files referencing that namespace are flagged as stale. |
| FM-007 | TESTABLE | S0 | EP+ST | **Decision-layer propagation failure.** A design decision is confirmed at one layer (transcripts, guides) but contradicted at another (spec). The contradiction is undetected because no mechanism checks cross-layer consistency. Observed: hash algorithm = BLAKE3 in 8+ guide files but SHA-256 in spec/01-store.md. | Bilateral loop (§6): checks spec↔implementation AND spec↔design-decisions in both directions. ADR-as-data (C3, §4): every ADR is a datom with content-addressed identity — querying "what hash algorithm?" returns a single authoritative answer from the store, not a document-local claim. | Every design decision appears consistently across all layers. Measured by: query store for ADR-STORE-002 (hash algorithm), verify all documents referencing EntityId hash agree. Cross-layer contradiction count = 0 after bilateral scan. |
| FM-008 | TESTABLE | S0 | ST | **Derived quantity staleness.** Manually-maintained counts, percentages, and activation lists appear in multiple documents. When the source changes, derived quantities go stale with no detection signal. Observed: "62 Stage 0 INVs" in 3 files after actual count changed to 64; "104 total" in 2 files after actual count changed to 107. | Query-derived metrics (§4, INV-QUERY-001): counts and derived statistics should be Datalog queries over the store, not hardcoded numbers in prose. The store is the single source of truth; documents are projections. Drift detection (§7): any derived value that disagrees with a store query is flagged. | Zero hardcoded counts in any document. "How many Stage 0 INVs?" is answered by `[:find (count ?e) :where [?e :inv/stage 0]]`, not by a prose number. Measured by: change an INV's stage assignment, verify all projected counts update automatically. |
| FM-009 | TESTABLE | S1 | AX | **Silent ADR contradiction.** A settled design decision (formalized as an ADR) is contradicted by a downstream artifact, with no detection signal. The artifact author may be a different agent that never loaded the ADR. Observed: ADR-GUIDANCE-001 rejected flat rules in favor of comonadic topology; guide/08-guidance.md implements flat rules. | ADR traceability (C5, §4): every implementation artifact traces to the ADR that governs it. Contradiction detection (§6, 5-tier): ADR assertions are datoms; implementing a contradicted alternative triggers a logical contradiction signal. Seed (§5): session startup loads relevant ADRs so agents cannot unknowingly relitigate. | Every guide implementation choice traces to a supporting ADR. Implementing an alternative explicitly rejected by an ADR triggers a contradiction signal. Measured by: assert a datom contradicting ADR-GUIDANCE-001, verify 5-tier contradiction engine detects it within 1 scan cycle. |

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

### FM-005: Semantic ID Collision (Phantom Alignment)

**Discovered**: Session 006 (2026-03-02)
**Trigger**: Deep audit of all 13 guide files against spec/ revealed that invariant IDs in
guides frequently reference the *correct ID* but describe the *wrong invariant*.

**What happened**: The specification was modularized and finalized in spec/ with invariant IDs
assigned to specific content (e.g., INV-SCHEMA-003 = "Schema-as-Data Representation"). The
implementation guides in guide/ were written during or before this finalization, using a
different numbering scheme. When the spec IDs were settled, the guides were not realigned.
The result: guide/02-schema.md references INV-SCHEMA-003 but describes content that actually
corresponds to INV-SCHEMA-005 in the spec. A grep for "INV-SCHEMA-003" finds it in both
files — phantom alignment.

**Scale of the problem**: 14-agent audit found misalignment in 6 of 9 namespace guides:
- SCHEMA: 8/8 INV IDs map to wrong content
- HARVEST: 7/8 INV IDs wrong
- SEED: 3/4 IDs wrong
- RESOLUTION: 4/8 IDs wrong or reassigned
- QUERY: 2/11 IDs swapped
- MERGE: INV-MERGE-008 — guide tests receipt arithmetic, spec requires idempotency

**Why this is a hard problem for agents**: ID-level consistency checks (grep, cross-reference
tables) show alignment. Only a *semantic* comparison — reading what each ID *means* in both
documents — reveals the mismatch. Agents performing surface-level validation ("does the guide
reference INV-SCHEMA-003? yes → aligned") will miss this entirely. The phantom alignment is
invisible to any check that doesn't compare content.

**Root cause**: Documents were produced by different agents in different sessions with no
automated mechanism to verify that an ID in document A means the same thing as the same ID
in document B. The ID is a label, not a content-addressed identifier. Two documents can
agree on labels while disagreeing on content — the classic nominal vs. structural typing
failure.

**DDIS/Braid mechanism**: Content-addressed identity (INV-STORE-002, C2) eliminates this
class of failure at the data layer. A datom's identity IS its content — if the content
changes, the identity changes. There is no way for two datoms to have the "same ID" but
"different content." At the document projection layer, bilateral scan (§6) must compare
not just ID presence but semantic content, flagging any case where a projected document
uses an ID whose projected content diverges from the store's authoritative content.

**Acceptance criterion**: The bilateral scan detects 100% of semantic ID collisions. Measured
by: create two projections of the same store where 5 INV IDs have been swapped (same IDs,
different content assignments). Run bilateral scan. All 5 must be flagged. False negative
rate: 0%.

**Observations**:
- Session 006: 26 of 39 Stage 0 guide INV references semantically misaligned with spec
- The most dangerous variant: INV-MERGE-008. Guide has a proptest *labeled* INV-MERGE-008
  that tests receipt arithmetic correctness. Spec's INV-MERGE-008 requires idempotent delivery
  (`MERGE(MERGE(S,R),R) = MERGE(S,R)`). The test passes — but tests the wrong property.

---

### FM-006: Undetected Cross-Document Drift

**Discovered**: Session 006 (2026-03-02)
**Trigger**: After adding INV-INTERFACE-008, INV-INTERFACE-009, and INV-BUDGET-006 to spec/,
a 14-agent audit found that zero guide files had been updated to reflect the additions.

**What happened**: Three new invariants were added to the specification (spec/14-interface.md,
spec/13-budget.md) and cross-referenced in spec/16-verification.md and spec/17-crossref.md.
The spec's internal cross-references were updated: Appendix A counts, Appendix C Stage 0
elements, verification matrix rows. But no guide file was updated. The guide/09-interface.md
header still says "Stage 0 elements: INV-INTERFACE-001–003 (3 INV)" when the spec now says
5 INV. The guide/10-verification.md says "104 invariants" when the spec says 107.

**Additionally**: The audit discovered that guide/10-verification.md already had stale counts
*before* the new INVs were added — it said "62 Stage 0" when the spec already said 64. This
suggests the drift had been accumulating across multiple edit sessions.

**Why this is a hard problem for agents**: When an agent edits spec/14-interface.md, it has no
way to know which other files reference INTERFACE invariant counts or Stage 0 scope. The
dependency graph between documents is implicit — there is no manifest of "when you add an INV
to INTERFACE, these 7 files need updating." Each edit session has its own context window;
documents outside that window are invisible. The drift accumulates silently.

**Root cause**: No dependency tracking between documents. The relationship "guide/09-interface.md
depends on spec/14-interface.md's invariant list" is not recorded anywhere. There is no
equivalent of a compiler's dependency graph for prose documents.

**DDIS/Braid mechanism**: Drift detection (§7, INV-GUIDANCE-004) monitors for structural
divergence. In the datom store, every projection (document) has a frontier — the latest
transaction it reflects. When the store advances past a projection's frontier, the projection
is stale. The `braid status` command surfaces stale projections. The `braid guidance` system
injects drift warnings into tool responses. The harvest warning system (INV-INTERFACE-007)
proactively alerts agents when store state has changed but dependent artifacts haven't been
updated.

**Acceptance criterion**: Adding a new INV to the store triggers a staleness signal for every
dependent projection within 1 command cycle. Measured by: transact INV-TEST-001 into the
INTERFACE namespace, run `braid status`, verify guide/09-interface.md is flagged as stale
with the specific reason "new INV in INTERFACE namespace not reflected."

**Observations**:
- Session 006: 3 new INVs added to spec, 0/13 guide files updated
- guide/10-verification.md was already 2 edits behind (said 62 when should say 64)
- guide/12-stages-1-4.md had stale percentage (59.6% → 59.8%) and stale activation list
- Total: 17 stale values across 5 guide files after a single spec edit session

---

### FM-007: Decision-Layer Propagation Failure

**Discovered**: Session 006 (2026-03-02)
**Trigger**: Cross-cutting audit found that spec/01-store.md line 196 says "SHA-256" while
guide/00-architecture.md, guide/01-store.md, and all worked examples say "BLAKE3."

**What happened**: The hash algorithm for EntityId computation was discussed in design sessions
(transcripts/01-datomic-rust-crdt-spec-foundation.md mentions BLAKE3 extensively). The
implementation guides were written with BLAKE3 as the settled choice. But when the formal
specification (spec/01-store.md) was written, it used "SHA-256" in the EntityId struct
comment. No one noticed the contradiction because:
1. The spec was written in a different session than the guides
2. No automated check compares the spec's type comments against the guide's type definitions
3. Both "SHA-256" and "BLAKE3" are plausible choices — neither triggers an obvious error
4. No formal ADR exists for the hash algorithm decision, so there is no single authoritative
   source to check against

**Why this is a hard problem for agents**: Design decisions propagate through a document
pipeline: transcripts → seed → spec → guide → code. Each stage is produced by a different
agent (or the same agent in a different session with a different context window). A decision
confirmed at stage N may be contradicted at stage N+1 if the agent at N+1 doesn't load the
full decision context. The contradiction is invisible to any check that operates within a
single document — you must compare *across* documents to find it.

**Root cause**: Design decisions exist as prose in multiple documents rather than as
content-addressed datoms in a single store. The "pipeline" model of document production
(each stage reads the previous stage's output) guarantees that decisions not propagated at
one stage are lost at all subsequent stages. This is FM-004 (cascading incompleteness)
manifesting at the decision level rather than the fact level.

**DDIS/Braid mechanism**: ADR-as-data (C3, §4). When the hash algorithm decision is a datom
(`[ADR-STORE-hash :decision "BLAKE3" :alternatives ["SHA-256", "SHA-3"] :rationale "..."]`),
every projection (spec, guide, code) must be consistent with that datom. The bilateral loop
(§6) checks that spec/01-store.md's EntityId definition agrees with the ADR datom. If not,
it is flagged as drift. The seed (§5) loads relevant ADRs at session start, ensuring every
agent begins with the authoritative decision context.

**Acceptance criterion**: Every design decision exists as exactly one datom in the store. All
document projections are derived from that datom. Cross-layer contradiction count = 0 after
a bilateral scan. Measured by: query `[:find ?decision :where [?e :adr/id "hash-algorithm"]
[?e :adr/decision ?decision]]` — returns exactly one value, and that value matches every
occurrence in spec/, guide/, and src/.

**Observations**:
- Session 006: BLAKE3 appears in 8+ guide files; SHA-256 appears in spec/01-store.md
- No ADR-STORE-002 for hash algorithm exists (the ID is used for EAV vs Relational)
- This is the same class of failure as FM-001 (knowledge loss) but at a different layer:
  the knowledge exists *somewhere*, it just wasn't propagated to all layers

---

### FM-008: Derived Quantity Staleness

**Discovered**: Session 006 (2026-03-02)
**Trigger**: Audit found "62 Stage 0 INVs" in guide/10-verification.md, "64" in
guide/12-stages-1-4.md, "52" in guide/11-worked-examples.md, and "64" in spec/17-crossref.md
— four different values for the same quantity across four documents.

**What happened**: The count "how many Stage 0 invariants?" is a derived quantity — it should
be computable from the authoritative list of invariants and their stage assignments. Instead,
it was manually written as a prose number in multiple documents. When the authoritative list
changed (new INVs added, stage assignments updated), some documents were updated and others
were not. The manual maintenance burden scaled with the number of documents referencing the
count, and no mechanism verified cross-document consistency.

**Additional instances**: "104 total INVs" (should be 107) in guide/10-verification.md.
"42 Kani proofs" (should be 44). "40.4%" (should be 41.1%). "14.4%" (should be 14.0%).
Every percentage that depends on the total count is silently wrong.

**Why this is a hard problem for agents**: Numbers in prose don't have dependency tracking.
When an agent updates spec/17-crossref.md to say "107 INVs," there is no signal that
guide/10-verification.md also says a number that should match. The stale value doesn't
cause an error — it's just a wrong number in a sentence. An agent would need to know the
full dependency graph of derived quantities to update them all, and that graph exists only
in human intuition, not in any computable form.

**Root cause**: Derived quantities are materialized as constants instead of being computed
from the source. The same architectural anti-pattern that motivated database normalization
(don't store derived values, compute them) applies to documents.

**DDIS/Braid mechanism**: Query-derived metrics (§4, INV-QUERY-001). In the datom store,
"how many Stage 0 INVs?" is a Datalog query: `[:find (count ?e) :where [?e :inv/stage 0]]`.
The answer is always current because it's computed from the source of truth, not cached in
prose. Document projections use query results, not hardcoded numbers. The `braid seed`
output computes counts at generation time. The `braid status` command shows current metrics.
No human-maintained count can go stale because no human-maintained count exists.

**Acceptance criterion**: Zero hardcoded counts in any generated document. Every numeric
claim about store contents is a query result. Measured by: change an INV's stage from 0 to 1,
run `braid seed`, verify the Stage 0 count in the generated output decreases by 1 without
any manual edit.

**Observations**:
- Session 006: 17 stale numeric values across 5 guide files
- 4 different values for "Stage 0 INV count" across 4 documents
- Percentages derived from stale counts are also wrong (compounding error)
- guide/12-stages-1-4.md §12.7 activation list counts to 21 INVs for Stage 2 but
  header says "17 additional INVs" — the list and the summary disagree within the same file

---

### FM-009: Silent ADR Contradiction

**Discovered**: Session 006 (2026-03-02)
**Trigger**: Audit of guide/08-guidance.md found it implements flat procedural rules for
guidance selection, contradicting ADR-GUIDANCE-001 which explicitly rejected flat rules
in favor of comonadic topology.

**What happened**: The specification (spec/12-guidance.md §12.5) contains ADR-GUIDANCE-001:
"Comonadic Topology Over Flat Rules." The ADR's "Alternatives Rejected" section explicitly
states that flat priority-ordered rules were considered and rejected because they don't
compose, can't express context-dependent guidance, and create a maintenance burden as the
rule set grows. Despite this, guide/08-guidance.md defines a `DriftSignals` struct and
`DriftPriority` enum that implement exactly the flat-rule approach that the ADR rejected.

**Additionally**: The guide's `AntiDriftMechanism` enum defines 6 variants, but only 2 of 6
correspond to the spec's six named anti-drift mechanisms. The other 4 are non-spec
substitutions (SpecLanguage, BudgetAware, BasinCompetition, ProactiveWarning). This means
an implementing agent following the guide would build the wrong mechanisms.

**Why this is a hard problem for agents**: The guide author (an agent in a different session)
likely never loaded ADR-GUIDANCE-001. The guide was written from the high-level design intent
("guidance should prevent drift") without checking which specific approach was chosen and
which were rejected. The agent made a reasonable engineering choice (flat rules are simpler)
that happened to be the one explicitly rejected by the design process. Without loading the
ADR, the agent had no way to know.

**Root cause**: ADRs capture *rejected alternatives* — the paths not taken. An agent that
doesn't load the relevant ADRs will sometimes independently arrive at a rejected alternative,
because rejected alternatives are often the obvious first choice (they were considered first
for a reason). The ADR exists precisely to prevent this re-derivation, but only if the agent
loads it.

**DDIS/Braid mechanism**: Seed (§5) loads relevant ADRs at session start. When an agent
begins work on the GUIDANCE namespace, `braid seed --task "implement guidance"` includes
ADR-GUIDANCE-001 in the seed output, ensuring the agent knows which alternatives were rejected.
Contradiction detection (§6, 5-tier): if an agent asserts a datom that contradicts an existing
ADR (e.g., asserting "guidance uses flat priority rules" when ADR-GUIDANCE-001 asserts
"guidance uses comonadic topology"), the 5-tier contradiction engine detects the logical
inconsistency. Dynamic CLAUDE.md (INV-GUIDANCE-007): the generated CLAUDE.md includes
namespace-specific ADR references, so even without explicit seed loading, the agent's
instructions reference the governing ADRs.

**Acceptance criterion**: An agent cannot implement a rejected alternative without triggering
a contradiction signal. Measured by: seed a session for GUIDANCE namespace work, attempt to
transact a datom asserting flat-rule guidance, verify the contradiction engine flags it against
ADR-GUIDANCE-001 within 1 transact cycle.

**Observations**:
- Session 006: guide/08-guidance.md contradicts ADR-GUIDANCE-001 (comonadic topology vs. flat rules)
- guide/08-guidance.md contradicts ADR-GUIDANCE-003 (6 mechanisms — only 2 of 6 align)
- guide/08-guidance.md's GuidanceFooter type has different fields than spec/12-guidance.md
- 6 of 7 spec types for the GUIDANCE namespace are completely absent from the guide
- INV-GUIDANCE-007 (Dynamic CLAUDE.md, a Stage 0 invariant) gets 1 line in the guide
  despite having a 70-line three-level specification in the spec

---

## Statistics

| Metric | Value |
|--------|-------|
| Total failure modes | 9 |
| OBSERVED | 0 |
| MAPPED | 1 (FM-003) |
| TESTABLE | 8 (FM-001, FM-002, FM-004, FM-005, FM-006, FM-007, FM-008, FM-009) |
| VERIFIED | 0 |
| UNMAPPED (design gaps) | 0 |
| S0 (Structural) | 6 (FM-001, FM-004, FM-005, FM-006, FM-007, FM-008) |
| S1 (Methodological) | 2 (FM-002, FM-009) |
| S2 (Operational) | 1 (FM-003) |

### Coverage Summary

All 9 observed failure modes map to DDIS/Braid mechanisms. No design gaps identified.
FM-005 through FM-009 were discovered during the Session 006 deep audit (14 subagents
cross-referencing all guide/ files against spec/, SEED.md, and ADRS.md). These five new
FMs share a common theme: **multi-document coherence failure** — the class of problem that
arises when design knowledge is distributed across multiple prose documents instead of
residing in a single queryable store. This is precisely the problem Braid is designed to
solve. The audit itself is evidence for the design thesis.

| FM | Mechanism | Target SLA | Current Manual Rate |
|----|-----------|------------|---------------------|
| FM-001 | Harvest gap detection + FP/FN calibration | ≥99% decision capture | ~53% (47% miss) |
| FM-002 | Provenance typing lattice + traceability constraint | 100% verifiable provenance | Unknown (1 observed fabrication) |
| FM-003 | Single-substrate store + Associate/Assemble | ≥95% analysis coverage | ~54% (46% miss) |
| FM-004 | Fitness function coverage + bilateral loop | ≥99% completeness detection | ~53% (47% miss) |
| FM-005 | Content-addressed identity + bilateral scan | 100% semantic ID collision detection | ~33% aligned (67% phantom) |
| FM-006 | Drift detection + frontier staleness tracking | Drift flagged within 1 command cycle | 0% detection (17 stale values undetected) |
| FM-007 | ADR-as-data + bilateral loop | 0 cross-layer contradictions | 1 contradiction (BLAKE3 vs SHA-256) undetected |
| FM-008 | Query-derived metrics (Datalog over store) | 0 hardcoded counts | 17 stale hardcoded values across 5 files |
| FM-009 | Seed-loaded ADRs + contradiction detection | 0 silent ADR contradictions | 3 contradictions (topology, mechanisms, types) undetected |

### Failure Mode Clustering

The 9 failure modes cluster into three groups by root mechanism:

**Group A — Knowledge Capture** (FM-001, FM-002, FM-003):
Single-session failures where the agent's working knowledge exceeds what was externalized.
Primary mechanism: Harvest + Provenance + Single-substrate query.

**Group B — Cross-Document Coherence** (FM-004, FM-005, FM-006, FM-007, FM-008):
Multi-session failures where documents produced by different agents in different sessions
diverge without detection. Primary mechanisms: Bilateral loop + Drift detection + Content-
addressed identity + Query-derived metrics. This is the dominant cluster (5 of 9 FMs).

**Group C — Decision Preservation** (FM-009):
Multi-session failure where a settled decision is unknowingly relitigated because the
deciding context was not loaded. Primary mechanism: Seed + Contradiction detection.
