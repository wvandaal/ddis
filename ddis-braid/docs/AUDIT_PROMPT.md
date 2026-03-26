# Braid Full Formal Audit + Runtime Performance Assessment

> **What this is**: An optimized prompt for a comprehensive, lab-grade audit of the Braid project.
> Designed per `prompt-optimization` (DoF-calibrated, demonstration-driven, k*-managed),
> `spec-first-design` (formalize the audit itself), `rust-formal-engineering` (Curry-Howard,
> cardinality, cleanroom three-box), and `skill-composition` (one reasoning mode per phase).
>
> **How to execute**: Give this prompt to Claude Opus 4.6 in a fresh conversation with the
> full project context loaded (CLAUDE.md + SEED.md + working directory = `/data/projects/ddis/ddis-braid/`).
> Execute phases sequentially. Launch parallel subagents only within a phase, never across phases.

---

## Mission

You are performing a **formal Fagan-class inspection** of the Braid project: a Rust runtime
for formal epistemology (~117K LOC, ~2,083 tests, 22 spec files, 16 guide files, 2 crates).

Your mandate: produce a zero-defect assessment that a principal engineer would stake their
reputation on. This means you will discover things the authors missed. You will find contradictions
they didn't notice. You will measure fidelity they assumed. You will surface axiological drift
they couldn't see from inside the system.

**The project's true north**: Braid is infrastructure for organizational learning — not a
software tool. The kernel is universal substrate; DDIS is the first application (replaceable
via C8). Every change must close a loop (ADR-FOUNDATION-014). The atomic operation at every
level: observe reality, compare to model, reduce discrepancy.

**Your true north**: Every finding must be *evidenced* (cite file:line), *classified* (severity
+ category), *actionable* (specific remediation), and *traced* (to which invariant, ADR, or
constraint it violates). Findings without evidence are opinions. Opinions are not findings.

---

## Demonstration

This is what an excellent audit finding looks like. It encodes the format, depth, rigor, and
traceability expected of every finding you produce.

```
FINDING F-001: Bare retraction invisible to LIVE view
  Severity: P0-CRITICAL | Category: CORRECTNESS
  Evidence: crates/braid-kernel/src/store.rs:1847 — live_view() filters datoms by
    checking only for the latest assertion per [e,a]. A retraction datom with no
    subsequent assertion causes the attribute to disappear from LIVE view, but the
    retraction itself is not surfaced. Compare spec/01-store.md INV-STORE-017
    ("retractions are first-class datoms visible in all views") — the LIVE view
    implementation violates this invariant.
  Impact: Any boundary check running against the LIVE view will miss retracted
    attributes entirely, producing false-positive coherence scores. This undermines
    F(S) accuracy (TRILATERAL namespace) and creates a silent data integrity gap
    that compounds over time as more retractions accumulate.
  Traces to: INV-STORE-017 (violated), C1 (append-only — retractions must be
    visible, not hidden), ADR-FOUNDATION-014 (open loop — retraction data produced
    but not consumed by the coherence model).
  Remediation: live_view() must include retraction datoms in its output, or the
    coherence engine must query the full historical view for boundary checks. The
    former is simpler and preserves the single-view invariant. Estimated: ~20 LOC
    change in store.rs + ~5 new test cases for retraction visibility.
  Confidence: 0.95 (verified by code reading; not yet confirmed via test failure)
```

Do NOT produce findings less rigorous than this. If you cannot cite file:line evidence,
state that explicitly and classify the finding as UNVERIFIED.

---

## Phase 1: Deep Internalization (Very High DoF)

**Reasoning mode**: Formal/Algebraic — discover the system's structure.
**Goal**: Build a complete mental model. You cannot audit what you do not understand.

### 1.1 Read the Foundations

Read in this exact order. Do not skip. Do not skim.

