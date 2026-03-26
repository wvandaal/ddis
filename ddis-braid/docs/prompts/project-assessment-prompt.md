# Braid Project Assessment Prompt

> **Usage**: Paste this entire document into a fresh conversation with a frontier-class LLM
> (Claude Opus, GPT-4.5, Gemini Ultra). It is optimized for a single-turn, deep-substrate
> assessment at turn 1. Do NOT break it into multiple messages — the structure is load-bearing.
>
> **Generated**: 2026-03-26 by Claude Opus 4.6 using prompt-optimization + spec-first-design
> + rust-formal-engineering + skill-composition methodologies.

---

## Your Mission

You are conducting a deep assessment of **Braid**, an ambitious Rust project that implements
a **formal epistemology runtime** — infrastructure for organizational learning that gives
stateless LLM agents durable memory, self-calibration, and convergent coherence verification.

Your mandate: assess the project with the rigor of a principal investigator reviewing a
research program. Evaluate architectural soundness, implementation quality, roadmap alignment,
and strategic trajectory. Then produce the maximally accretive path forward — one that
satisfies lab-grade, zero-defect engineering standards while advancing the theoretical vision.

**What this is NOT**: a code review, a feature request, or a process audit. This is a
first-principles evaluation of whether a system designed to make organizations learn is
itself learning — and whether its architecture can deliver on its foundational claims.

---

## I. The Theoretical Vision

### The Atomic Operation

Every level of the system performs one operation:
**Observe reality → Compare to model → Reduce the discrepancy.**

This single operation — applied at the level of datoms, boundaries, gradients, calibration,
and policy merge — constitutes a complete learning system.

### The Core Identity

- **DDIS is a formal epistemology** — a mathematical framework for how shared knowledge grows
- **Braid is the runtime for that epistemology** — not a software tool, but infrastructure for organizational learning
- **The DDIS methodology (INV/ADR/NEG ontology) is the first APPLICATION** on the braid substrate, not part of the substrate itself (Constraint C8: Substrate Independence)
- **Braid is the Y-combinator for LLMs**: seed → session → harvest → store → new seed. When this converges, the function finds itself in its own output. f(Y(f)) = Y(f).

### The Eight Divergence Types (Complete Taxonomy)

| # | Type | What diverges | Detection | Resolution |
|---|------|--------------|-----------|------------|
| 1 | Epistemic | Model vs reality | Harvest gap | Harvest |
| 2 | Structural | Impl vs spec | Bilateral scan | Associate + reimplement |
| 3 | Consequential | Current vs future | Uncertainty tensor | Guidance |
| 4 | Aleatory | Agent vs agent | Merge conflict | Deliberation |
| 5 | Logical | Invariant vs invariant | Contradiction detection | ADR |
| 6 | Axiological | Implementation vs goals | Fitness function | Human review |
| 7 | Temporal | Frontier vs frontier | Frontier comparison | Sync barrier |
| 8 | Procedural | Behavior vs methodology | Drift detection | Dynamic AGENTS.md |

Plus Type 9 (Reflexive): system vs system's-model-of-itself.

### The Three Learning Loops

| Loop | What it learns | Mechanism |
|------|---------------|-----------|
| Weight calibration (OBSERVER-4) | How much each boundary matters | Predicted vs actual ΔF(S) → adjust weights |
| Structure discovery (OBSERVER-5) | What should be aligned with what | Temporal coupling → proposed boundaries |
| Ontology discovery (OBSERVER-6) | What categories of knowledge exist | Observation clustering → emergent categories |

### Three Irreducible Primitives

1. **Morphisms** — functions moving knowledge between reality and store
2. **Reconciliation** — intelligence comparing model to reality
3. **Acquisition Function** — hypothesis generator: α(action | store) = E[ΔF(S)] / cost(action)

Everything else is an instance of these three.

### The Epistemological Triangle

