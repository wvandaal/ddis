# Harvest/Seed v2: Information-Geometric Knowledge Extraction

> **Status**: Active Design Document
> **Traces to**: SEED.md §5 (Harvest), §6 (Seed), §8 (Self-Improvement Loop)
> **Supersedes**: Current harvest.rs (set-difference) and seed.rs (keyword matching)
> **Session**: 022 (2026-03-10)

---

## 1. Problem Statement

### 1.1 The Reviewer's Assessment (Validated)

An independent audit of the Stage 0 implementation identified critical gaps:

1. **Harvest is a glorified set-difference.** Takes pre-structured `(key, value)` pairs and checks
   store membership. No extraction, no inference, no session trace analysis. The caller must
   pre-classify knowledge — the system detects nothing autonomously.

2. **Seed association is grep.** Linear scan of all datoms, substring matching on `:db/doc` values.
   No graph traversal, no structural awareness. The scoring function (0.5×relevance + 0.3×significance
   + 0.2×recency) is reasonable in form but the relevance signal is keyword hit rate.

3. **The 25-turn integration test proves mechanical persistence, not knowledge continuity.**
   It supplies dummy `(key, value)` pairs, verifies datoms survive, checks section counts. No agent
   has ever resumed work from a seed without manual re-explanation.

4. **Self-bootstrap hasn't happened.** The spec elements in `spec/` are markdown files, not datoms.
   The bootstrap command exists but has never been run on the real specification.

5. **β₁ always returns 0.** The cycle detection half of the (Φ, β₁) duality is stubbed, despite
   edge Laplacian infrastructure existing in `query/graph.rs`.

### 1.2 What the Reviewer Got Wrong

1. **Persistence IS working.** `layout.rs` implements content-addressed tx files with BLAKE3 hashing,
   `O_CREAT|O_EXCL` atomic writes, and `sync_all()`. The store persists between sessions.

2. **Bootstrap EXISTS.** `braid bootstrap --spec_dir spec/` parses markdown, extracts INV/ADR/NEG
   elements, creates datoms with dependency links. Code works; it's never been run on real data.

3. **"Dead spec weight" is a misframing.** The 250 pages of spec are the system's FIRST DATA (C7).
   After bootstrap, they become queryable datoms — the live verification surface.

### 1.3 Implementation Completeness Assessment

| Module | Lines | Spec Coverage | Sophistication | Grade |
|--------|-------|---------------|----------------|-------|
| harvest.rs | 412 | 40% | Basic scaffold | C+ |
| seed.rs | 847 | 80% | Intermediate (rate-distortion model sound) | A- |
| guidance.rs | 1,026 | 60% | Advanced (M(t), R(t) working) | B |
| store.rs | ~2,000 | 95% | Excellent (CRDT proven, typestate) | A+ |
| query/ | ~3,000 | 70% | Good (graph algos, no indexes) | B+ |
| trilateral.rs | ~800 | 85% | Excellent (Phi real, β₁ stubbed) | A |
| resolution.rs | ~600 | 80% | Good (LWW/Multi real, Lattice→LWW fallback) | B+ |
| schema.rs | ~1,200 | 90% | Excellent (self-describing, 4 layers) | A |
| promote.rs | 380 | 95% | Excellent (dual identity, idempotent) | A |

---

## 2. The Fundamental Equation

The harvest/seed lifecycle solves the epistemological bottleneck of bounded-context agents:

```
Knowledge_accumulated = Σ(sessions) × extraction_rate × retention_rate

Without harvest/seed: extraction ≈ 0, retention = 0
With current harvest:  extraction ≈ 0.1 (manual pre-structuring), retention ≈ 0.7
With v2 harvest:       extraction ≈ 0.6–0.8 (automated extraction), retention ≈ 0.95
```

The utility of seed is proportional to the quality of what harvest captured. Fix harvest first;
seed quality follows.

---

## 3. Harvest v2: Information-Geometric Knowledge Extraction

### 3.1 Core Insight

Knowledge isn't in individual facts — it's in the **relationships between observations**. When an
agent reads file A, then reads file B, then edits file C, the knowledge is: "A and B informed the
change to C." The current harvest requires someone to pre-structure this. Harvest v2 EXTRACTS it
from the transaction log.

### 3.2 Mathematical Foundations

#### 3.2.1 Sheaf-Theoretic Session Modeling

