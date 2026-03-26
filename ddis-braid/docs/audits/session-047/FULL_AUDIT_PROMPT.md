# Braid Full Audit Prompt

> **Purpose**: This prompt produces a comprehensive, formally rigorous audit of the
> Braid project — identifying unsoundness, architectural misalignment, accretive
> opportunities, and the optimal path to production-grade zero-defect software.
>
> **Target**: An LLM agent with full codebase access, operating in Claude Code or
> equivalent environment with Read/Grep/Glob/Bash tools.
>
> **Design methodology**: This prompt was engineered using four composition skills
> (prompt-optimization, spec-first-design, rust-formal-engineering, skill-composition)
> with explicit DoF calibration, trajectory seeding, and phase separation.

---

## Prompt

You are conducting a comprehensive audit of **Braid**, a Rust implementation of a
formal epistemology runtime. This is not a code review. This is a first-principles
investigation of whether the system's mathematical foundations, architectural
decisions, implementation, and empirical behavior are sound, coherent, and aligned
with the project's stated goals.

### What Braid Is

Braid is infrastructure for organizational learning — not a software development
tool. Its atomic operation: observe reality, compare to model, reduce the
discrepancy. The system is built on three primitives:

1. **Morphisms** — structure-preserving maps between knowledge representations
2. **Reconciliation** — detecting and resolving divergence between model and reality
3. **Acquisition function** — scoring potential observations by expected information gain

The kernel is a universal substrate. DDIS (Decision-Driven Implementation
Specification) is the first *application* on this substrate — one epistemological
policy among many. The test: "would this make sense for a React project? A research
lab? A compliance team?" If no, it belongs in the application layer, not the kernel.

The core data model is an append-only datom store: `[entity, attribute, value,
transaction, operation]`. The store is a G-Set CRDT under set union — merging two
stores is the mathematical set union of their datom sets. Conflict resolution is
per-attribute at the query layer, not the storage layer.

### What You Must Internalize Before Proceeding

Read these files **in this order**. Do not begin analysis until you have read all of
them. Each restructures your understanding for the next:

1. **`SEED.md`** — The foundational design document. 11 sections. Contains the
   divergence taxonomy, all hard constraints (C1-C8), reconciliation mechanisms,
   three learning loops, and staged roadmap. This is the source of truth for what
   the system *should* be.

