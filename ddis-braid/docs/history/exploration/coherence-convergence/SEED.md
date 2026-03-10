# Conversation Seed: Braid Coherence Engine Architecture

> **Session origin**: 2026-03-03
> **Prior sessions**: "Datomic Implementation in Rust" (original Braid architecture), current session (Datalog vs Prolog → coherence engine → DDIS computational core)
> **Principal**: Willem (wvandaal)
> **Recommended next action**: Feasibility experiment — translate instances of ALL SEVEN DDIS primitive types into logical forms, validate LLM translation fidelity and cross-element coherence checking

---

## 1. What Is Being Built

**DDIS** (Decision-Driven Implementation Specification) is a meta-specification standard Willem created for maintaining verifiable coherence between intent, specification, implementation, and observed behavior — across people, AI agents, and time. It addresses four types of divergence:

- **Axiological**: Intent → Specification (the spec doesn't capture what you actually want)
- **Logical**: Specification → Specification (the spec contradicts itself)
- **Structural**: Specification → Implementation (code doesn't match spec)
- **Behavioral**: Implementation → Observed Behavior (code doesn't do what it claims)

**Braid** is the embedded temporal database that serves as DDIS's knowledge substrate. It replaces a 62,500-line Go CLI backed by a 39-table normalized SQLite schema with an architecturally cleaner system built on datoms.

**The ultimate goal** (from Willem, verbatim): "To be able to say, at any point in a project of any scale, with formal justification rather than subjective confidence: I know what I want. It is logically coherent and internally consistent. This specification is a full and accurate formalization of what I want and how we will build it. The implementation traces back to the specification at every point."

---

## 2. Braid Core Architecture (Settled Decisions)

These are locked commitments from the "Datomic Implementation in Rust" session and the SEED.md spec:

### Data Model
- **Datoms**: `[entity, attribute, value, transaction, operation]` — atomic facts
- **Store**: `(P(D), ∪)` — a grow-only set (G-Set CRDT). Retractions are datoms with `op=retract`. The store never shrinks.
- **Schema-as-data**: Attributes are themselves entities. Schema emerges from usage.
- **Identity by content**: Two agents asserting the same fact produce one datom.

### Five Axioms
- **A1 (Identity)**: A datom is `[e, a, v, tx, op]`. Tx is an entity carrying provenance.
- **A2 (Store)**: Append-only G-Set. All semantic resolution is query-layer.
- **A3 (Snapshots)**: Default query mode is local frontier. Optional sync barriers for consistent cuts.
- **A4 (Queries)**: Monotonic queries run without coordination (CALM theorem). Non-monotonic queries are frontier-relative.
- **A5 (Resolution)**: Per-attribute conflict resolution: lattice, LWW, or multi-value.

### Infrastructure
- **Embedded library** (no daemon process)
- **File-backed** with git as coordination/versioning layer
- **Four indexes**: EAVT, AEVT, VAET, AVET + fifth LIVE index for current-state views
- **Hybrid Logical Clocks** for transaction ordering
- **17 axiomatic meta-schema attributes** in genesis transaction (tx=0)
- **Query engine**: Datomic-style Datalog dialect, semi-naive evaluation, CALM-compliant
- **Six query strata** with 17 named patterns
- **FFI mechanism** at Datalog/imperative boundary (SQ-010): Datalog handles joins, host functions handle computation
- **Implementation language**: Rust

### Harvest/Seed Lifecycle
- The core innovation: conversations are disposable, knowledge is durable
- **Harvest**: At conversation end, extract valuable knowledge into the datom store
- **Seed**: At conversation start, assemble relevant context from the store
- **20–30 turn lifecycle** calibrated to LLM attention degradation
- **Proactive warnings** at context consumption thresholds (70%, 85%, 95%)

### Self-Bootstrap Principle
- The DDIS spec is written using DDIS methodology
- The spec's invariants, ADRs, negative cases become the first datoms in the store
- The system's first act of coherence verification is checking its own specification

---

## 3. The Seven DDIS Primitives

DDIS has seven primitive element types that form a coherence verification machine:

| # | Primitive | ID Pattern | What It Does |
|---|-----------|-----------|--------------|
| 1 | **Invariant (INV)** | INV-{NS}-{NNN} | Falsifiable claim with violation condition. Detects logical + structural divergence. |
| 2 | **ADR** | ADR-{NS}-{NNN} | Decision record: problem, options, decision, rationale, consequences. Prevents axiological divergence. |
| 3 | **Negative Case (NEG)** | NEG-{NS}-{NNN} | Safety property — what the system MUST NOT do. Temporal logic (□ ¬ ...). Bounds solution space. |
| 4 | **Uncertainty Marker (UNC)** | UNC-{NS}-{NNN} | Explicit acknowledgment of what's not settled. Carries confidence (0.0–1.0), impact-if-wrong, resolution path. |
| 5 | **Contradiction Detection** | 5-tier system | Exact → Logical → Semantic → Pragmatic → Axiological. Finds spec-internal conflicts. |
| 6 | **Fitness Function F(S)** | 7 dimensions | Coverage, Coherence, Divergence, Depth, Completeness, Formality, Certainty → single score 0.0–1.0. |
| 7 | **Bilateral Feedback Loop** | Forward + Backward | Does implementation satisfy spec? Does spec accurately describe implementation? Converges when no changes needed. |

**Primitive interaction web:**
```
INV ←contradicts→ INV         (Contradiction Detection finds conflicts)
INV ←justifies→ ADR           (ADRs explain why invariants exist)
NEG ←bounds→ INV              (NEGs constrain how INVs can be satisfied)
UNC ←qualifies→ INV/ADR       (UNCs mark which INVs/ADRs aren't settled)
F(S) ←measures→ all           (Fitness function scores the whole system)
Bilateral ←verifies→ INV+NEG  (Bilateral loop checks INVs/NEGs both directions)
```

---

## 4. The Central Insight: The Coherence Engine IS the Computational Core of DDIS

### 4.1 What The Coherence Engine Is

The coherence engine is not "a checker for invariants." It is **the computational substrate that makes the seven primitives a machine rather than a checklist.** Every relationship arrow in the primitive interaction web becomes a computable predicate. Without it, the primitives are structured documentation. With it, they are a live, self-checking, self-diagnosing specification system.

**Core concept ("type system for specifications")**: Live coherence checking during spec authoring, in natural language. The composition of:
1. **LLM-mediated translation**: Prose spec elements → logical forms (automatic, during `braid transact`)
2. **Coherence engine**: Checks new logical forms against all existing ones (subsecond for hundreds of elements)
3. **Cascade computation**: A single change propagates through the entire dependency web
4. **Actionable diagnosis**: Not just "contradiction found" but full causal chain + resolution options

### 4.2 What Every Primitive Type Contributes to Logical Forms

The logical form is not just "violation conditions for invariants." It's a **uniform extraction of commitments, assumptions, exclusions, prohibitions, and satisfaction conditions** across ALL element types:

| Spec Element | What Gets a Logical Form | Coherence Checks Enabled |
|-------------|--------------------------|--------------------------|
| **Invariant** | Violation condition | Inv ↔ Inv contradiction, Inv ↔ ADR conflict |
| **ADR Commitment** | What the decision commits to | ADR ↔ ADR conflict, ADR ↔ INV conflict |
| **ADR Assumption** | What the decision takes for granted | Assumption validity (does this still hold given current goals?) |
| **ADR Exclusion** | What was explicitly rejected | Exclusion conflict (did a later ADR reintroduce what was rejected?) |
| **Negative Case** | Prohibited state (safety property) | Reachability analysis (do positive commitments entail a prohibited state?) |
| **Uncertainty Marker** | Provisional claim + impact-if-wrong | Uncertainty propagation (settled elements on uncertain foundations) |
| **Goal** | Satisfaction condition | Goal entailment (do commitments still serve this goal?) |

### 4.3 The LLM Translation Is Slot-Filling, Not Arbitrary Logic

The translation prompt is structured, not open-ended:
- For invariants: "extract the violation condition"
- For ADRs: "extract what this COMMITS TO, what it ASSUMES, and what it EXCLUDES"
- For negative cases: "extract the prohibited state"
- For uncertainty markers: "extract the provisional claim and the impact chain"
- For goals: "extract the satisfaction condition"

Slot-filling is one of the most reliable LLM capabilities. Round-trip verification (prose → clause → prose → compare) catches unreliable translations.

### 4.4 The Five Contradiction Tiers Map to Engine Capabilities

| Tier | Description | Detection Method |
|------|-------------|-----------------|
| **1. Exact** | Same attribute, different values | Pure Datalog query. Trivial. |
| **2. Logical** | Mutually exclusive implications | Horn clause contradiction check (Prolog: `incompatible/2`) |
| **3. Semantic** | Different words, same conflict | LLM translation bridges the semantic gap; engine checks the logical gap |
| **4. Pragmatic** | Compatible in isolation, incompatible in practice | **Prolog's strongest territory**: `jointly_violates/3` over compound states |
| **5. Axiological** | Internally consistent but misaligned with goals | Goal entailment checking: `unserved_goal/2` |

### 4.5 The Fitness Function Becomes Partially Computable

| Dimension | Computable by Engine? | How |
|-----------|----------------------|-----|
| **V (Coverage)** | Yes — Datalog | Query goal → invariant dependency graph |
| **C (Coherence)** | **Yes — this IS the engine's core function** | Count of zero contradictions = 1.0 |
| **D (Divergence)** | Partially | Spec-side via bilateral; implementation-side requires runtime |
| **H (Depth)** | Yes — Datalog | Proportion of invariants with falsification + logical form + verification tag |
| **I (Formality)** | Yes — Datalog | Proportion of elements with successful logical form translations |
| **K (Completeness)** | Partially | Known gaps detectable; unknown gaps aren't |
| **U (Certainty)** | Yes — with propagation | Effective confidence (min over dependency chain), not just stated confidence |

F(S) becomes a **live computed metric** that updates on every transact, not a manual assessment.

### 4.6 Spec-Level vs Implementation-Level Checking

The coherence engine catches a class of problems that implementation-level tools (Kani, proptest) fundamentally cannot: contradictions in the spec itself. If the spec permits a prohibited state, the code that faithfully implements the spec will also permit it — and it won't be a "bug." It'll be a correct implementation of a contradictory specification. The coherence engine catches this BEFORE implementation begins.

---

## 5. The Ten Meta-Rules

The coherence engine operates through ten meta-rules organized in three families:

### Contradiction Family (require Prolog for deep checks)

```
MR-1: Commitment Contradiction
  Two active elements have logically incompatible commitments.
  Requires: incompatible/2 (Prolog: unification over hypothetical models)

MR-2: Exclusion Violation
  A later element's commitment entails a pattern that an earlier ADR explicitly rejected.
  Requires: entails/2 (Prolog: logical entailment checking)

MR-3: Negative Case Reachability
  An active commitment entails a state that a negative case prohibits.
  Requires: entails/2 (Prolog)

MR-4: Pragmatic Contradiction
  Two commitments are individually compatible but jointly unsatisfiable 
  given a system constraint.
  Requires: jointly_violates/3 (Prolog: compound satisfiability)
  THIS IS THE HARDEST AND MOST VALUABLE CHECK.
```

### Drift Family (mostly Datalog-expressible)

```
MR-5: Assumption Invalidation
  An ADR's assumption no longer holds (e.g., goal it assumed active was retracted).
  May require Prolog for holds/1 evaluation.

MR-6: Dependency Orphaning
  An active element depends on an element that has been superseded or retracted.
  Pure Datalog.

MR-7: Uncertainty Propagation / False Certainty
  An element appears settled (confidence > 0.9) but depends on uncertain ground 
  (effective confidence < 0.7 when propagated through dependency chain).
  Pure Datalog (min-confidence computation over dependency graph).
```

### Coverage Family (Datalog-expressible)

```
MR-8: Goal Entailment Failure
  An active goal's satisfaction condition is not entailed by active commitments.
  Requires Prolog for entailment check.

MR-9: Coverage Gap
  A goal has no supporting invariants in the dependency graph.
  Pure Datalog.

MR-10: Formality Gap
  An invariant has no logical form (no machine-checkable representation).
  Pure Datalog.
```

---

## 6. Cascade Analysis: How Meta-Rules Compose

### Key Insight: Meta-rules form a fixed-point system

When one meta-rule fires, it produces new facts (contradictions, drift signals) that can trigger other meta-rules. The process continues until no new facts are produced — a fixed point. Same structure as Datalog's semi-naive evaluation, but over coherence facts rather than datom facts.

### Cascade Pattern 1: Goal Change → Multi-Rule Cascade

A single goal retraction can cascade through five meta-rules: assumption invalidation (MR-5) → commitment contradiction (MR-1) → negative case reachability (MR-3) → uncertainty propagation (MR-7) → coverage gap (MR-9). Six elements affected, full resolution options generated, computed in subseconds.

**Concrete example**: Retracting goal "temporal completeness" → ADR-003 (append-only) assumption invalidated → INV-STORE-001 undermined → if ADR-089 (compaction) exists, it now contradicts ADR-003 → NEG-STORE-001 (no deletion) becomes reachable via compaction → all elements depending on INV-STORE-001 have effective confidence 0.0.

### Cascade Pattern 2: Uncertainty Resolution Creates New Contradictions

Resolving an uncertainty can reveal contradictions that were previously invisible. While sync barrier latency was uncertain, INV-QUERY-001 (bounded time) and INV-QUERY-002 (CALM compliance) coexisted. Resolving to "unbounded latency" creates a pragmatic contradiction.

**Critical implication**: Every uncertainty resolution must trigger a full coherence re-check. The resolution may change the logical landscape.

### Cascade Pattern 3: Constructive Goal Entailment

The engine operates generatively — discovering that existing commitments already serve new goals without human re-derivation. Asserting goal "offline operation" → engine discovers G-Set merge (ADR-STORE-001) is coordination-free → concludes offline merge already supported → partially closes coverage gap through logical inference.

### Cascade Output Format

```
⚠ COHERENCE CASCADE (triggered by: retraction of Goal G1)

  1. ADR-003 (append-only store) — assumption invalidated
     ADR-003 assumes temporal_completeness is a priority. G1 retracted.

  2. INV-STORE-001 (monotonic store) — undermined
     Depends on ADR-003. Effective confidence: 0.0

  3. INV-LIVE-001 (LIVE index consistency) — undermined
     Depends on INV-STORE-001. Effective confidence: 0.0

  4. ADR-089 (compaction) — conflicts with ADR-003
     ADR-003 undermined but still active. Direct contradiction.

  5. NEG-STORE-001 (no datom deletion) — reachable
     ADR-089's compaction entails prohibited state.

  Resolution required:
    [a] Supersede ADR-003, relax INV-STORE-001 and NEG-STORE-001 (align with G2)
    [b] Retract ADR-089, find alternative for storage efficiency
    [c] Scope: permit compaction only for non-referenced datoms
  
  Affected elements: 6  |  Run `braid diagnose --cascade G1` for proof tree.
```

---

## 7. Engine Architecture

### Two-Layer Design

```
┌─────────────────────────────────────────────────────┐
│              Coherence Engine                        │
│                                                     │
│  ┌──────────────────────────────────────────────┐   │
│  │  Datalog Layer (fixed-point, terminating)     │   │
│  │  • Cascade computation                        │   │
│  │  • Coverage, dependency, uncertainty graphs    │   │
│  │  • F(S) computation (5 of 7 dimensions)       │   │
│  │  • Meta-rules 6, 7, 9, 10 (pure Datalog)     │   │
│  │  Calls down for: incompatible/2, entails/2,   │   │
│  │    jointly_violates/3, goal_search/2           │   │
│  └───────────────┬──────────────────────────────┘   │
│                  │ (FFI boundary — SQ-010)            │
│  ┌───────────────▼──────────────────────────────┐   │
│  │  Prolog Layer (goal-directed, fuel-bounded)   │   │
│  │  • Unification over logical forms              │   │
│  │  • Satisfiability / counterexample search      │   │
│  │  • Goal backward chaining                      │   │
│  │  • Resolution generation                       │   │
│  │  • Meta-rules 1, 2, 3, 4, 5, 8               │   │
│  │  • ~2,000–3,000 lines Rust (cleanroom)         │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

**Datalog detects and computes the structural graph. Prolog resolves the logical questions Datalog can identify but can't answer.**

### Integration
- FFI via SQ-010 (existing mechanism)
- Fuel-bounded Prolog evaluation (fuel monad, deterministic, timeout → `:timeout` not `:passed`)
- Conservative: false negatives safe, false positives impossible by construction

---

## 8. The Predicate Ontology (Four Layers)

### Layer 0: Spec Element Facts (from datom store)
```
element/2, active/1, superseded/2, retracted/2,
depends_on/2, traces_to/2, asserted_at/2, asserted_by/2
```

### Layer 1: Logical Forms (LLM-extracted, per element type)
```
violation/2          — invariant falsification conditions
commitment/2         — ADR positive commitments
assumes/2            — ADR assumptions
excludes/2           — ADR rejected alternatives
prohibited/2         — negative case safety properties
uncertain/3          — uncertainty marker qualifications
provisional_claim/2  — uncertain claims
impact_if_wrong/2    — uncertainty impact chains
goal/2               — goal satisfaction conditions
```

### Layer 2: Meta-Rules (10 rules, 3 families)
```
contradiction/4, pragmatic_contradiction/4, neg_reachable/3,
drift/3, false_certainty/3,
unserved_goal/2, coverage_gap/1, formality_gap/1
```

### Layer 3: Computed Metrics + Resolution
```
coverage_score/1, coherence_score/1, depth_score/1,
formality_score/1, certainty_score/1, fitness/1,
resolution/2, resolution_cost/2
```

---

## 9. Self-Referential Coherence

The meta-rules are themselves spec elements stored as datoms. The coherence engine can verify its own meta-rules for consistency. This completes the self-bootstrap loop: system specifies itself → stores spec as datoms → verifies spec using engine → engine is part of spec.

---

## 10. Architecture Decisions (From This Session)

| ADR | Decision | Rationale |
|-----|----------|-----------|
| ADR-VE-001 | Cleanroom engine over Scryer Prolog | Self-specifiable, no opaque deps, domain-specialized |
| ADR-VE-002 | Rules as datoms | Consistent with FD-012, enables temporal queries + CRDT merge |
| ADR-VE-003 | FFI integration via SQ-010 | Preserves Datalog simplicity, accretive |
| ADR-VE-004 | Fuel-bounded evaluation | Deterministic, reproducible, conservative |
| ADR-VE-005 | "Coherence engine" not "verification engine" | Names actual capability, not aspiration |
| ADR-VE-006 | Logical forms for ALL seven primitive types | Enables cross-element coherence checking |

---

## 11. Critical Reframes and Design Principles

### Settled Reframes
1. **"You don't need Prolog — you need a coherence engine."** Only unification, SLD resolution, backtracking, fuel, tabling, and meta-predicates required.
2. **"Don't call it verification — call it coherence/divergence detection."** Prolog search ≠ formal proof.
3. **"The coherence engine isn't a checker — it's the computational core of DDIS."** Every primitive interaction arrow becomes a computable predicate.
4. **The Go CLI's 0% bilateral adoption was because it was never built as first-class.** Hypothesis untested, not falsified.
5. **Willem's use case: divergence detection in evolving specs.** Not zero-defect via formal proof.

### Design Principles
- Every decision must pass: "Does this make the next session more effective?"
- Datalog detects (cheap, always-on). Prolog diagnoses (expensive, on-demand).
- Output optimized for agent consumption: actionable recommendations, not proof trees.
- Start simple. Let complexity earn its way in through demonstrated need.
- Every uncertainty resolution triggers full coherence re-check.
- `incompatible/2` and `entails/2` are the hardest design problems. Don't underestimate them.

---

## 12. Staging

**Stage 0**: Datom store + Datalog + coherence Datalog layer (MR 6,7,9,10) + harvest/seed
**Stage 1**: Coherence Prolog layer (MR 1-5,8) + full 5-tier contradiction detection + F(S)
**Stage 2+**: Agent-authored rules, distributed coherence, meta-circular verification

---

## 13. Open Questions

### Immediate
1. Can LLMs reliably extract logical forms from ALL SEVEN primitive types?
2. What is the Horn clause BNF?
3. How are `incompatible/2` and `entails/2` defined? (The property vocabulary problem)

### Design
4. Hard gate vs soft warning on transact?
5. How are goals formalized as logical constraints?
6. What is `system_constraint/1` and where do system constraints come from?

### Deferred
7. When do agent-authored rules become necessary?
8. How does distributed coherence checking work?

---

## 14. Recommended Next Actions

1. **Expanded feasibility experiment**: Test LLM translation on instances of ALL SEVEN primitive types + cross-element coherence
2. **Define core predicates**: `incompatible/2`, `entails/2`, `jointly_violates/3`, `holds/1`
3. **Draft spec/18-coherence.md**: Full coherence namespace in DDIS methodology
4. **Build Stage 0**: Store → Datalog → coherence Datalog layer → harvest/seed

---

## 15. Anti-Patterns

1. Don't let formalism outrun usability
2. Don't conflate Prolog search with formal proof
3. Don't build rule lifecycle before having rules
4. Don't optimize engine before validating translation layer
5. Don't pollute agent context with engine internals
6. Don't treat 0% bilateral adoption as validation failure
7. Don't treat ten meta-rules as final — grow from operational experience
8. Don't underestimate `incompatible/2` and `entails/2` — they're the hardest part

---

## 16. Key Terminology

| Term | Meaning |
|------|---------|
| **Coherence engine** | Logic engine checking spec consistency (detection + diagnosis, not proof) |
| **Logical form** | Machine-readable representation of a spec element's logical content |
| **Divergence** | Mismatch between any two layers (intent/spec/implementation/behavior) |
| **Type system for specifications** | Live coherence checking during authoring, in natural language |
| **Cascade** | Chain reaction through meta-rule dependency graph from single change |
| **False certainty** | Element appears settled but depends on uncertain foundations |
| **Pragmatic contradiction** | Two commitments compatible alone but jointly unsatisfiable |

---

## 17. Reference Documents

- **SEED.md**: `https://raw.githubusercontent.com/wvandaal/ddis/refs/heads/main/ddis-braid/SEED.md`
- **SPEC.md**: Modularized into `spec/` directory
- **IMPLEMENTATION_GUIDE.md**: Modularized into `docs/guide/` directory
- **"Datomic Implementation in Rust" transcript**: `/mnt/user-data/uploads/Datomic_implementation_in_Rust.md`
- **Seven DDIS Primitives document**: Provided inline during session

Key spec refs: SQ-010 (FFI), FD-003 (CALM), FD-012 (everything is transaction), SR-001/002 (indexes), SR-004 (HLC), SR-008/PO-012 (genesis), SQ-004/SQ-009 (query strata), CR-001 (conservative conflict detection)

---

*This seed captures the full state as of 2026-03-03. The coherence engine has been identified as the computational core of DDIS. A new session should read this document, then proceed to Action 1 (expanded feasibility experiment) unless directed otherwise.*