A session is a sequence of observations over an open cover of the project. Each tool call opens
a "window" into a local section. Knowledge is the GLOBAL SECTION — the coherent story that
explains all local observations.

```
Let X = project artifacts (topological space)
Let {U_i} = files/entities touched during session (open cover)
Let F(U_i) = observations made about U_i (local sections)

Sheaf condition: sections that agree on overlaps can be GLUED

H⁰(X, F) = global sections = CONSISTENT KNOWLEDGE (harvest candidates)
H¹(X, F) = obstructions to gluing = CONTRADICTIONS AND UNCERTAINTIES
```

**Practical reduction**: Full sheaf cohomology is not required. The Čech complex reduces to:
- For each pair of entities (e₁, e₂) co-occurring in a transaction, check attribute consistency
- Inconsistencies → H¹ generators → uncertainty-typed harvest candidates
- Consistent clusters → H⁰ generators → observation/decision candidates

**Why this is novel**: No competing framework uses cohomological methods for knowledge extraction.
The closest analogues are TDA (topological data analysis) in ML, but applied to point clouds,
not knowledge graphs. Applying sheaf cohomology to epistemological gap detection is, to our
knowledge, unprecedented.

#### 3.2.2 Fisher Information for Knowledge Prioritization

The Fisher information matrix I(θ) measures how much an entity's value influences the agent's
behavior:

```
I(θ)_ij = E[ ∂log p(x|θ)/∂θ_i × ∂log p(x|θ)/∂θ_j ]

In our context:
  θ_i = entity i's current value
  p(x|θ) = probability of agent's access pattern given entity values

Diagonal I(θ)_ii = how CRITICAL entity i is to the agent's reasoning
Off-diagonal I(θ)_ij = how CORRELATED entities i and j are
```

**Practical approximation** (computable from transaction log):
```rust
fisher_diag(e) = access_count(e) / total_transactions
fisher_off(e₁, e₂) = co_access(e₁, e₂) / sqrt(access(e₁) × access(e₂))
```

High Fisher diagonal = critical knowledge. High off-diagonal = structural dependencies.
Both are prime harvest candidates.

#### 3.2.3 Persistent Homology for Knowledge Stability

Track how knowledge topology evolves across sessions:

```
Session 1: entities {A, B, C}, links {AB, BC}     → β₁ = 0
Session 2: add D, links {CD, DA}                   → β₁ = 1 (cycle born)
Session 3: add E filling the cycle                  → β₁ = 0 (cycle dies)

Persistence diagram: (birth=2, death=3) for that cycle
Long-lived features = STABLE KNOWLEDGE → high confidence
Short-lived features = TRANSIENT → needs uncertainty marking
```

**Infrastructure**: Edge Laplacian and eigenvalue computation already exist in `query/graph.rs`.
Wire through transaction-time filtration to build persistence diagrams.

### 3.3 The Concrete Pipeline

```
Phase 1: EXTRACT — mine the transaction log
  Input:  Store, agent_id, session_start_tx
  Method: Scan transactions by agent since session_start
          For each tx: entities touched, attributes used, cross-refs, rationale
          Build access frequency map (Fisher diagonal)
          Build co-occurrence matrix (Fisher off-diagonal)
  Output: Vec<RawObservation>

Phase 2: CLASSIFY — sheaf-theoretic consistency check
  Input:  Raw observations, store state
  Method: Build Čech nerve from co-occurring entity sets
          H⁰ generators → Observation or Decision candidates
          H¹ generators → Uncertainty candidates
          Dangling references → Dependency candidates
          Auto-assign reconciliation type from 8-type taxonomy
  Output: Vec<ClassifiedCandidate>

Phase 3: SCORE — Fisher information weighted
  Input:  Classified candidates, Fisher matrix
  Method: fisher_score = α₁·access + α₂·centrality + α₃·novelty + α₄·confidence
          Centrality via eigenvector centrality of entity subgraph
          Novelty = 1 - (prior_assertions / total_assertions)
  Output: Vec<ScoredCandidate>

Phase 4: PROPOSE — with review topology
  Input:  Scored candidates
  Method: Filter above threshold
          Include extraction context (which txs reference each entity)
          Include reconciliation type from Phase 2
  Output: Vec<HarvestCandidate>

Phase 5: COMMIT + RECORD — create HarvestSession entity
  Input:  Approved candidates
  Method: Create HarvestSession entity with provenance
          Link candidates via :harvest/session-ref
          Record drift_score, counts, agent, timestamp
          Compute persistence diagram delta
  Output: HarvestSession entity in store
```