- **ASSERTION** (monadic): Reality → Store. How knowledge enters.
- **FALSIFICATION** (comonadic): H → (H, {¬Hᵢ}). How knowledge is tested.
- **WITNESS** (constructive): ¬¬H → H. How survival becomes knowledge.

Every datom exists in triple context: how it entered, how it could be destroyed, why it survived.
- OPINION = assertion only → contributes 0.0 to F(S)
- HYPOTHESIS = assertion + falsification context → 0.15
- KNOWLEDGE = assertion + falsification + constructive witness → 1.0

### Hard Constraints (Non-Negotiable)

- **C1**: Append-only store. Retractions are new datoms with op=retract.
- **C2**: Identity by content. Two agents asserting the same fact produce one datom.
- **C3**: Schema-as-data. Schema is datoms, not DDL.
- **C4**: CRDT merge by set union. No heuristics at merge time.
- **C5**: Traceability. Every artifact traces to spec, every spec to SEED.md.
- **C6**: Falsifiability. Every invariant has explicit violation condition.
- **C7**: Self-bootstrap. DDIS specifies itself; spec elements ARE the first data.
- **C8**: Substrate independence. Kernel must not hardcode DDIS or any methodology.

---

## II. Architecture

### Core Abstractions

```
Datom:       [entity, attribute, value, transaction, operation]
Store:       (P(D), ∪) — grow-only set. Merges are set union. Never shrinks.
Transaction: Entity carrying provenance (who, when, why, causal predecessors).
Resolution:  Per-attribute: lattice-resolved, last-writer-wins, or multi-value.
Frontier:    All datoms known to a specific agent at a specific point.
Harvest:     End-of-session knowledge extraction into store.
Seed:        Start-of-session relevant knowledge assembly from store.
Guidance:    Methodology pointer injected into every response.
F(S):        Fitness function quantifying convergence. Target: F(S) → 1.0.
```

### Architecture Stack

```
SUBSTRATE (universal, C8-compliant)
├─ Datom Store          — append-only, content-addressed, BLAKE3 hashing
├─ Quad Indexes         — EAVT, AEVT, VAET, AVET + LIVE materialized views
├─ Datalog Engine       — stratified, semi-naive, CALM-compliant
├─ Schema               — 19 axiomatic meta-schema attrs, 6-layer architecture
├─ CRDT Merge           — set union, per-attribute resolution modes
├─ Boundary Registry    — configurable coherence measurement
├─ Fitness Gradient     — F(S) computation across 7 components
├─ HLC Timestamps       — causally-ordered, globally-unique

LEARNING LOOPS (self-improving)
├─ Weight Calibration   — hypothesis ledger, predicted vs actual ΔF(S)
├─ Concept Engine       — observation clustering, emergent categories
├─ Structure Discovery  — temporal coupling → proposed boundaries

INTELLIGENCE (steering)
├─ Guidance System      — methodology.rs + routing.rs + context.rs
├─ Seed Assembly        — relevance scoring, budget compression
├─ Bilateral Engine     — spec↔impl bidirectional verification
├─ Trilateral Coherence — multi-dimensional convergence measurement

POLICY (domain-specific, replaceable per C8)
├─ Policy Manifest      — claim types, evidence types, boundaries as datoms
├─ Extractor Framework  — observer registry, domain-specific plugins
├─ Projector Registry   — output generation (seed, AGENTS.md, status)

INTERFACE
├─ CLI                  — 17+ commands: init, observe, harvest, seed, status, etc.
├─ MCP Server           — machine-to-machine protocol
├─ Dynamic AGENTS.md    — live store projection into agent instructions
```

### Key Design Decisions (Settled — documented in ADRS.md)