2. **`CLAUDE.md`** (this project's AGENTS.md) — Session methodology, constraints,
   negative cases (NEG-001 through NEG-010), the reconciliation taxonomy table.
   Pay particular attention to C8 (substrate independence) and NEG-009 (don't
   regress to "software tool" framing) and NEG-010 (don't open loops).

3. **`spec/README.md`** then **`spec/00-preamble.md`** — The specification
   architecture: 4 waves, 22 files, three-level refinement (Level 0 algebraic law,
   Level 1 state machine, Level 2 Rust implementation contract), 7 verification
   tags (V:TYPE, V:PROP, V:KANI, V:CONTRACT, V:MODEL, V:DEDUCTIVE, V:MIRI).

4. **`spec/01-store.md`** — The algebraic foundation. Study the Level 0 formalism:
   `(P(D), ∪)` is a join-semilattice. INV-STORE-001 through INV-STORE-010.
   Understand how each invariant maps from algebraic law to Rust types.

5. **`docs/design/ADRS.md`** — 139 design decisions across 14 categories. Every
   settled choice with rationale. Do not relitigate these (NEG-002) unless you find
   a formal contradiction.

6. **`docs/design/FAILURE_MODES.md`** — Known failure modes with acceptance criteria.
   These are test cases for evaluating whether the methodology works.

7. **`crates/braid-kernel/src/lib.rs`** — The public API surface. 60+ exported
   functions across 30+ modules. Understand what the kernel exposes.

8. **`crates/braid-kernel/src/store.rs`** — The core implementation. Typestate
   pattern (Building→Committed→Applied), content-addressed EntityIds, hybrid
   logical clocks. This is the foundation everything builds on.

9. **`crates/braid-kernel/src/kani_proofs.rs`** — 22+ bounded model checking
   harnesses. Understand what is formally verified vs. what is only tested.

10. **`crates/braid-kernel/src/bilateral.rs`** — Fitness function F(S), convergence
    analysis, spectral certificates. This is the coherence measurement engine.

After reading these 10 artifacts, you should be able to answer:
- What algebraic structure governs the store? What laws must hold?
- What is the typestate protocol for transactions? What does it prove?
- Where is the boundary between kernel (universal) and application (DDIS-specific)?
- What is formally verified (Kani) vs. property-tested (proptest) vs. only unit-tested?
- What does F(S) actually measure, and is the measurement sound?

If you cannot answer these questions with precision, re-read before continuing.

---

### Phase 1: Soundness Audit (Very High DoF — Discover)

**Goal**: Determine whether the system's formal guarantees are actually sound.
Not "does it compile" or "do tests pass" — but "do the mathematical claims hold?"

**Methodology**: For each subsystem, apply the three-box decomposition:
- **Black box**: What does the specification claim? (Read `spec/` files)
- **State box**: What invariants does the implementation maintain? (Read source)
- **Clear box**: Is the implementation verifiable against the state box? (Line-by-line)

Investigate these specific concerns:

#### 1.1 CRDT Soundness

The store claims to be a G-Set CRDT with set union as merge. Verify:

- **Commutativity**: `merge(A, B) = merge(B, A)` — Is this proven by Kani? Does
  the proof cover all Value types (9 variants)? What about edge cases with
  `ordered_float::OrderedFloat`?
- **Associativity**: `merge(merge(A, B), C) = merge(A, merge(B, C))` — Same questions.
- **Idempotency**: `merge(A, A) = A` — Trivial for set union but verify the
  implementation doesn't have side effects (index updates, frontier advancement).
- **Content-addressed identity**: Two agents asserting the same fact produce one
  datom. Verify that `EntityId::from_content()` is deterministic across all inputs
  and that the BLAKE3 hash covers exactly [e, a, v, tx, op].
- **Monotonicity**: The store only grows. Verify no code path removes datoms.
  Search for any `.remove()`, `.clear()`, `.retain()` on the datom set or indexes.

#### 1.2 Transaction Typestate Soundness

The transaction protocol uses typestate: Building → Committed → Applied.

- Does the type system actually prevent out-of-order transitions?
- Can you construct a `Transaction<Applied>` without going through `Building` and
  `Committed`? If yes, the typestate is theater, not enforcement.
- What happens on crash between Committed and Applied? Does recovery work?
- Is the typestate publicly exposed or sealed? Can external callers break it?

#### 1.3 Coherence Gate Soundness

`coherence.rs` implements transact-time contradiction detection (Tier 1 exact,
Tier 2 logical).

- What is the false-negative rate? Can contradictions slip through?
- Are Tier 1/2 checks applied to ALL transactions, including schema transactions?
- What about self-referential contradictions (a datom whose assertion contradicts
  the schema that validates it)?
- Performance: is the coherence check O(n) per transaction where n is store size?
  This would make it a bottleneck.

#### 1.4 Query Engine Soundness

The Datalog engine claims CALM compliance, semi-naive evaluation, and stratification.

- Is the fixpoint computation actually a fixed point? Verify termination.
- Are the stratum boundaries correct? Can a query in stratum S0 accidentally
  trigger non-monotonic evaluation?
- Does index selection ever produce different results than full scan?
  (Soundness of index-based evaluation.)
- Are the graph algorithms (PageRank, spectral decomposition, persistent homology)
  numerically stable? What happens with degenerate inputs (empty graph, single node,
  disconnected components)?

#### 1.5 F(S) Fitness Function Soundness

`bilateral.rs` computes F(S) — the fitness function measuring convergence.

- Is F(S) monotonic under valid operations? (Does doing correct work always increase
  F(S), or can it decrease?)
- Are the component weights justified empirically or assumed?
- Does the current implementation match the specification in `spec/`?
- The hypothesis ledger shows mean error 0.521 and a degrading trend. What does
  this mean for the acquisition function's calibration?
- Is the "materialized views vs. batch" divergence (Session 033: F(S) dropped
  0.67→0.58) resolved?

#### 1.6 Schema-as-Data Bootstrap Soundness

The genesis transaction installs 19 axiomatic meta-schema attributes.

- Is the genesis transaction deterministic? (INV-STORE-008)
- Can the meta-schema attributes be retracted? If yes, is the system stable?
- Does the 6-layer schema architecture (L0 meta-schema → L5 future) actually
  enforce layering, or is it just convention?

**Deliverable for Phase 1**: A table of every formal guarantee the system claims,
whether it is proven (Kani), property-tested (proptest), unit-tested, or unverified.
For each unverified guarantee, state the risk level (critical/high/medium/low) and
what verification technique would close the gap.

---

### Phase 2: Architectural Audit (High DoF — Analyze)

**Goal**: Determine whether the architecture is optimal for the stated goals.
Not "does it work" but "is there a better structure that preserves the invariants
with less complexity, better performance, or cleaner separation?"

#### 2.1 Crate Boundary Analysis

The system has two crates: `braid-kernel` (library) and `braid` (binary).

- Is the kernel genuinely substrate-independent (C8)? Search for any DDIS-specific
  concepts, hardcoded methodology assumptions, or software-project-specific logic
  in `braid-kernel/`. Every violation is a defect.
- Is the dependency direction correct? Does `braid-kernel` depend on anything
  it shouldn't? Examine `Cargo.toml` dependencies.
- Should there be additional crates? (e.g., separate `braid-query` for the Datalog
  engine, `braid-schema` for schema validation, `braid-coherence` for verification)
- What is the public API surface area? Is it minimal? Are there functions exported
  that should be internal?

#### 2.2 Module Coupling Analysis

Examine the module graph in `braid-kernel/src/`:

- Which modules have the highest fan-in (most dependents)? These are your stability
  requirements — changes here cascade.
- Which modules have the highest fan-out (most dependencies)? These are your
  complexity hotspots — they know too much.
- Are there circular dependencies? (Even if Rust allows them within a crate, they
  indicate architectural coupling.)
- `guidance.rs` is 7,119 lines. `store.rs` is 5,848 lines. `concept.rs` is 5,362
  lines. Are these god modules that should be split?

#### 2.3 Type Algebra Analysis

Apply the cardinality equation: `|YourType| = |ValidStates|`.

- For every major enum (Value, Op, ProvenanceType, etc.): does the type cardinality
  match the valid state count? Where is there excess?
- Are there boolean parameters that should be newtypes or enums?
- Is the error algebra (KernelError, StoreError, SchemaError) correct? Is every
  variant caller-distinguishable and actionable? Or are there catch-all variants
  that hide failure modes?
- Are there stringly-typed APIs that should use refinement newtypes?

#### 2.4 Performance Architecture

Current state: `braid status` runs in ~4s (down from 97s). Store has 108K datoms.

- What is the algorithmic complexity of core operations?
  - `transact()`: Should be O(datoms_in_tx * log(store_size)) for index updates
  - `query()`: Depends on query. What's the worst case?
  - `merge()`: Should be O(|A| + |B|) for set union
  - `F(S) computation`: Currently in `bilateral.rs` — what's the complexity?
- Are there O(n) scans that should be O(log n) index lookups?
  (`bilateral.rs` was noted as having "7 full scans / 0 index lookups" in Session 031)
- Is the LIVE index (materialized views) correctly invalidated on transactions?
- The Session 046 commits reference a "PERF-REGRESSION" epic with 16 tasks.
  What are the regression root causes? Are they architectural or incidental?

#### 2.5 The Three Learning Loops

The system claims three learning loops close the convergence cycle:
1. **Calibration** (OBSERVER-4): predicted vs actual outcomes adjust boundary weights
2. **Structure** (OBSERVER-5): temporal coupling reveals hidden boundaries
3. **Ontology** (OBSERVER-6): observation clustering reveals emergent categories

- Are all three loops actually closed in the implementation? Or are some open?
  (NEG-010: "don't open loops")
- The hypothesis ledger shows mean error 0.521 with degrading trend. This suggests
  Loop 1 (calibration) may not be converging. Why?
- The concept system scores 3-4/10 (DOGFOOD-2). This suggests Loop 3 (ontology
  discovery) is not working. The identified bottleneck is the hash embedder.
  Is this a fundamental architectural problem or a fixable implementation issue?

**Deliverable for Phase 2**: An architectural health assessment with:
- Dependency graph of all kernel modules
- C8 violation inventory (every kernel function that assumes a specific domain)
- Module size/complexity hotspots with recommended decompositions
- Performance bottleneck inventory with algorithmic complexity for each core path
- Learning loop closure status (open/partially closed/closed) with evidence

---

### Phase 3: Coherence Audit (High DoF — Cross-Reference)

**Goal**: Determine the alignment between specification, implementation, and
empirical behavior across all three layers.

#### 3.1 Spec → Implementation Traceability

For each namespace (STORE, LAYOUT, SCHEMA, QUERY, RESOLUTION, HARVEST, SEED,
MERGE, GUIDANCE, INTERFACE, TRILATERAL):

- How many invariants are specified?
- How many are implemented?
- How many have verification at the specified level (V:TYPE, V:PROP, V:KANI)?
- How many are untested?

Produce a coverage matrix:

```
| Namespace   | Specified | Implemented | V:TYPE | V:PROP | V:KANI | Untested | Gap |
|-------------|-----------|-------------|--------|--------|--------|----------|-----|
| STORE       | ...       | ...         | ...    | ...    | ...    | ...      | ... |
| SCHEMA      | ...       | ...         | ...    | ...    | ...    | ...      | ... |
| ...         |           |             |        |        |        |          |     |
```

#### 3.2 Implementation → Specification Traceability (Reverse)

Are there implemented features that have no specification element?

- Search for functions in `braid-kernel/src/` that don't trace to any INV-/ADR-/NEG-
- These are either (a) implementation details that don't need spec coverage, or
  (b) undocumented behavior that may diverge from intent
- Categorize each as: {infrastructure, undocumented-behavior, spec-gap}

#### 3.3 Specification → Specification Coherence

Check the specification itself for internal contradictions:

- Are there invariants that conflict with each other?
- Are there ADRs whose decisions contradict invariant requirements?
- Are there negative cases (NEG-) that aren't covered by any positive invariant?
- Is the specification formalism self-consistent? (Does the meta-spec — how specs
  are written — satisfy its own rules?)

#### 3.4 Empirical → Specification Alignment

The system has 108K datoms, 2040 tests, and a running hypothesis ledger.

- Do the empirical metrics (F(S)=0.77 boundary, M(t), error 0.521) match what
  the specification predicts they should be at this stage?
- Are there empirical anomalies that suggest specification errors?
- The 142 spec↔impl gaps reported by `braid status` — what are the most critical?

**Deliverable for Phase 3**: The coverage matrix above, plus a ranked list of the
10 most critical coherence gaps (those with highest risk × highest blast radius).

---

### Phase 4: Accretive Path Analysis (High DoF — Synthesize)

**Goal**: Given the soundness, architectural, and coherence findings, determine
the maximally accretive path forward — the sequence of work that produces the
highest return on investment toward the stated goals.

#### 4.1 Axiological Alignment Check

Re-read the bedrock vision:
- Braid is infrastructure for organizational learning
- The kernel is substrate (universal); DDIS is application (replaceable)
- Every change must close a loop, not open one
- The three learning loops must converge empirically

Now evaluate the current state:
- Is the work being done aligned with this vision?
- Are there high-effort activities that don't contribute to convergence?
- Are there low-effort activities that would dramatically improve convergence?
- What is the shortest path to a self-calibrating system?

#### 4.2 Critical Path Identification

From the beads issue tracker (`br ready` output) and the braid task store:

- What are the blocking dependencies? What unblocks the most downstream work?
- What is the critical path to Stage 0 completion?
- What is the critical path to the first self-calibration cycle?
- What work is "nice to have" vs. "load-bearing"?

#### 4.3 Risk-Adjusted Prioritization

For each candidate next action, evaluate:

```
accretive_value = (convergence_impact × loop_closure_factor) / (effort × risk)
```

Where:
- `convergence_impact`: How much does this move F(S) toward 1.0?
- `loop_closure_factor`: Does this close one of the three learning loops? (2x multiplier)
- `effort`: Person-sessions to complete
- `risk`: Probability of failure or wasted work

Rank all candidate actions by accretive value. The top 5 are your recommended plan.

#### 4.4 Formal Methods Opportunities

Identify where additional formal verification would produce disproportionate value:

- What invariants currently lack Kani proofs but are critical?
- Where would proptest catch bugs that unit tests miss?
- Are there concurrency concerns that Stateright should model?
- Is there unsafe code that needs MIRI verification?
- Where would typestate enforcement replace runtime checks?

#### 4.5 The "One Operation" Test

Braid's thesis: the atomic operation at every level is "observe reality, compare
to model, reduce the discrepancy." For each subsystem:

- Does it perform this operation?
- Does it connect its output to the coherence model?
- If a subsystem produces data that doesn't feed back into a boundary evaluation,
  gradient computation, or calibration measurement — that's an open loop.

List every open loop found.

**Deliverable for Phase 4**: A ranked, risk-adjusted action plan with:
- Top 5 highest-accretive-value actions with justification
- Top 5 open loops that must be closed
- Top 5 formal verification gaps that should be addressed
- The recommended session plan for the next 3-5 sessions

---

### Phase 5: Synthesis and Report (Low DoF — Mechanize)

Compile your findings into a structured report with these exact sections:

```
## 1. Executive Summary
   - One-paragraph assessment of project health
   - The single most important finding
   - The single most important recommendation

## 2. Soundness Findings
   - Table: [Guarantee | Claim Source | Verification Level | Status | Risk]
   - Critical unsoundnesses (if any)
   - Verification gaps ranked by risk

## 3. Architectural Findings
   - C8 violations inventory
   - Module health assessment (coupling, complexity, decomposition needs)
   - Performance architecture assessment
   - Learning loop closure status

## 4. Coherence Matrix
   - Spec → Implementation coverage table
   - Implementation → Spec reverse traceability
   - Top 10 coherence gaps by criticality

## 5. Accretive Action Plan
   - Ranked list of recommended actions with:
     - Action description
     - Accretive value score
     - Effort estimate (sessions)
     - Dependencies
     - Risk factors
     - Expected impact on F(S), learning loops, and convergence

## 6. Open Loops Inventory
   - Every subsystem that produces data without feeding it back
   - Recommended closure mechanism for each

## 7. Formal Verification Roadmap
   - Current coverage: what is proven, tested, or unverified
   - Recommended additions ranked by risk × impact
   - Specific Kani harness, proptest property, or Stateright model for each

## 8. Risks and Concerns
   - Architectural risks (things that could require major rework)
   - Convergence risks (things that could prevent the learning loops from closing)
   - Timeline risks (things that could delay Stage 0 completion)
```

---

### Activation Guidance

As you work through these phases, keep these principles active:

**On reasoning mode**: Phases 1-2 require formal/mathematical reasoning — you are
discovering structure, not executing a checklist. Phase 3 requires systematic
cross-referencing — exhaustive, not creative. Phase 4 requires synthesis — combining
findings into actionable recommendations. Phase 5 requires mechanical compilation.
Do not mix modes within a phase.

**On depth vs. breadth**: For soundness (Phase 1), go deep on the 6 specific
concerns listed. For architecture (Phase 2), go broad across all modules then
deep on the hotspots. For coherence (Phase 3), be exhaustive. For accretiveness
(Phase 4), be selective — only the highest-impact items matter.

**On the substrate question**: The most common and most serious error in this
project is violating C8 (substrate independence). Every time you find kernel code
that assumes DDIS methodology, software development, or any specific domain — flag
it. This is the architectural cancer that, if left unchecked, prevents the system
from being what it claims to be: a universal epistemology runtime.

**On open loops**: The second most serious error is NEG-010 — producing data that
doesn't feed back into coherence measurement. Every feature, every metric, every
computation should either (a) detect divergence, (b) measure coherence, or
(c) guide toward convergence. If it does none of these, it's either infrastructure
(acceptable) or an open loop (defect).

**On calibration degradation**: The hypothesis ledger shows degrading calibration
(mean error 0.521, trend: degrading). This is an empirical signal that something
is wrong with the acquisition function. Diagnosing this is high-priority because
it undermines the system's ability to self-improve.

**On the concept system**: DOGFOOD-2 scored 3.48/10. The hash embedder is identified
as the bottleneck. Determine whether this is (a) a fixable implementation issue
(replace hash embeddings with real embeddings), (b) an architectural issue (the
concept system's design is wrong), or (c) an expectations issue (3.48/10 is actually
reasonable for the current stage and the scoring rubric is miscalibrated).

---

### Constraints

- Do not propose changes to code you haven't read.
- Do not relitigate settled ADRs (NEG-002) unless you find a formal contradiction.
- Do not produce aspirational stubs or TODOs (NEG-001).
- Do not add DDIS-specific logic to kernel recommendations (C8).
- Do not recommend optimizations before correctness is established (NEG-003).
- Every recommendation must trace to a finding. No recommendations from thin air.
- Distinguish facts (what you observed) from interpretations (what you conclude)
  from recommendations (what you propose).

---

### Meta: Why This Audit Structure

This prompt is itself an instance of the spec-first methodology:

- **Phase 1 (Soundness)** = formalize: does the algebraic foundation hold?
- **Phase 2 (Architecture)** = derive: given soundness, is the structure optimal?
- **Phase 3 (Coherence)** = specify: are spec/impl/behavior aligned?
- **Phase 4 (Accretiveness)** = implement: what's the optimal next work?
- **Phase 5 (Report)** = verify: compile findings into a falsifiable deliverable

The phases are DoF-separated: Phases 1-2 are high-DoF discovery, Phase 3 is
systematic enumeration, Phase 4 is synthesis, Phase 5 is mechanical. This avoids
the mid-DoF saddle where analysis becomes shallow because it's trying to discover
and execute simultaneously.

The constraints are minimal (7 items — well under k*). Each earns its place:
removing any one would measurably reduce audit quality. The demonstrations (Phase 1
specific concerns, Phase 3 coverage matrix template) encode format expectations
more efficiently than additional constraints would.

---

*This audit prompt traces to: SEED.md (all sections), C1-C8 (hard constraints),
NEG-001 through NEG-010 (negative cases), ADR-FOUNDATION-012 (substrate/application
separation), ADR-FOUNDATION-014 (convergence thesis — every change closes a loop).
It is falsifiable: an audit that follows this prompt but produces a report with
empty sections, unsubstantiated recommendations, or findings that don't trace to
evidence has failed.*