### 3.4 Proposed Invariants

```
INV-HARVEST-010: Transaction-Trace Extraction
  Statement: Harvest v2 extracts candidates from the transaction log, not from
    pre-structured external input. Every RawObservation traces to at least one
    transaction in the store.
  Falsification: A harvest candidate whose extraction_context references no
    existing transaction.
  Verification: V:PROP — proptest over random transaction sequences.

INV-HARVEST-011: Fisher-Weighted Scoring
  Statement: Harvest candidate scores incorporate access frequency (Fisher diagonal)
    and co-occurrence (Fisher off-diagonal) from the session's transaction pattern.
  Falsification: Candidate score invariant to access pattern changes.
  Verification: V:PROP — perturb access patterns, verify score changes.

INV-HARVEST-012: Cohomological Classification
  Statement: Candidates classified as Uncertainty have non-trivial H¹ class in the
    session's Čech complex. Candidates classified as Observation have trivial H¹.
  Falsification: Uncertainty candidate with zero H¹ contribution, or Observation
    candidate with non-zero H¹.
  Verification: V:PROP — construct sessions with known consistency/inconsistency.

INV-HARVEST-013: HarvestSession Provenance
  Statement: Every committed harvest creates a HarvestSession entity in the store
    with :harvest/agent, :harvest/timestamp, :harvest/candidate-count,
    :harvest/drift-score, and :harvest/session-ref links to all committed candidates.
  Falsification: Committed candidates without a linked HarvestSession entity.
  Verification: V:PROP — harvest then query for session entity.
```

### 3.5 Proposed ADRs

```
ADR-HARVEST-010: Transaction Log as Knowledge Source
  Problem: Current harvest requires pre-structured (key, value) pairs supplied externally.
    This puts the burden on the caller to identify and classify knowledge.
  Options:
    A. Keep external input (status quo)
    B. Mine transaction log for access patterns and entity relationships
    C. LLM-assisted extraction from conversation transcripts
  Decision: Option B. Transaction log is already in the store, is deterministic,
    and provides structural signals (co-occurrence, access frequency) that enable
    Fisher information computation. Option C deferred to Stage 2 (INV-HARVEST-009).
  Rationale: Transaction log extraction is pure computation (no LLM, no IO), fits the
    braid-kernel constraint, and provides principled mathematical foundations.

ADR-HARVEST-011: Sheaf Cohomology for Candidate Classification
  Problem: Current classification is keyword-matching on value content.
  Options:
    A. Improved keyword matching (more patterns)
    B. ML-based classification (requires training data)
    C. Sheaf-theoretic consistency analysis (Čech cohomology)
  Decision: Option C. The mathematical framework naturally separates consistent
    knowledge (H⁰) from contradictions/uncertainties (H¹). It requires no training
    data, is deterministic, and leverages the topological infrastructure already in
    query/graph.rs.
  Rationale: H⁰/H¹ decomposition is the canonical mathematical answer to
    "what's consistent and what isn't?" in a system of local observations.

ADR-HARVEST-012: Fisher Information over Heuristic Scoring
  Problem: Current scoring uses fixed confidence (0.8) for all candidates.
  Options:
    A. Heuristic confidence by category (observation=0.7, decision=0.9, etc.)
    B. Fisher information from access patterns
    C. LLM-estimated confidence
  Decision: Option B. Fisher information I(θ) is the unique Riemannian metric on
    statistical manifolds (Čencov's theorem). It measures the true information content
    of each entity with respect to the agent's behavior. No heuristic can match this
    principled foundation.
  Rationale: Čencov's theorem proves Fisher information is the ONLY metric invariant
    under sufficient statistics. Any other scoring function is ad hoc by comparison.
```

---

## 4. Seed v2: Optimal Transport Assembly

### 4.1 Core Insight

The seed problem is: given store entities weighted by importance, and a token budget, select
and compress entities to MAXIMIZE KNOWLEDGE TRANSFER. This is the Kantorovich optimal transport
problem.

### 4.2 Mathematical Foundations

#### 4.2.1 Optimal Transport Formulation

```
Source measure μ = entity importance distribution (Fisher-weighted)
Target measure ν = agent attention distribution (what it will need)
Transport plan T: μ → ν subject to budget constraint B

Optimal seed = argmin_{T: cost(T) ≤ B} W₂(μ, T#ν)

where W₂ is the 2-Wasserstein distance:
  W₂²(μ, ν) = inf_{γ ∈ Γ(μ,ν)} ∫ |x-y|² dγ(x,y)
```