1. `CLAUDE.md` (this project's AGENTS.md) — constraints C1–C8, negative cases NEG-001–010, reconciliation taxonomy, core abstractions
2. `SEED.md` — 11 sections: what DDIS is, the divergence problem, specification formalism, datom abstraction, harvest/seed lifecycle, reconciliation mechanisms, self-improvement loop, interface principles, existing codebase, staged roadmap, design rationale
3. `spec/README.md` — master index, wave grouping, element counts, reading order
4. `docs/guide/README.md` — build order, cognitive phase protocol, spec cross-reference
5. `docs/design/ADRS.md` — all settled design decisions with rationale

### 1.2 Internalize the Algebraic Core

For each of these, answer the question — do not just read the file:

| File | Activation Question |
|------|-------------------|
| `spec/01-store.md` | What is the algebraic structure of the datom store? What monoid does it form? |
| `spec/02-schema.md` | How does the schema bootstrap itself? What is the fixed-point? |
| `spec/03-query.md` | What is the stratification lattice? Where does CALM compliance break? |
| `spec/04-resolution.md` | What partial order governs conflict resolution? Where are the joins? |
| `spec/18-trilateral.md` | What is the coherence metric's mathematical structure? Is it a proper metric? |
| `spec/07-merge.md` | Is the merge operation a join-semilattice homomorphism? Prove or disprove. |
| `spec/12-guidance.md` | What is the control-theoretic model? Where are the feedback loops? |

### 1.3 Internalize the Implementation

Explore the codebase — not sequentially, but structurally. For each module, ask:
- What algebraic structure does this implement?
- What invariants does its type system encode at compile time vs. runtime?
- What is the cardinality of its core types? Does it match the valid state count?

Start with the kernel core:
```
crates/braid-kernel/src/datom.rs     — the atomic unit
crates/braid-kernel/src/store.rs     — the grow-only set (P(D), ∪)
crates/braid-kernel/src/schema.rs    — schema-as-data bootstrap
crates/braid-kernel/src/query/       — Datalog evaluator
crates/braid-kernel/src/resolution.rs — conflict resolution lattice
crates/braid-kernel/src/trilateral.rs — coherence geometry
crates/braid-kernel/src/guidance.rs   — routing and gradient computation
crates/braid-kernel/src/topology.rs   — spectral coordination
```

Then the lifecycle layer:
```
crates/braid-kernel/src/harvest.rs    — end-of-session extraction
crates/braid-kernel/src/seed.rs       — start-of-session assembly
crates/braid-kernel/src/merge.rs      — CRDT set union
crates/braid-kernel/src/bilateral.rs  — coherence scanning
crates/braid-kernel/src/witness.rs    — falsification protocol
```

Then the CLI layer:
```
crates/braid/src/commands/            — 26 command modules
crates/braid/src/live_store.rs        — hot-reload store wrapper
crates/braid/src/bootstrap.rs         — schema + policy bootstrap
crates/braid/src/inject.rs            — AGENTS.md dynamic generation
```

**Do NOT edit any files during this phase. Read, trace, understand.**

### 1.4 Checkpoint

Before proceeding, answer these questions (write your answers — they calibrate Phase 2):

1. What are the three learning loops and where is each implemented?
2. What is the observation-projection duality and how does the code realize it?
3. What is C8 (substrate independence) and which kernel modules, if any, violate it?
4. What is the reconciliation taxonomy (8 divergence types) and which types have working implementations?
5. What is the current F(S) score and what does each component measure?
6. Where are the open loops (data produced but not consumed by the coherence model)?

If you cannot answer all six with file:line evidence, you have not internalized deeply enough.
Go back and read more.

---

## Phase 2: Spec-Implementation Fidelity Matrix (High DoF → Low DoF)

**Reasoning mode**: Cross-referencing — trace every formal claim to its evidence.
**Goal**: Quantify exactly how faithful the implementation is to the specification.

### 2.1 Invariant Coverage Audit

For EVERY invariant in `spec/` (all INV-* elements across all 22 files):

| INV ID | Spec File:Line | Implemented? | Implementation File:Line | Tested? | Test File:Line | Type-Encoded? | Notes |
|--------|---------------|-------------|------------------------|---------|---------------|--------------|-------|
| INV-STORE-001 | spec/01-store.md:NN | Yes/Partial/No | store.rs:NN | Yes/Partial/No | store.rs:test_NN | Compile-time/Runtime/Neither | ... |

**Activation**: For each invariant, ask: "Does the Rust type system prove this at compile time
(Curry-Howard), or is it a runtime proof obligation (test), or is it merely hoped-for?"

This is the most labor-intensive phase. Do it methodically. Do not sample — cover ALL invariants.
If there are too many to cover in one pass, partition by namespace and use parallel subagents
(one per namespace wave).

### 2.2 ADR Traceability Audit

For every ADR in `docs/design/ADRS.md` and `spec/`:

- Is the decision actually reflected in the implementation?
- Are rejected alternatives truly absent from the code?
- Does the rationale still hold given the current implementation state?

### 2.3 Negative Case Verification

For every NEG-* in `spec/` and CLAUDE.md:

- Has the negative case been violated in the current codebase?
- Is there a test that would catch the violation?
- Are there code patterns that are *close* to violating it?

### 2.4 Cross-Reference Integrity

- Every `spec/` element should trace to `SEED.md`. Verify the traces.
- Every `docs/guide/` file should correspond to a `spec/` file (per the cross-reference table). Verify alignment.
- Every guide instruction should match what the code actually does. Flag divergences.

### 2.5 Constraint Verification (C1–C8)

For each hard constraint:

| Constraint | Description | Violations Found | Evidence |
|-----------|-------------|-----------------|----------|
| C1 | Append-only store | ... | file:line |
| C2 | Identity by content | ... | file:line |
| C3 | Schema-as-data | ... | file:line |
| C4 | CRDT merge by set union | ... | file:line |
| C5 | Traceability | ... | file:line |
| C6 | Falsifiability | ... | file:line |
| C7 | Self-bootstrap | ... | file:line |
| C8 | Substrate independence | ... | file:line |

**C8 deserves special attention**. For every function in `braid-kernel`, ask: "Would this make
sense if braid managed a React project with Jest tests and Jira tickets?" If no, it violates C8.

---

## Phase 3: Runtime Performance Assessment (Low DoF — Measurement)

**Reasoning mode**: Empirical — measure, don't guess.
**Goal**: Profile hot paths, identify bottlenecks, quantify where time goes.

### 3.1 Build and Profile

```bash
cargo build --release 2>&1  # Note any warnings
```

Profile the critical paths:
```bash
time ./target/release/braid status          # Should be <1s (was 97s, fixed to 3s)
time ./target/release/braid harvest --dry-run
time ./target/release/braid seed --task "test"
time ./target/release/braid guidance
time ./target/release/braid query '[:find ?e :where [?e :db/type "task"]]'
```

### 3.2 Algorithmic Complexity Audit

For each hot path identified above, trace the call chain and answer:

- What is the time complexity? (cite the dominant loop/recursion)
- What is the space complexity? (cite the dominant allocation)
- Are there unnecessary allocations? (clone(), to_string(), Vec collect where iter suffices)
- Are indexes used effectively? (EAVT/AEVT/VAET/AVET — which queries use which index?)
- Is there lock contention? (Mutex, RwLock — what is the critical section duration?)
- Are there cache invalidation pathologies? (LiveStore refresh, materialized views)

### 3.3 Store Scaling Analysis

The store currently holds ~108K datoms. Answer:

- What is the growth rate per session? (estimate from transaction log)
- At what datom count do current algorithms degrade? (linear scans, O(n) filters)
- Which operations are O(n) that should be O(log n) or O(1)?
- What is the serialization/deserialization cost? (store load time, format efficiency)

### 3.4 Memory Profile

- What is the peak memory footprint during `braid status`?
- Are there owned Strings where &str or Cow would suffice?
- Are there Vec allocations where SmallVec or ArrayVec would eliminate heap allocation?
- Is there data duplication between indexes? (same datom stored in multiple index structures)

### 3.5 Performance Findings

For each performance finding, use this format:
```
PERF-NNN: <title>
  Severity: P0-BLOCKING / P1-HIGH / P2-MEDIUM / P3-LOW
  Location: file:line (call chain)
  Current: O(?) with measured time of Xms at N datoms
  Target: O(?) — achievable via <specific technique>
  Impact: <what user-visible behavior this affects>
  Evidence: <how you measured or derived this>
```

---

## Phase 4: Formal Audit — Fagan/IEEE Synthesis (Mixed DoF)

**Reasoning mode**: Adversarial — assume the system has defects and find them.

### 4.1 Fagan Inspection Protocol

Apply the full Fagan inspection sequence:

1. **Preparation** (done in Phases 1–3): You have internalized the spec, implementation, and performance characteristics.

2. **Overview**: Summarize your understanding of each subsystem in one paragraph. Flag any subsystem you do not fully understand — incomplete understanding is itself a finding.

3. **Inspection**: For each of the following categories, search actively for defects:

   **Correctness Defects**
   - Logic errors in query evaluation (stratification, negation, aggregation)
   - State machine violations (are there unreachable states? missing transitions?)
   - Off-by-one in index lookups (EAVT/AEVT/VAET/AVET boundary conditions)
   - Race conditions in LiveStore refresh, daemon communication
   - Unsound `unsafe` blocks (if any — check with `grep -r "unsafe"`)

   **Soundness Defects (Formal Methods)**
   - Invariants claimed but not enforced (aspirational invariants — NEG-007)
   - Proof sketches that don't actually prove what they claim
   - Mathematical structures claimed but not verified (e.g., "CRDT merge is set union" — is it actually commutative, associative, idempotent in the implementation?)
   - Monotonicity claims that are false (F(S) monotonicity was previously falsified — is the fix correct?)

   **Architectural Defects**
   - C8 violations: kernel code that assumes DDIS methodology
   - Open loops: data produced but never consumed by the coherence model (ADR-FOUNDATION-014)
   - Circular dependencies between modules
   - God objects (modules doing too many things — check file sizes and responsibility count)
   - Dead code (functions defined but never called)

   **Type System Defects (Curry-Howard Lens)**
   - Types that admit invalid states (excess cardinality)
   - Boolean blindness (functions taking multiple bool parameters)
   - Stringly-typed APIs (String where a newtype would enforce invariants)
   - Missing newtypes at domain boundaries (EntityId as raw u64 without validation?)
   - Error types that aren't caller-distinguishable

   **Specification Defects**
   - Contradictions between spec files (invariant in file A contradicts invariant in file B)
   - Undefined terms (spec uses a concept without defining it)
   - Ambiguous invariants (multiple valid interpretations)
   - Missing invariants (behaviors observed in code with no corresponding spec element)
   - Circular dependencies between invariants

   **Documentation Defects**
   - Guide instructions that don't match the implementation
   - Stale references to moved/renamed files or functions
   - Worked examples that no longer compile or produce different output

4. **Rework List**: Compile all findings into a prioritized remediation plan.

5. **Follow-Up**: For each P0/P1 finding, specify the verification method that would confirm the fix.

### 4.2 IEEE Walkthrough Elements

In addition to the Fagan inspection, apply IEEE walkthrough criteria:

- **Completeness**: Are there behaviors the system exhibits that have no specification?
- **Consistency**: Do all spec files use terms the same way? Do code comments match spec definitions?
- **Testability**: For each invariant, is the falsification condition actually testable with the current test infrastructure?
- **Feasibility**: Are there spec elements that are technically impossible to implement as stated?
- **Maintainability**: Can a new developer (or agent) understand and modify the code? Where are the knowledge cliffs?

### 4.3 Contradiction Detection (5-Tier)

Apply DDIS's own 5-tier contradiction detection to its own specification:

1. **Direct contradiction**: INV-A says X, INV-B says not-X
2. **Implication contradiction**: INV-A implies P, INV-B implies not-P
3. **Boundary contradiction**: INV-A's domain overlaps INV-B's in a conflicting way
4. **Temporal contradiction**: INV-A holds at time T but INV-B requires not-INV-A at time T
5. **Axiological contradiction**: INV-A optimizes for goal G1, INV-B optimizes for conflicting goal G2

---

## Phase 5: Axiological Synthesis (Very High DoF)

**Reasoning mode**: Philosophical/Strategic — align findings with the project's deepest goals.

### 5.1 True North Alignment

Answer these questions with evidence:

1. **Is Braid actually infrastructure for organizational learning, or has it drifted into being a software development tool?** Cite specific code patterns or features that answer this.

2. **Does the kernel actually maintain substrate independence (C8)?** List every violation, no matter how small.

3. **Does the system actually close all loops (ADR-FOUNDATION-014)?** Map every data flow and identify any that terminate without feeding back into the coherence model.

4. **Are the three learning loops (calibration, structure discovery, ontology discovery) actually implemented, or are they aspirational?** For each loop, trace the data flow from input through processing to output.

5. **Is the convergence thesis (ADR-FOUNDATION-014: the system converges toward truth through iterative discrepancy reduction) supported by the implementation?** What evidence exists that F(S) actually improves over time?

### 5.2 The Maximally Accretive Path Forward

Based on everything you have found, answer:

1. **What is the single highest-leverage change** that would move the project closest to its true north?

2. **What are the top 5 findings** that, if left unaddressed, will compound into systemic problems?

3. **What is the optimal execution order** for all remediation work? (Consider: dependency chains, risk reduction rate, axiological alignment)

4. **What capabilities are missing entirely** that the spec describes but the implementation doesn't even stub?

5. **What is your honest assessment** of the project's current maturity level (0–10) across:
   - Correctness (do the implemented features work correctly?)
   - Completeness (how much of the spec is implemented?)
   - Performance (is it fast enough for real use?)
   - Architecture (is the structure clean and maintainable?)
   - Formal rigor (does the code actually prove what the spec claims?)
   - Axiological alignment (is the implementation serving the true north?)

### 5.3 Premortem on the Audit Itself

Before finalizing, run a premortem on your own audit:

- What could you have missed? (Name specific areas of the codebase you explored less thoroughly.)
- Where might your assessment be wrong? (Name specific findings where your confidence is < 0.8.)
- What biases might have shaped your findings? (Recency? Anchoring on the first defect found? Over-weighting visible issues?)

---

## Output Specification

### Finding Format

Every finding follows the demonstrated format (see Demonstration section above). Minimum fields:
- **ID**: F-NNN (sequential), PERF-NNN (performance), SPEC-NNN (specification), ARCH-NNN (architecture)
- **Severity**: P0-CRITICAL, P1-HIGH, P2-MEDIUM, P3-LOW, P4-INFORMATIONAL
- **Category**: CORRECTNESS, SOUNDNESS, ARCHITECTURE, PERFORMANCE, SPECIFICATION, DOCUMENTATION, AXIOLOGICAL
- **Evidence**: file:line citation (mandatory for P0–P2; "UNVERIFIED" permitted for P3–P4)
- **Impact**: What breaks, degrades, or drifts if this is not fixed
- **Traces to**: Which INV, ADR, NEG, or constraint this relates to
- **Remediation**: Specific action (not "fix this" — state what the fix is)
- **Confidence**: 0.0–1.0

### Summary Tables

Produce these summary tables at the end:

1. **Finding Summary**: Count by severity and category
2. **Invariant Coverage Matrix**: % implemented, % tested, % type-encoded per namespace
3. **Performance Summary**: Top 10 hot paths with measured/estimated times
4. **Constraint Compliance**: C1–C8 pass/fail with evidence count
5. **Axiological Score Card**: The 6-dimension maturity assessment from Phase 5.2
6. **Priority Remediation Queue**: Top 20 findings ordered by (severity × impact × tractability)

### Deliverable Structure

```
## Executive Summary (500 words max)
## Phase 1 Checkpoint Answers (6 questions with file:line evidence)
## Phase 2 Fidelity Matrix (tables + findings)
## Phase 3 Performance Assessment (profiles + findings)
## Phase 4 Formal Audit (Fagan findings + IEEE walkthrough + contradiction analysis)
## Phase 5 Axiological Synthesis (true north + path forward + premortem)
## Appendix A: Complete Finding Registry (all F-NNN, PERF-NNN, SPEC-NNN, ARCH-NNN)
## Appendix B: Invariant Coverage Matrix (full)
## Appendix C: Recommended Execution Plan (dependency-ordered waves)
```

---

## Execution Architecture

### Subagent Strategy (If Using Parallel Agents)

**Wave 0 — Internalization (sequential, single agent)**:
Phase 1 in its entirety. One agent. Cannot be parallelized — understanding must be holistic.

**Wave 1 — Fidelity Analysis (parallel, 4 agents by namespace wave)**:
- Agent 1: Wave 1 namespaces (STORE, LAYOUT, SCHEMA, QUERY, RESOLUTION)
- Agent 2: Wave 2 namespaces (HARVEST, SEED, MERGE, SYNC)
- Agent 3: Wave 3 namespaces (SIGNAL, BILATERAL, DELIBERATION, GUIDANCE, BUDGET, INTERFACE)
- Agent 4: Wave 4 namespaces (TRILATERAL, TOPOLOGY, COHERENCE, WITNESS, REFLEXIVE)

Each agent produces a partial invariant coverage matrix for their namespaces.

**Wave 2 — Deep Audit (parallel, 3 agents by domain)**:
- Agent 5: Correctness + Soundness (Fagan categories 1–2)
- Agent 6: Architecture + Type System (Fagan categories 3–4)
- Agent 7: Performance Assessment (Phase 3 in its entirety)

**Wave 3 — Synthesis (sequential, single agent)**:
Phase 5. Receives all prior outputs. Produces the final deliverable.

### Tool Usage

- `Read` for all file reading — never `cat`
- `Grep` for all content search — never `grep` or `rg`
- `Glob` for file discovery — never `find`
- `Bash` only for: `cargo check`, `cargo test`, `cargo build`, `time` profiling, `git log`
- **Do NOT edit any files** — this is a read-only audit

### Skill Loading (Per Phase)

Following `skill-composition` protocol — one reasoning mode per phase:

| Phase | Load Skill | Shed When |
|-------|-----------|-----------|
| 1 (Internalization) | None — raw understanding, no meta-framework | — |
| 2 (Fidelity) | `spec-first-design` (cross-referencing lens) | Phase 2 complete |
| 3 (Performance) | `rust-formal-engineering` (allocation/complexity lens) | Phase 3 complete |
| 4 (Formal Audit) | None — synthesis of Phases 1–3, no new framework | — |
| 5 (Axiological) | None — pure judgment, all frameworks absorbed | — |

---

## Anti-Patterns (What NOT To Do)

1. **Do not produce findings without file:line evidence.** "The store might have issues" is not a finding.
2. **Do not conflate spec aspirations with implementation reality.** The spec says what SHOULD be true; your job is to determine what IS true.
3. **Do not skip the checkpoint in Phase 1.** If you cannot answer the 6 questions, your subsequent analysis will be shallow. Go back.
4. **Do not sample invariants.** Cover ALL of them. This is a Fagan-class inspection, not a spot check.
5. **Do not optimize the prompt mid-execution.** Trust the phase structure. Each phase builds on the previous.
6. **Do not produce a laundry list of trivial findings to pad the count.** 10 P0 findings are worth more than 100 P4 findings. Focus on what matters.
7. **Do not skip the premortem on your own audit.** Intellectual honesty about your blind spots is itself a finding.