| ID | Decision | Rationale |
|----|----------|-----------|
| FD-001 | Append-only store | Mutable state is root of distributed correctness bugs |
| FD-002 | EAV over relational | Schema evolves without migrations |
| FD-003 | Datalog for queries | Natural graph joins, CALM compliance |
| FD-004 | Datom store over vector DB | Verification substrate, not retrieval heuristic |
| FD-005 | Per-attribute conflict resolution | Different attributes have different semantics |
| FD-006 | Self-bootstrap (C7) | Spec elements are first dataset |
| FD-007 | Content-addressable identity | Eliminates "same fact, different ID" problem |
| FD-010 | Embedded + optional session daemon | At scale, per-command process creation is O(n*k) |
| FD-013 | BLAKE3 hashing | 14x faster than SHA-256, pure Rust |
| ADR-FOUNDATION-012 | Braid as Epistemology Runtime | Substrate/application separation |
| ADR-FOUNDATION-013 | Declarative Policy Manifest | Coherence model as datoms, not code |
| ADR-FOUNDATION-014 | The Convergence Thesis | All problems = open loops; closing them = learning |
| ADR-FOUNDATION-017 | Hypothetico-Deductive Loop | observe → reconcile → hypothesize → act → calibrate |
| ADR-FOUNDATION-020 | Falsification-First Principle | F(S) rewards survived falsification |

---

## III. Current Implementation State (2026-03-26)

### Quantitative Metrics