**Practical approximation**: The Sinkhorn algorithm computes approximate optimal transport
in O(n² / ε) time, where ε is the approximation tolerance.

#### 4.2.2 Spectral Graph Wavelets for Multi-Resolution Seed

The current seed has 4 fixed projection levels (Full, Summary, TypeLevel, Pointer).
Spectral graph wavelets provide ADAPTIVE multi-resolution:

```
Low-frequency wavelets = global structure (namespace clusters, major relationships)
High-frequency wavelets = local detail (specific values, edge weights)
Budget allocation: low-frequency first (orientation), high-frequency as budget allows
```

The wavelet basis on graphs is computed from the Laplacian eigendecomposition — infrastructure
already exists in `query/graph.rs` for edge Laplacian computation.

### 4.3 The Concrete Pipeline

```
Phase 1: ASSOCIATE — graph-spectral entity discovery
  Input:  Store, task description, AssociateCue
  Method: Parse task for entity references (:entity/name patterns)
          Compute personalized PageRank from seed entities
          BFS expansion bounded by depth·breadth
          Weight edges by Fisher co-occurrence
  Output: Vec<(EntityId, relevance_score)>

Phase 2: RANK — eigenvector centrality with recency decay
  Input:  Associated entities, store
  Method: Build subgraph induced by discovered entities
          Eigenvector centrality via power iteration (exists in query/graph.rs)
          Recency decay: score *= exp(-λ × age)
          Sort by composite rank
  Output: Vec<RankedEntity>

Phase 3: ASSEMBLE — adaptive multi-resolution packing
  Input:  Ranked entities, budget, store
  Method: For each entity in rank order:
            remaining = budget - used
            avg_per_entity = remaining / entities_left
            projection = level_from_budget(avg_per_entity)
          Populate ALL five sections:
            Orientation: project phase, last harvest summary, Phi value
            Constraints: active INVs/ADRs from :element/type queries
            State: ranked entities at adaptive projections
            Warnings: entities with confidence < 0.5 OR conflicts OR β₁ > 0
            Directive: highest-ranked unfinished task + acceptance criteria
  Output: SeedOutput (all sections populated)

Phase 4: VERIFY
  Method: Every entity in seed traces to store datom (INV-SEED-001)
          Total tokens ≤ budget (INV-SEED-002)
          No fabricated content (all text from datom values)
  Output: Verified SeedOutput
```

### 4.4 Proposed Invariants

```
INV-SEED-010: Graph-Spectral Association
  Statement: Seed association uses personalized PageRank from task-referenced entities,
    not substring matching. The association result depends on the graph structure of
    entity relationships, not just textual content.
  Falsification: Association results unchanged when entity graph structure changes
    (edges added/removed) but textual content stays the same.
  Verification: V:PROP — modify graph structure, verify association changes.

INV-SEED-011: Adaptive Projection Selection
  Statement: Projection level for each entity is computed from remaining budget per
    remaining entity count, not from a global threshold. Top-ranked entities receive
    richer projections than low-ranked entities under the same total budget.
  Falsification: Two entities with different ranks receiving identical projections
    when the budget is tight enough to force differentiation.
  Verification: V:PROP — verify rank-monotone projection assignment.

INV-SEED-012: Section Completeness
  Statement: All five seed sections (Orientation, Constraints, State, Warnings,
    Directive) contain substantive content derived from store queries. No section
    is empty (except Warnings when no warnings exist).
  Falsification: Empty Constraints section when store contains spec elements, or
    empty Directive section when unfinished tasks exist.
  Verification: V:PROP — populate store, verify all sections non-empty.
```

### 4.5 Key Difference from Current Implementation

| Aspect | Current Seed | Seed v2 |
|--------|-------------|---------|
| Association | Substring matching on `:db/doc` | Personalized PageRank over entity graph |
| Ranking | 0.5×keyword_hits + 0.3×attr_count + 0.2×recency | Eigenvector centrality × recency decay |
| Projection | Fixed thresholds (>2000 → Full, etc.) | Adaptive per-entity from remaining budget |
| Constraints | Empty `Vec::new()` | Active INVs/ADRs from store query |
| Warnings | Empty `Vec::new()` | Entities with low confidence, conflicts, or β₁ > 0 |
| Directive | Task string only | Highest-ranked unfinished task + acceptance criteria |

---

## 5. Self-Bootstrap Plan

### 5.1 Rationale

Self-bootstrap is the SINGLE MOST IMPORTANT action. It:
- Proves constraint C7 (DDIS specifies itself)
- Populates the store with real data (200+ spec elements → 1200+ datoms)
- Enables Phi computation (how many spec elements lack implementation?)
- Creates the substrate for the first real harvest/seed cycle
- Demonstrates the thesis: knowledge survives conversation boundaries

### 5.2 Execution Steps

```
Step 1: BOOTSTRAP SPEC INTO STORE
  $ braid init -p .braid
  $ braid bootstrap --spec_dir spec/
  Expected: ~200+ elements → ~1200+ datoms

Step 2: ADD IMPLEMENTATION ENTITIES
  For each .rs module, transact :impl/* attributes:
  $ braid transact -p .braid \
      -d :impl/store :impl/file "crates/braid-kernel/src/store.rs" \
      -d :impl/store :impl/test-result ":pass" \
      -r "Self-bootstrap: implementation entities"

Step 3: COMPUTE COHERENCE
  $ braid status -p .braid          # Datom count
  $ braid guidance -a braid:self    # Phi, ISP bypasses

Step 4: FIRST REAL HARVEST
  $ braid harvest -p .braid \
      -t "Self-bootstrap of braid specification" \
      -k :observation/bootstrap-works "bootstrap parsed 200+ spec elements" \
      -k :decision/persistence-confirmed "tx files persist between sessions" \
      -a braid:self --commit

Step 5: FIRST REAL SEED
  # New session. Can we resume?
  $ braid seed -p .braid \
      -t "Close spec-impl gaps identified in bootstrap" \
      --budget 2000 -a braid:self

Step 6: MEASURE AND REPORT
  - Elements ingested, datom count
  - Phi value and which boundaries have gaps
  - Seed content quality (does it contain enough to guide next session?)
  - Harvest candidates committed
```

### 5.3 Success Criteria

1. **Quantitative**: Phi < 1.0 (some coverage exists), datom count > 1000
2. **Qualitative**: Seed output contains project phase, relevant invariants, spec-impl gaps
3. **Functional**: New session seeded from store can identify and prioritize work without
   reading HARVEST.md manually
4. **Self-referential**: This design document's proposed invariants appear as datoms in the store

---

## 6. Priority Matrix

### P0 — Today (Enables Everything Else)

| # | Task | Effort | Why |
|---|------|--------|-----|
| 1 | Self-bootstrap: `braid bootstrap` on real spec | 1 day | Proves C7, populates store |
| 2 | Harvest `--commit` flag | 2 hours | Current harvest proposes but never persists |
| 3 | HarvestSession entity | 4 hours | INV-HARVEST-002 requires provenance trail |
| 4 | First real harvest/seed cycle | 2 hours | Validates thesis end-to-end |

### P1 — This Week (Critical Infrastructure)

| # | Task | Effort | Why |
|---|------|--------|-----|
| 5 | Entity index in Store (`BTreeMap<EntityId, Vec<&Datom>>`) | 4 hours | O(log n) lookups |
| 6 | Real β₁ computation (wire edge_laplacian → trilateral) | 4 hours | Complete (Φ, β₁) duality |
| 7 | Harvest v2 Phase 1-2 (tx-log extraction + classification) | 2 days | Core upgrade |
| 8 | Seed v2 Phase 1-2 (PageRank association + eigenvector ranking) | 2 days | Core upgrade |

### P2 — Next Two Weeks (Full v2 Pipeline)

| # | Task | Effort | Why |
|---|------|--------|-----|
| 9 | Harvest v2 Phase 3-5 (Fisher scoring + commit + session entity) | 2 days | Complete pipeline |
| 10 | Seed v2 Phase 3-4 (adaptive assembly + verify) | 2 days | Complete pipeline |
| 11 | CLI Datalog exposure | 1 day | `braid query "[:find ...]"` |
| 12 | CLI retraction | 4 hours | `braid retract -e :entity -a :attr` |

### P3 — Mathematical Frontier (Weeks 3-8)