| Metric | Value | Context |
|--------|-------|---------|
| **Rust LOC** | 127,228 | Across braid-kernel (core) + braid (CLI) |
| **Tests passing** | 2,043 | Unit, integration, proptest, Kani proofs (475 #[test] fns) |
| **Datoms in store** | 108,587 | The project's own accumulated knowledge |
| **Entities** | 10,243 | Distinct things the store knows about |
| **Transactions** | 10,810 | Distinct assertion events |
| **Issues total** | 883 | Tracked in beads (dependency-aware issue DB) |
| **Issues closed** | 813 (92%) | Across 46 development sessions |
| **Issues open** | 70 (8%) | 38 actionable, 32 blocked |
| **F(S)** | 0.62 | Fitness score (stagnant — not improving) |
| **Hypothesis ledger** | 145/273 completed | Mean error 0.521, trend: DEGRADING |
| **Spec↔Impl gaps** | 142 | Boundary coherence: 0.77 |
| **Untested INVs** | 265 | Current-stage invariants without L2+ witness |
| **Sessions** | 46 | Development sessions completed |
| **Velocity** | 812 issues closed in 30 days | Extremely high throughput |

### Kernel Source Files (40 files)

The largest files (by size, indicative of complexity concentration):

| File | Size | Responsibility |
|------|------|---------------|
| guidance.rs | 260 KB | Guidance system (merged methodology + routing) |
| store.rs | 222 KB | Core datom store, indexes, LiveStore |
| seed.rs | 205 KB | Seed assembly pipeline |
| concept.rs | 198 KB | Concept engine (emergent categories) |
| schema.rs | 180 KB | Schema system, 6-layer architecture |
| bilateral.rs | 169 KB | Spec↔impl bilateral verification |
| harvest.rs | 165 KB | Harvest pipeline |
| compiler.rs | 109 KB | Datalog query compiler |
| topology.rs | 108 KB | Multi-agent coordination topology |
| budget.rs | 101 KB | Token budget management |
| routing.rs | 106 KB | R(t) routing and acquisition function |
| context.rs | 90 KB | Context assembly |
| task.rs | 98 KB | Task management |
| trilateral.rs | 79 KB | Trilateral coherence model |
| methodology.rs | 64 KB | Methodology scoring |
| resolution.rs | 64 KB | Conflict resolution |
| policy.rs | 60 KB | Policy manifest |
| merge.rs | 55 KB | CRDT merge |
| signal.rs | 53 KB | Signal routing |
| coherence.rs | 48 KB | Coherence geometry |
| deliberation.rs | 49 KB | Deliberation engine |
| kani_proofs.rs | 76 KB | Formal verification harnesses |

### Specification State

- **22 spec files** across 4 waves (Foundation, Lifecycle, Intelligence, Integration)
- **83+ Stage 0 invariants** formalized with IDs, falsification conditions, verification tags
- **14 namespaces**: STORE, LAYOUT, SCHEMA, QUERY, RESOLUTION, HARVEST, SEED, MERGE, SYNC, SIGNAL, BILATERAL, DELIBERATION, GUIDANCE, BUDGET, INTERFACE, TRILATERAL, TOPOLOGY, COHERENCE, WITNESS, REFLEXIVE
- **30+ ADR categories** documented in ADRS.md with rationale and alternatives

### Recent Work (Sessions 044-046, last ~3 days)

- **Concept engine**: Observation clustering into emergent concepts
- **Self-calibrating thresholds**: Otsu algorithm for concept boundary detection
- **Online calibration**: Per-observation threshold adjustment (scored 4.1/10 in testing)
- **Inquiry engine**: From categorization to confrontation (epistemological ascent)
- **Performance regression analysis**: 16 tasks from deep architecture audit
- **DOGFOOD-2**: External validation on Go CLI project scored 3.48/10 — hash embedder bottleneck identified
- **Concept collapse**: Emergent concepts collapsing to single cluster — active investigation

### Key Subsystem Status

| Subsystem | Status | Notes |
|-----------|--------|-------|
| Datom Store | Production | Append-only, quad-indexed, LIVE views, content-addressed |
| Schema | Production | 6-layer architecture, self-bootstrap |
| Datalog Engine | Production | Stratified, semi-naive, 6 strata |
| Harvest/Seed | Operational | Semi-automated, narrative persistence |
| Guidance | Operational | 3-file split (methodology + routing + context) |
| CRDT Merge | Operational | Set union with per-attribute resolution |
| Policy Manifest | Verified | Declarative, C8-compliant, 33 tests |
| Extractor Framework | Complete | Layer 6 schema, meta-extractor |
| Bilateral Engine | Operational | 169 KB, drives F(S) |
| Hypothesis Ledger | Active but degrading | Mean error 0.521 (was 0.254 in Session 036) |
| Concept Engine | Experimental | Observation clustering, hash embedder bottleneck |
| Topology | Complete | Spectral partition, CALM classification, phase planning |
| Formal Verification | Active | Kani proofs (76 KB), proptest strategies |
| LiveStore | Production | In-memory + persistent, refresh_if_needed() |

---

## IV. The Roadmap

### Original Staged Plan (from SEED.md)

**Stage 0: Harvest/Seed Cycle** — Validate: harvest/seed transforms workflow from "fight context loss" to "ride context waves."
- Deliverables: transact, query, status, harvest, seed, guidance, dynamic AGENTS.md
- Success criterion: Work 25 turns, harvest, start fresh — new session picks up seamlessly
- **Status: ~95% complete** — all deliverables implemented, S0-CLOSE epic still open

**Stage 1: Budget-Aware Output + Guidance Injection**
- Budget system, attention economics, witness framework, F(S) reporting
- **Status: ~85% complete** — witness 11/11, ACP, R(t), topology, TAP-SPLIT done

**Stage 2: Branching + Deliberation**
- Branching store, deliberation protocol, structured conflict resolution
- **Status: ~50% complete** — deliberation engine exists, branching partial

**Stage 3: Multi-Agent Coordination**
- Agent working sets (W_α), merge cascade, sync barriers, topology compilation
- **Status: ~28% complete** — topology pipeline done, agent coordination partial

**Stage 4: Advanced Intelligence**
- Active observer daemon, ontology discovery, cross-project transfer
- **Status: ~8% complete** — daemon EPIC created, not implemented

### Evolved Phase Model (Session 036 revision)

The 5-stage plan was falsified and replaced with a 5-phase model:

| Phase | Name | Status |
|-------|------|--------|
| A | Substrate | ~95% — store, schema, query, resolution |
| B | Lifecycle | ~85% — harvest, seed, merge, guidance |
| C | Intelligence | ~60% — bilateral, trilateral, topology, concept engine |
| D | Self-Calibration | ~40% — hypothesis ledger active, attention NOT STARTED, comonadic partial |
| E | Active Runtime | ~5% — daemon not implemented, active observer not started |

### Top Priorities (from bv triage)

1. **S0-CLOSE** (P0 epic): Stage 0 completion — make harvest/seed replace HARVEST.md
2. **Layer 4 schema**: Task + plan workflow attributes (23 attrs)
3. **Persistent session identity**: Named entities + observation auto-linking
4. **Validation sessions**: Budget-aware output, bilateral CLI, instrumentation

### Known Problems

1. **F(S) stagnant at 0.62** — not improving across sessions
2. **Hypothesis ledger degrading** — mean error 0.521 (was 0.254), trend worsening
3. **142 spec↔impl boundary gaps** — coherence at 0.77
4. **265 untested invariants** — current-stage INVs without L2+ witness
5. **Concept collapse** — emergent concepts collapsing to single cluster
6. **Hash embedder bottleneck** — DOGFOOD-2 scored 3.48/10 on external project
7. **File size concentration** — 6 files > 150 KB; guidance.rs at 260 KB
8. **Velocity stall** — 0 issues closed in last 7 days (was 133/week prior)

---

## V. Assessment Framework

Evaluate the project across these dimensions. For each, provide: (a) current state assessment
with evidence, (b) gap analysis vs the theoretical vision, (c) specific recommendations with
priority ordering.

### Dimension 1: Architectural Fidelity

Does the implementation faithfully realize the theoretical vision?
- Are the 8 hard constraints (C1-C8) respected in the codebase?
- Is the substrate/application separation (C8) actually maintained?
- Does the one-operation principle (observe → compare → reduce) hold at every level?
- Are there open loops (NEG-010 violations)?
- Is the architecture actually converging, or is it accreting complexity?

### Dimension 2: Engineering Quality

Does the codebase meet lab-grade, zero-defect Rust engineering standards?
- **Type safety**: Are illegal states unrepresentable? What is the excess cardinality?
- **Error algebra**: Are error types caller-distinguishable and actionable?
- **Unsafe discipline**: SAFETY proofs for every unsafe block?
- **Module decomposition**: Are the 40 kernel files appropriately factored?
- **File sizes**: 6 files > 150 KB — is this technical debt or inherent complexity?
- **Test coverage**: 2,043 tests (475 test fns) for 127K LOC — is this sufficient?
- **Formal verification**: Kani proofs cover which invariants? What's the gap?

### Dimension 3: Convergence Health

Is the system actually learning? The bedrock claim is that braid converges toward truth.
- F(S) at 0.62 and stagnant — why? Is this a measurement problem or a real plateau?
- Hypothesis ledger degrading (error 0.521 → 0.254 → 0.521) — what broke?
- 142 spec↔impl gaps — are these closing or opening?
- 265 untested INVs — is the verification pipeline keeping up with specification?
- Concept collapse — is the ontology discovery loop working?
- Is the saw-tooth invariant (healthy F(S) oscillation) observable?

### Dimension 4: Roadmap Alignment

Is the project progressing toward its goals, or has scope creep diverted it?
- Original timeline: Stage 0 in "1-2 weeks." It's been 46 sessions over ~5 weeks. Assessment?
- S0-CLOSE is still open — what's actually blocking completion?
- 883 issues total, 70 open — is this healthy or is there issue inflation?
- Recent work on concept engine and inquiry engine — is this aligned with the critical path or premature?
- The velocity stall (0 closed in 7 days) — is this a natural pause or a signal?

### Dimension 5: Strategic Trajectory

Where should the project go from here to maximize impact?
- What is the highest-leverage work right now? (Not just "what's next in the backlog")
- Should the project prioritize depth (making what exists production-grade) or breadth (implementing remaining stages)?
- The DOGFOOD-2 score of 3.48/10 on an external project — what does this tell us?
- Is the daemon (Phase E) actually necessary, or can the CLI architecture deliver the vision?
- What would it take to reach F(S) > 0.8?
- Is there a "minimum viable product" that could demonstrate the vision to external users?

---

## VI. What I Want From You

### Output Structure

Produce a structured assessment document with these sections:

1. **Executive Summary** (1 paragraph): The single most important finding.

2. **Architectural Assessment**: For each of the 8 hard constraints, state whether it is
   maintained, at risk, or violated — with evidence from the codebase description.

3. **Engineering Quality Report**: Using the Rust formal engineering lens (types as propositions,
   make illegal states unrepresentable, error algebra, module factoring), assess the codebase.
   Identify the top 5 engineering improvements that would most improve code quality.

4. **Convergence Diagnosis**: Why is F(S) stagnant? Why is the hypothesis ledger degrading?
   Provide a formal diagnosis with the rigor of a scientific root cause analysis.

5. **Roadmap Critique**: What's working, what's not, and what should change. Be brutally
   honest about scope creep, premature optimization, and misaligned priorities.

6. **The Optimal Path Forward**: A prioritized plan for the next 5-10 sessions. For each
   recommended action:
   - What to do (specific, actionable)
   - Why this is highest-leverage (trace to bedrock vision)
   - What it unblocks
   - Estimated scope (small / medium / large)
   - Which hard constraints or learning loops it advances

7. **Strategic Risks**: What could prevent this project from delivering on its vision? Not
   bugs or missing features — structural risks, theoretical gaps, or architectural dead ends.

8. **The Deepest Question**: Based on your analysis, what is the one question the project
   should be asking itself but isn't?

### Quality Standards

- **First principles only.** Trace every recommendation to a bedrock principle, not to
  conventional wisdom. "Most projects do X" is not a reason.
- **Be specific.** "Improve test coverage" is not useful. "Add property tests for
  store.rs merge operation to verify C4 (CRDT set union commutativity) under concurrent
  append" is useful.
- **Be honest.** If the project is overengineered, say so. If the theoretical vision is
  unrealizable, say so. If the architecture is sound but the execution is weak, say so.
  Flattery is axiological divergence.
- **Think in formal systems.** When you assess convergence, think about fixed points.
  When you assess architecture, think about algebraic structure. When you assess
  engineering, think about type-theoretic guarantees. The project's own framework
  gives you the vocabulary — use it.

---

## VII. A Demonstration of the Quality I Expect

Here is an example of the depth and specificity I'm looking for, applied to a hypothetical finding:

> **Finding: F(S) Stagnation is a Measurement Artifact**
>
> F(S) = 0.62 has been stable across Sessions 033-046. This appears alarming but the
> diagnosis is nuanced. F(S) is computed from 7 weighted components (CO-009). Three of
> those components — coverage, depth, and coherence — scale with the ratio of verified
> invariants to total invariants. As the project adds new spec elements (the denominator
> grows), verification must keep pace (the numerator) or F(S) drops.
>
> The project has been adding spec elements faster than verifying them: 265 untested INVs
> represent denominator growth without numerator growth. This is not stagnation — it is
> the natural consequence of expanding the specification horizon faster than the
> verification pipeline can follow.
>
> **Diagnosis**: The saw-tooth invariant predicts F(S) dips from new specification (surprise)
> followed by recovery from verification (consolidation). We see the dip but not the
> recovery. The verification pipeline is the bottleneck.
>
> **Recommendation**: Freeze new specification work for 3-5 sessions. Focus entirely on
> witness verification for the 265 untested INVs. Predict: F(S) will rise to ~0.75.
> This validates the saw-tooth model and proves the system is learning, not stagnating.
>
> **Trace**: This recommendation advances OBSERVER-4 (calibration loop) by providing
> ground truth for the hypothesis ledger, which is degrading precisely because untested
> INVs produce unreliable ΔF(S) measurements.

This is the kind of analysis I want: grounded in the project's own formalism, specific in
its diagnosis, actionable in its recommendation, and traced to first principles.

---

*This assessment will be used to steer the project's next phase of development. The goal
is not validation — it is calibration. Tell us what's true, not what we want to hear.
The system is designed to learn from honest assessment. Give it something to learn from.*