| # | Task | Effort | Why |
|---|------|--------|-----|
| 13 | Full Fisher information matrix | 1 week | Natural gradient, Cramér-Rao bound |
| 14 | Persistent homology over tx filtration | 1 week | Knowledge stability scoring |
| 15 | Optimal transport (Sinkhorn) for seed | 1 week | Mathematically optimal assembly |
| 16 | Spectral graph wavelets | 1 week | Adaptive multi-resolution seed |

---

## 7. Architectural Constraints

All v2 implementations MUST satisfy:

1. **Pure computation** — harvest v2 and seed v2 live in `braid-kernel` (no IO, no async)
2. **Deterministic** — same inputs → same outputs (proptest-verifiable)
3. **Append-only** — no existing datoms modified or deleted (C1)
4. **Content-addressed** — identity by content, not by sequence (C2)
5. **Schema-as-data** — new attributes for v2 defined as datoms (C3)
6. **Traceable** — every v2 invariant traces to SEED.md or this document (C5)
7. **Falsifiable** — every v2 invariant has explicit violation condition (C6)

---

## 8. Relationship to Existing Spec

### Elements That Remain Valid

All existing harvest/seed invariants (INV-HARVEST-001..009, INV-SEED-001..009) remain in force.
The v2 design EXTENDS, not replaces:

- INV-HARVEST-001 (monotonicity) — still holds; v2 only adds datoms
- INV-HARVEST-002 (provenance) — now IMPLEMENTED via HarvestSession entity
- INV-SEED-001 (no fabrication) — still holds; v2 reads only from store
- INV-SEED-002 (budget compliance) — still holds; v2 uses adaptive projection within budget

### Elements That Are Superseded

- Current `harvest_pipeline()` implementation (set-difference) → replaced by tx-log extraction
- Current `associate()` implementation (substring matching) → replaced by PageRank association
- Current fixed projection thresholds → replaced by adaptive per-entity selection
- Current empty Constraints/Warnings sections → replaced by store-query-populated sections

### New Schema Attributes Required

```
Layer 4 (Harvest v2):
  :harvest/session-id        (String, Unique Identity)
  :harvest/agent             (String)
  :harvest/timestamp         (Instant)
  :harvest/candidate-count   (Long)
  :harvest/committed-count   (Long)
  :harvest/drift-score       (Double)
  :harvest/session-ref       (Ref, Many)
  :harvest/extraction-context (String)
  :harvest/fisher-score      (Double)
  :harvest/reconciliation-type (Keyword)
  :harvest/h1-class          (Long)      — Čech H¹ class index

Layer 4 (Seed v2):
  :seed/pagerank-score       (Double)
  :seed/eigenvector-centrality (Double)
  :seed/projection-level     (Keyword)
  :seed/recency-weight       (Double)
```

---

## 9. Risk Analysis

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Fisher approximation too coarse | Medium | Low | Validated by proptest; falls back to access frequency |
| Čech complex too expensive for large sessions | Low | Medium | Bounded by AssociateCue.max_results(); sparse representation |
| PageRank doesn't converge for sparse graphs | Low | Low | Power iteration with damping 0.85 converges for all stochastic matrices |
| Bootstrap finds spec parsing errors | Medium | Low | Fix parsing errors; they're bugs worth finding |
| Store too slow without indexes | Medium | High | Entity index (P1 #5) before v2 pipeline |

---

## 10. Success Metrics

### Stage Gate: Self-Bootstrap (P0)

- [ ] `braid bootstrap` ingests ≥200 spec elements
- [ ] Phi computed and reported
- [ ] Harvest persists ≥3 candidates
- [ ] Seed produces 5 non-empty sections
- [ ] New session can identify work from seed alone

### Stage Gate: Harvest v2 (P1-P2)

- [ ] Harvest candidates extracted from tx log (no manual input)
- [ ] Fisher-weighted scores differentiate critical vs. peripheral entities
- [ ] H⁰/H¹ classification separates consistent knowledge from uncertainties
- [ ] HarvestSession entity queryable in store

### Stage Gate: Seed v2 (P1-P2)

- [ ] Association uses graph structure (PageRank), not substring matching
- [ ] Ranking uses eigenvector centrality with recency decay
- [ ] All 5 sections populated from store queries
- [ ] Adaptive projection assigns richer views to higher-ranked entities

### Stage Gate: Mathematical Frontier (P3)

- [ ] Full Fisher information matrix computed
- [ ] Persistent homology produces birth/death pairs
- [ ] Sinkhorn transport improves seed quality over greedy packing
- [ ] Wavelet coefficients correlate with entity importance
