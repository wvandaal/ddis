# Coherence Engine Architecture: Formal Analysis

> **Date**: 2026-03-03
> **Scope**: Complete formal methods assessment of the coherence engine proposal in
> `exploration/coherence-convergence/` against the broader Braid/DDIS specification
> **Method**: Cleanroom software engineering analysis grounded in formal methods,
> spec-driven design, and abstract algebra. First-principles reasoning throughout.
> No assumptions admitted without explicit justification.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Documents Analyzed](#2-documents-analyzed)
3. [Transcript Arc: The Complete Reasoning Chain](#3-transcript-arc)
4. [Algebraic Structure Analysis](#4-algebraic-structure-analysis)
5. [Logical Foundations Assessment](#5-logical-foundations-assessment)
6. [Termination, Soundness, and Completeness](#6-termination-soundness-and-completeness)
7. [CALM Theorem Application](#7-calm-theorem-application)
8. [Self-Referential Coherence and Godelian Limits](#8-self-referential-coherence)
9. [Cross-Reference: Alignment with Main SEED.md](#9-cross-reference-alignment)
10. [Cross-Reference: Alignment with Formal Specification (spec/)](#10-cross-reference-spec)
11. [Cross-Reference: Alignment with Implementation Guide (guide/)](#11-cross-reference-guide)
12. [ADR Alignment and Gaps](#12-adr-alignment)
13. [Failure Mode Coverage](#13-failure-mode-coverage)
14. [Design Decision Inventory](#14-design-decision-inventory)
15. [Formal Claims: Verdict Table](#15-formal-claims-verdict-table)
16. [Unresolved Tensions and Open Problems](#16-unresolved-tensions)
17. [Weakest Links in the Formal Chain](#17-weakest-links)
18. [Recommendations](#18-recommendations)

---

## 1. Executive Summary

The coherence engine proposal represents the most architecturally significant extension
to Braid since the original five axioms. Its central claim -- that the seven DDIS primitives
become a computable coherence machine rather than structured documentation -- is sound and
well-motivated. The two-layer architecture (Datalog for structural detection, Prolog for
logical diagnosis) is a principled decomposition that respects the CALM theorem boundary.

**What is formally sound:**
- The G-Set CRDT store algebra (L1-L5 properties, Shapiro et al. CvRDT)
- The Datalog/Prolog separation along the monotonicity boundary
- Fuel-bounded Prolog preserving soundness (timeout != pass)
- Rules-as-datoms as natural extension of C3/C7/FD-012
- The meta-rule family structure forming a well-founded DAG per cascade pass
- FFI integration via SQ-010 as the correct architectural seam

**What is overstated or requires qualification:**
- The cascade "fixed-point" is a fuel-bounded approximation, not a true least fixed point
- "False positives impossible by construction" holds for the logic engine but not end-to-end
  (LLM translation layer introduces probabilistic channel)
- MR-9 (Coverage Gap) and MR-10 (Formality Gap) are non-monotonic, not "pure Datalog"
- Horn clause representation of all seven primitives is partially faithful -- violation
  conditions, temporal safety, and entailment exceed the Horn clause fragment
- The staging does not align with the main SEED.md's five-stage roadmap

**What is the critical path:**
1. LLM translation fidelity (the oracle assumption underlying everything)
2. The property vocabulary for `incompatible/2` and `entails/2` (coNP-complete to undecidable)
3. FFI generalization from pure computation to search-with-fuel

**No hard contradictions** were found between the coherence engine proposal and the existing
Braid specification. All foundational commitments (C1-C7, A1-A5) are correctly represented.

---

## 2. Documents Analyzed

| Document | Location | Size | Role |
|---|---|---|---|
| Coherence Engine SEED | `exploration/coherence-convergence/SEED.md` | 462 lines, 17 sections | Primary proposal under analysis |
| Original Transcript | `exploration/coherence-convergence/original_transcript.md` | ~3,093 lines | Full reasoning chain behind the proposal |
| Conversation Summary | `exploration/coherence-convergence/CONVERSATION_SUMMARY.md` | 63 lines | Compaction summary |
| Main Braid SEED | `SEED.md` | ~850 lines | Foundational design document |
| Formal Specification | `spec/` (18 files) | ~7,000 lines | Modularized specification |
| Implementation Guide | `guide/` (13 files) | ~3,600 lines | Build plan |
| ADR Index | `ADRS.md` | ~500 lines | Settled design decisions |
| Failure Modes | `FAILURE_MODES.md` | ~300 lines | Known failure patterns |

---

## 3. Transcript Arc: The Complete Reasoning Chain

The transcript traverses five distinct phases, each shifting the architectural direction:

### Phase 1: Datalog vs. Prolog (Formal Separation)

The analysis establishes that Datalog's restrictions *purchase* the formal properties the five
axioms require. Specifically:

- A2 (Store = G-Set CRDT) requires monotonic or stratifiably non-monotonic evaluation
- A4 (monotonic queries uncoordinated) is CALM-specific to Datalog
- INV-CONFLICT-CONSERVATIVE-001 depends on monotonicity of causal-ancestor computation

**Conclusion reached**: "Replacing Datalog with Prolog for the datom query engine would
undermine the formal properties that make Braid correct." Prolog supplements; it does not
replace.

Two architectures compared:
- **Architecture A** (Stratified Separation): Two distinct engines sharing a datom store
- **Architecture B** (Unified Tabled Engine): One engine with tabled (Datalog-mode) and
  non-tabled (Prolog-mode) predicates

Architecture B initially preferred for its accretive property ("you verify the thing itself,
not a representation of it").

### Phase 2: Rules-as-Datoms (The Reflexive Turn)

The decision that "agents can write rules freely -- rules are datoms" is identified as
"the single most consequential architectural decision since the five axioms." This enables:

- Temporal auditability of the verification logic itself
- CRDT merge of rule bases across agents
- Meta-verification: rules verifying rules
- Self-referential engine consistency checking

Seven engine invariants formalized (INV-ENGINE-001 through INV-ENGINE-007). Semi-decidability
of meta-verification explicitly acknowledged: fuel bound becomes "load-bearing, not optional."

### Phase 3: Critical Intervention (The Honest Pushback)

Claude pushes back hard:

1. System over-specified top-down from theory; empirical evidence favors bottom-up from usage
2. Prolog verification engine solves a problem that doesn't exist yet
3. "Zero-defect via Prolog" is a category error (Prolog search != formal proof in the
   CompCert/seL4/Ironclad sense)
4. Formalism outrunning usability

Recommends radical Stage 0 strip-back: datom store + basic Datalog + harvest/seed.
~5,000-10,000 lines of Rust, no Prolog.

### Phase 4: The Reframe (Coherence, Not Verification)

Willem provides critical context correction:

- The Go CLI's 0% bilateral adoption was because it was never built as first-class, not
  because the concept is invalid. Hypothesis untested, not falsified.
- The actual use case: divergence detection and diagnosis in evolving specs, not zero-defect
  via formal proof.

**The breakthrough**: "A type system for specifications. Checked live, during authoring,
in natural language." The composition of LLM (natural-language-to-logic translation) +
Prolog (logical reasoning) produces something no existing tool provides.

Architecture B abandoned in favor of FFI integration via SQ-010 (Architecture A variant).
Coherence engine elevated from Stage 2+ to Stage 0.

### Phase 5: Full Primitive Coverage (The Deepening)

Analysis extends to all seven DDIS primitives. Each primitive type contributes distinct
logical content:

| Primitive | Extractable Content | Coherence Checks Enabled |
|---|---|---|
| Invariant | Violation condition | INV-INV contradiction, INV-ADR conflict |
| ADR | Commitment, assumption, exclusion | ADR-ADR conflict, assumption validity, exclusion violation |
| Negative Case | Prohibited state | Reachability analysis |
| Uncertainty | Provisional claim + impact chain | Uncertainty propagation, false certainty detection |
| Goal | Satisfaction condition | Goal entailment, coverage gap |

Ten meta-rules formalized in three families (Contradiction: MR-1..4, Drift: MR-5..7,
Coverage: MR-8..10). Cascade analysis demonstrates fixed-point behavior. Four-layer predicate
ontology specified. Self-referential coherence established.

---

## 4. Algebraic Structure Analysis

### 4.1 The Store as (P(D), union): G-Set CvRDT

**Claim**: The datom store forms a join-semilattice under set union and constitutes a
G-Set CvRDT in the Shapiro et al. sense.

**Verification**:

The five lattice laws hold by the ZFC axioms for set union over P(D):

```
L1 (Commutativity):   S1 union S2 = S2 union S1
L2 (Associativity):   (S1 union S2) union S3 = S1 union (S2 union S3)
L3 (Idempotency):     S union S = S
L4 (Monotonicity):    S subset_of (S union S')
L5 (Growth-only):     |S(t+1)| >= |S(t)|
```

The partial order is subset inclusion: `S1 <= S2 iff S1 subset_of S2`. The join is set
union. P(D) is in fact a *complete* lattice (every subset has a least upper bound).

**CvRDT requirements** (Shapiro et al. 2011):
1. Join-semilattice of states -- **satisfied**
2. Merge function = least upper bound -- **satisfied** (MERGE = union)
3. Monotonically increasing state updates -- **satisfied** (TRANSACT(S,T) = S union T.datoms)
4. Strong eventual consistency -- **satisfied** by L1-L3

**Verdict**: **Correct and well-founded.** Textbook G-Set CvRDT.

**Qualification**: The identity axiom uses `identity(d) = hash(d.e, d.a, d.v, d.tx, d.op)`.
The G-Set properties hold modulo hash collision (2^{-128} birthday probability for BLAKE3).
In a cleanroom formal methods context, this should be stated as an explicit assumption: the
store is a G-Set over hash-equivalence-classes of datoms.

**Missing property**: The spec does not state a least element (bottom). Genesis provides S_0
but is not the algebraic bottom (empty set is). The reachable state space is
`{S in P(D) | S_0 subset_of S}`, a sub-lattice with S_0 as bottom.

### 4.2 Cascade Computation as Fixed-Point System

**Claim**: "Meta-rules form a fixed-point system... same structure as Datalog's semi-naive
evaluation, but over coherence facts rather than datom facts."

**Analysis**: For a well-defined fixed-point system we need:

1. **A lattice**: The domain is P(CoherenceFacts) under set inclusion and union. This is
   a complete lattice. **Satisfied.**

2. **A monotone function**: The cascade operator `T_cascade` must be monotone. This is the
   critical failure point.

   The cascade operator is:
   ```
   T_cascade(Facts) = Facts
     union DatalogDerivations(Facts)
     union FuelBoundedPrologDerivations(Facts)
   ```

   `DatalogDerivations` is monotone by construction (standard Datalog property).

   `FuelBoundedPrologDerivations` is **not monotone** in general:
   - Adding facts can change Prolog search behavior under fuel bounding
   - A previously-completing search may timeout with a larger knowledge base (bigger
     search space)
   - A previously-timing-out search may complete with new facts that provide a shorter
     proof path

   If any Prolog predicate uses negation-as-failure (standard in Prolog), the operator
   is non-monotone even without fuel bounding.

3. **Termination**: The ascending chain must stabilize. Despite non-monotonicity, termination
   IS guaranteed because:
   - The fact set can only grow (derived facts are never removed)
   - The Herbrand base is finite (finitely many spec elements, finite predicate set)
   - Each step adds zero or more facts from a bounded domain
   - Zero additions = termination

**Verdict**: The claim is **overstated**. The cascade terminates but converges to a
**fuel-bounded approximation of a fixed point**, not the true least fixed point. The
correct characterization:

> The cascade computation iterates to a conservative under-approximation of the ideal
> fixed point. Every reported fact is real (soundness). Some derivable facts may be
> missed due to fuel exhaustion (incompleteness). The approximation becomes exact when
> all Prolog predicates complete within their fuel budgets and use no negation-as-failure.

The analogy to "Datalog semi-naive evaluation" is misleading because semi-naive converges
to the exact least fixed point. The spec should distinguish the ideal from the computed
approximation.

### 4.3 Well-Foundedness of Meta-Rule Families

**Claim**: The ten meta-rules in three families cannot form circular dependencies.

**Analysis of the dependency DAG within a single cascade pass**:

```
Layer 0: Spec element facts (input)
Layer 1: Logical forms (LLM-extracted, input)
Layer 2: Contradiction results (MR-1..4)
       -> Drift results (MR-5..7)
       -> Coverage results (MR-8..10)
Layer 3: Metrics and resolution (computed from Layer 2)
```

Each meta-rule produces facts at a strictly higher layer. Contradiction detection (Layer 2a)
feeds into drift detection (Layer 2b), which feeds into coverage detection (Layer 2c). No
backward dependencies exist within a single cascade pass.

**Can MR-9 (coverage gap) trigger MR-1 (contradiction)?** No. A coverage gap is a structural
observation ("goal G has no supporting invariants") that produces no new commitments or
logical forms. It cannot trigger contradiction checks.

**Can MR-7 (uncertainty propagation) trigger MR-1?** No. Uncertainty propagation changes
confidence scores, not logical content. MR-1 operates on logical incompatibility.

**Verdict**: **Correct.** Within a single cascade pass, the meta-rules form a well-founded
partial order with no circular dependencies. Cross-pass cascades (triggered by external
actions like uncertainty resolution) can re-enter the full cascade from Layer 2, but each
pass is independently well-founded.

---

## 5. Logical Foundations Assessment

### 5.1 Horn Clause Representation of the Seven Primitives

**Claim**: All seven primitive types can be faithfully represented as Horn clauses via
LLM "slot-filling" extraction.

**Analysis by primitive type**:

| Primitive | Extractable as Horn? | What Is Lost |
|---|---|---|
| INV (violation condition) | **Partially.** Existential over bad states can be Horn. Universal negation ("never X") requires negation, which is non-Horn. | Temporal operators (always, eventually), universal absence reasoning |
| ADR commitment | **Yes.** Ground facts: `commits_to(ADR, Property)`. | Semantic depth of what the property means |
| ADR assumption | **Yes.** `assumes(ADR, Predicate)`. | Whether the assumption "holds" may require non-Horn reasoning |
| ADR exclusion | **Yes.** `excludes(ADR, Pattern)`. | Checking exclusion violation requires `entails/2`, which is outside Horn |
| NEG (prohibited state) | **No.** Expressed in temporal logic as `[] not(bad_state)`. Temporal operators are not Horn-expressible. | Safety/liveness distinction, temporal modality |
| UNC (uncertainty) | **Yes** (as ground facts). `uncertain(E, Conf, Desc)`. | Aggregation (min-confidence over paths) is not Horn |
| Goal (satisfaction) | **Partially.** Simple goals as Horn. Joint entailment ("X and Y together satisfy G") exceeds Horn. | Compositional satisfaction reasoning |

**Key finding**: What the LLM can reliably extract is **structured metadata** (commitments,
assumptions, exclusions, prohibited states as ground facts or simple predicates). This is
genuinely useful for many coherence checks. But the claim that these are "Horn clauses"
supporting full logical reasoning is inaccurate -- the interesting reasoning (entailment,
incompatibility, temporal safety) happens *outside* the Horn clause fragment.

The spec implicitly acknowledges this by placing `incompatible/2` and `entails/2` in the
Prolog layer. But SLD resolution over definite Horn clauses still cannot handle negation.
Whether the Prolog layer uses negation-as-failure, constraint logic programming, or some
other extension is not specified. This is a significant under-specification.

**Verdict**: **Partially correct, partially misleading.** The extraction is better described
as "structured predicate extraction" than "Horn clause translation." The resulting
predicates are useful inputs to the coherence engine, but calling them "Horn clauses" creates
a false impression that standard resolution-based reasoning suffices.

### 5.2 LLM Translation Reliability

**Claim**: "Slot-filling is one of the most reliable LLM capabilities. Round-trip verification
catches unreliable translations."

**Formal guarantees**: None can be made in the formal sense. LLM outputs are stochastic.

**Round-trip verification analysis**: The check (prose -> clause -> prose -> compare) is
*necessary but not sufficient*. It verifies that the LLM can reconstruct the original meaning
from the logical form, but not that the logical form captures all semantically relevant
content.

**Counterexample**: "The store must not shrink" -> `violated_if(store_shrinks)` -> "The store
shrinks violates this invariant." Round-trip succeeds. But the logical form
`violated_if(store_shrinks)` is an opaque predicate with no internal structure. Two
invariants producing `store_shrinks` and `no_datom_deletion` are semantically equivalent
but structurally different -- the engine would not detect the equivalence.

**What CAN be formalized**: The system is a probabilistic checker: with probability >= p
(depending on LLM quality and round-trip threshold), the logical form is faithful. This makes
the coherence engine a *probabilistic* rather than *sound* system end-to-end.

**Verdict**: **Reasonable as engineering, not formalizable as a guarantee.** The spec should
characterize this as "high-confidence extraction" and acknowledge the probabilistic nature
explicitly, rather than implying formal reliability.

### 5.3 Complexity of Core Predicates

**Claim**: "`incompatible/2` and `entails/2` are the hardest design problems."

**This is not just hard -- it is provably intractable in the general case:**

| Predicate | Over propositional logic | Over first-order logic | Over Horn clauses |
|---|---|---|---|
| `entails(phi, psi)` | coNP-complete | Undecidable (Church) | P (single Horn psi), coNP-complete (disjunctive/negated psi) |
| `incompatible(phi, psi)` | NP-complete (SAT of conjunction) | Undecidable | Depends on clause structure |
| `jointly_violates(phi, psi, C)` | NP-complete (3-SAT) | Undecidable | NP-complete even for propositional |

**Practical escape routes**:
1. **Closed property ontology**: Finite vocabulary with explicit incompatibility/entailment
   declarations. Makes lookups O(1) but requires manual engineering of all relationships.
2. **LLM-mediated semantic matching**: Scalable but inherits LLM limitations.
3. **Hybrid**: Property ontology for known relationships, LLM for novel ones.

**Verdict**: The spec is **correct** that these are the hardest problems. No tractable
general solution exists without constraining the property vocabulary. The spec should
explicitly classify the decidability of each predicate and state which approach is taken.

---

## 6. Termination, Soundness, and Completeness

### 6.1 Fuel-Bounded Prolog and Soundness

**Claim**: Fuel bounding preserves soundness. `timeout -> :timeout, not :passed`.

**Analysis**:
- `:passed` (proof found): Sound -- a proof is a proof regardless of fuel.
- `:timeout` (fuel exhausted): Conservative -- "I don't know." No false positive reported.
- `:failed` (search space exhausted within fuel): Sound only if fuel was sufficient for
  complete exploration. The spec states `:timeout` is used, not `:failed`, avoiding this
  ambiguity.

**Verdict**: **Correct.** Fuel bounding preserves soundness under the stated semantics.
The system is sound but incomplete (standard bounded model checking tradeoff).

### 6.2 End-to-End Soundness

**Claim**: "False negatives safe, false positives impossible by construction."

**Decomposition**: "False positives impossible by construction" means the engine never
reports a contradiction that doesn't exist. This holds for the *logic engine alone* but
fails end-to-end:

A false positive arises if:
1. The `incompatible/2` predicate is incorrectly defined (specification bug)
2. The LLM translation produces incorrect logical forms (probabilistic failure)
3. The Prolog evaluator is unsound (implementation bug)

Case 2 is the critical gap. If the LLM mistranslates a spec element's logical form, the
engine reasons over incorrect data. Example: ADR-A says "prefer simplicity" and ADR-B says
"support advanced analytics." The LLM might translate ADR-A as `excludes(complex_computation)`
and ADR-B as `commits_to(complex_computation)`, producing a false contradiction.

**Corrected statement**:

> False positives in the **logical engine** are impossible by construction. The system's
> end-to-end false positive rate depends on the fidelity of the LLM translation layer,
> which is probabilistic, not formally guaranteed.

### 6.3 Cascade Convergence Under Fuel Bounding

The cascade terminates (finite Herbrand base, monotonically growing fact set), but the
result is a conservative under-approximation of the true fixed point. The analogy to
Datalog semi-naive evaluation should be qualified: semi-naive converges to the exact least
fixed point; the fuel-bounded cascade converges to an approximation that is exact only when
all Prolog evaluations complete within their fuel budgets.

---

## 7. CALM Theorem Application

**Claim**: "Monotonic queries run without coordination (CALM theorem)." Meta-rules MR-6,
MR-7, MR-9, MR-10 are "pure Datalog" (implying monotonic).

**Classification of meta-rules by monotonicity**:

| Rule | Claimed | Actual | Reasoning |
|---|---|---|---|
| MR-1 (Commitment Contradiction) | Prolog | Non-monotonic | Requires `incompatible/2` |
| MR-2 (Exclusion Violation) | Prolog | Non-monotonic | Requires `entails/2` |
| MR-3 (Negative Case Reachability) | Prolog | Non-monotonic | Requires `entails/2` |
| MR-4 (Pragmatic Contradiction) | Prolog | Non-monotonic | Requires `jointly_violates/3` |
| MR-5 (Assumption Invalidation) | Mostly Datalog | Partially monotonic | `holds/1` may use negation |
| MR-6 (Dependency Orphaning) | Pure Datalog | **Monotonic** | `depends_on(A,B), superseded(B)` is additive |
| MR-7 (Uncertainty Propagation) | Pure Datalog | **Monotonic** (under correct lattice) | Min-confidence over paths; adding paths can only maintain or lower confidence. Monotone in the meet-semilattice of confidence values (lower = more information) |
| MR-9 (Coverage Gap) | Pure Datalog | **Non-monotonic** | Detects *absence*: "goal has NO supporting invariants" requires stratified negation |
| MR-10 (Formality Gap) | Pure Datalog | **Non-monotonic** | Detects *absence*: "invariant has NO logical form" requires stratified negation |
| MR-8 (Goal Entailment Failure) | Prolog | Non-monotonic | Requires entailment check |

**Critical finding**: MR-9 and MR-10 are **misclassified**. They are gap detectors that fire
when something is absent. Adding the missing element removes the gap. In Datalog terms:

```
coverage_gap(G) :- goal(G), NOT exists(I, supports(I, G), active(I))
```

The `NOT exists` makes this a stratified negation query (non-monotonic). Per CALM, these
**cannot run without coordination**. They should be classified as frontier-relative queries,
not coordination-free monotonic queries.

**MR-7 subtlety**: Min-confidence propagation is monotone only under a reversed lattice
where lower confidence = greater information. This lattice must be explicitly defined in the
spec. Without it, the monotonicity claim for MR-7 is informal.

**Verdict**: CALM application is **partially correct with two misclassifications** (MR-9,
MR-10) and one informally justified claim (MR-7 lattice).

---

## 8. Self-Referential Coherence and Godelian Limits

**Claim**: "The meta-rules are themselves spec elements stored as datoms. The coherence
engine can verify its own meta-rules for consistency."

**Analysis through Godel's incompleteness theorems**:

The Prolog layer with lists and successor arithmetic is Turing-complete. Godel's theorems
apply.

**What the engine CAN do**:
- Detect contradictions among its own meta-rules (e.g., MR-1 and MR-4 producing
  incompatible conclusions for the same input). This is a finite check over a finite domain.
- Verify structural properties of the meta-rule set (well-foundedness, stratification).

**What the engine CANNOT do**:
- Prove its own soundness ("if the engine says no contradiction exists, then no contradiction
  exists"). By Godel's second incompleteness theorem, this is impossible within the system.
- Certify its own completeness ("the engine detects all contradictions that exist").

**Practical impact**: The Godelian ceiling is real but its practical impact is minimal. The
useful capability -- detecting contradictions within the meta-rules -- is fully achievable.
The engine cannot certify its own correctness, but no system can. The self-bootstrap claim
is correctly scoped for detection, but the phrase "completes the self-bootstrap loop" is
suggestive of a stronger property than is achievable.

**Verdict**: **Correctly scoped for contradiction detection.** The spec should explicitly
state the ceiling: the engine can detect inconsistencies in its own meta-rules but cannot
prove its own soundness or completeness. There is no free lunch in self-reference.

---

## 9. Cross-Reference: Alignment with Main SEED.md

### 9.1 Five Axioms (A1-A5)

| Axiom | Coherence SEED | Main SEED | Verdict |
|---|---|---|---|
| A1 (Identity) | `[e, a, v, tx, op]`, tx carries provenance | Identical statement | **Aligned** |
| A2 (Store) | Append-only G-Set, query-layer resolution | Identical statement | **Aligned** |
| A3 (Snapshots) | Local frontier default, optional sync barriers | Identical statement | **Aligned** |
| A4 (Queries) | CALM: monotonic = coordination-free | Identical statement | **Aligned** |
| A5 (Resolution) | Per-attribute: lattice, LWW, multi-value | Identical. Minor omission: lattice defs are store facts | **Aligned** (minor gap) |

### 9.2 Seven DDIS Primitives

All seven correctly described. The coherence SEED omits the main SEED's structural distinction
between element types (INV, ADR, NEG, UNC -- artifacts with IDs) and verification mechanisms
(Contradiction Detection, F(S), Bilateral Loop -- processes over elements). The primitive
interaction web is a faithful distillation.

### 9.3 Hard Constraints (C1-C7)

All seven correctly represented. C7 (Self-bootstrap) extended to meta-rules with an
unacknowledged complexity for procedural rules (see Section 8).

### 9.4 Staging -- MISALIGNED

The coherence SEED proposes three stages:
- Stage 0: Datom store + Datalog + coherence Datalog layer (MR 6,7,9,10) + harvest/seed
- Stage 1: Prolog layer (MR 1-5,8) + 5-tier contradiction + F(S)
- Stage 2+: Agent-authored rules, distributed coherence, meta-circular verification

The main SEED has five stages:
- Stage 0: Harvest/Seed Cycle (transact, query, status, harvest, seed, guidance, CLAUDE.md)
- Stage 1: Budget-Aware Output + Guidance Injection
- Stage 2: Branching + Deliberation
- Stage 3: Multi-Agent Coordination
- Stage 4: Advanced Intelligence

**These do not align.** The coherence engine's stages need mapping onto the main roadmap:
- Coherence Datalog layer -> Main Stage 0 or 1
- Coherence Prolog layer -> Main Stage 2 (alongside deliberation)
- Agent-authored rules -> Main Stage 4

### 9.5 Reconciliation Taxonomy

The coherence SEED lists four divergence types (Axiological, Logical, Structural, Behavioral).
The main SEED has eight (Epistemic, Structural, Consequential, Aleatory, Logical, Axiological,
Temporal, Procedural). The coherence SEED's taxonomy is incomplete but not contradictory --
it covers the four types most relevant to spec-level checking.

### 9.6 Novel Additions (Not in Main SEED)

1. **LLM-mediated translation to logical forms** -- Entirely new. Not contradicted but
   introduces a non-deterministic component.
2. **Ten meta-rules in three families** -- Useful concretization of implicit checks.
3. **Two-layer Datalog+Prolog architecture** -- Consistent extension. Does not contradict
   ADR-QUERY-002 (which rejected top-down for the *primary* query engine, not auxiliary
   reasoning).
4. **Cascade analysis** -- New fixed-point computation over coherence facts.
5. **F(S) as live computed metric** -- Extends the existing static measurement model.

**Verdict**: No hard contradictions. The coherence engine is a consistent but substantial
extension of the main SEED, adding significant new architectural proposals that need formal
specification.

---

## 10. Cross-Reference: Alignment with Formal Specification (spec/)

### 10.1 Already Covered

| Spec Area | Coverage | Key References |
|---|---|---|
| Datom store | **Full** | spec/01-store.md (INV-STORE-001..014) |
| Query engine / Datalog | **Partial** | spec/03-query.md (INV-QUERY-001..021) |
| FFI boundary | **Partial** | ADR-QUERY-004, INV-QUERY-008. Needs extension for inference functions |
| Schema-as-data | **Full** | spec/02-schema.md (INV-SCHEMA-001..010) |
| Bilateral loop | **Partial** | spec/10-bilateral.md (INV-BILATERAL-001..002, five-point coherence) |
| Deliberation | **Partial** | spec/11-deliberation.md. Interface to coherence diagnosis not specified |
| Uncertainty | **Partial** | spec/15-uncertainty.md. MR-7 computes what this section tracks manually |

### 10.2 Entirely New (Requires spec/18-coherence.md)

1. **Logical forms** -- Schema, extraction protocol, round-trip verification
2. **Prolog layer** -- Unification, SLD resolution, fuel monad, tabling, meta-predicates
3. **Ten meta-rules** -- Formal definitions, family structure, cascade semantics
4. **Predicate ontology** -- Four-layer system (spec facts -> logical forms -> meta-rules -> metrics)
5. **Resolution generation** -- Templates, cost estimation, integration with deliberation
6. **F(S) live computation** -- Partially automated fitness function
7. **Horn clause BNF** -- Still an open question

### 10.3 Tensions with Existing Spec

**Tension 1: FFI Scope**. Existing ADR-QUERY-004 scopes FFI to three mathematical computations
(entropy, DAG traversal, SVD). The coherence engine adds four inference functions. The existing
`DerivedFunction` trait (`fn evaluate(&self, inputs: &[Value]) -> Result<Value, FfiError>`)
is inadequate for Prolog-style search with fuel bounds, knowledge base access, and
multi-valued results. Needs generalization to an `InferenceFunction` trait.

**Tension 2: Query Determinism**. INV-QUERY-002 ("identical expressions at identical
frontiers return identical results") must be reconciled with fuel-bounded evaluation. If fuel
is treated as a query parameter (like frontier), determinism holds. But this is not currently
specified.

**Tension 3: Transaction Pipeline Purity**. The coherence engine requires LLM calls during
`braid transact`. INV-STORE-014 and the typestate transaction lifecycle assume deterministic
pipeline stages. LLM translation is non-deterministic and externally dependent. Resolution:
treat LLM translation as a pre-transaction enrichment step, not part of the core transaction
pipeline.

### 10.4 Required Spec Modifications

If the coherence engine is adopted:

1. **spec/03-query.md** -- Extend FFI for inference functions. Add fuel as query parameter.
   New stratum for coherence queries. Reconcile determinism.
2. **spec/10-bilateral.md** -- Link F(S) to coherence engine live computation.
3. **spec/11-deliberation.md** -- Interface from coherence diagnosis to deliberation entry.
4. **spec/15-uncertainty.md** -- Add UNC entries for LLM translation fidelity, fuel adequacy,
   cascade amplification.
5. **spec/16-verification.md** -- Add verification matrix for coherence invariants.
6. **spec/17-crossref.md** -- Add COHERENCE namespace to indexes and counts.
7. **spec/02-schema.md** -- Document which schema layer hosts coherence attributes (Layer 2).

---

## 11. Cross-Reference: Alignment with Implementation Guide (guide/)

The coherence engine does not change Stage 0 deliverables (transact, query, harvest, seed,
guidance, CLAUDE.md) but adds:

- New schema attributes for coherence facts (`:coherence/logical-form`,
  `:coherence/meta-rule-result`, etc.)
- New Datalog rules for MR-6, MR-7, MR-9, MR-10
- New CLI commands (`braid diagnose`, `braid cascade`)
- LLM translation pipeline with round-trip verification

A new guide file (`guide/XX-coherence.md`) is needed alongside `spec/18-coherence.md`.
The cleanroom Prolog crate (~2,000-3,000 lines Rust) is a substantial new artifact not
in the current build plan.

---

## 12. ADR Alignment and Gaps

### 12.1 Existing ADRs Supporting the Coherence Engine

| ADR | Relationship |
|---|---|
| FD-003 (Datalog for Queries) | Coherence Datalog layer IS the query engine |
| FD-012 (Every Command = Transaction) | Coherence checks produce transactions with provenance |
| SQ-010 (Datalog/Imperative FFI) | Integration point for Prolog layer |
| FD-002 (EAV over Relational) | Logical forms and meta-rule results are EAV datoms |
| FD-008 (Schema-as-Data) | Meta-rules are datoms -- pure schema-as-data |
| CO-004 (Bilateral Convergence) | Coherence engine IS the computation behind bilateral convergence |
| CO-008 (Five-Point Coherence) | C1 (spec self-consistency) = core coherence engine function |
| CO-009 (Fitness Function) | 5 of 7 F(S) dimensions automated by engine |
| CR-001 (Conservative Conflict Detection) | Engine's conservative approach (no false positives) aligns |
| AS-001 (G-Set CRDT) | Coherence facts merge by set union -- correct by construction |

### 12.2 Existing ADRs with Potential Conflicts

| ADR | Issue |
|---|---|
| ADR-QUERY-004 (FFI for Derived Functions) | Scope limited to mathematical computations. Prolog inference is categorically different. |
| SQ-009 (Stratum Safety) | Six strata don't include coherence. New stratum may be needed. |
| PO-013 (Query Determinism) | Fuel-bounded evaluation may violate without explicit fuel parameter. |

### 12.3 New ADRs Proposed

Six explicit (ADR-VE-001 through ADR-VE-006):

| ID | Decision | Status |
|---|---|---|
| ADR-VE-001 | Cleanroom engine over Scryer Prolog | Settled |
| ADR-VE-002 | Rules as datoms | Settled |
| ADR-VE-003 | FFI integration via SQ-010 | Settled |
| ADR-VE-004 | Fuel-bounded evaluation | Settled |
| ADR-VE-005 | "Coherence engine" not "verification engine" | Settled |
| ADR-VE-006 | Logical forms for ALL seven primitive types | Settled |

Three implicit (need formalization):

| ID | Decision | Notes |
|---|---|---|
| ADR-VE-007 | LLM translation as pre-transaction enrichment | Separates non-deterministic LLM from deterministic store |
| ADR-VE-008 | Cascade as fixed-point over meta-rules | Analogous to semi-naive, with fuel-bounded approximation |
| ADR-VE-009 | Resolution generation feeds deliberation | Engine diagnoses, deliberation resolves |

---

## 13. Failure Mode Coverage

### 13.1 Existing Failure Modes Addressed

| FM | Failure Mode | How Engine Addresses It |
|---|---|---|
| FM-004 | Cascading Incompleteness | MR-9 (coverage gap) + MR-6 (dependency orphaning) catch missing links |
| FM-005 | Semantic ID Collision | Logical forms provide semantic comparison beyond structural IDs |
| FM-007 | Decision-Layer Propagation Failure | MR-1 (contradiction) + MR-2 (exclusion violation) catch ADR conflicts |
| FM-008 | Derived Quantity Staleness | Live F(S) computation replaces hardcoded counts |
| FM-009 | Silent ADR Contradiction | Primary target of MR-1 and MR-2 |

FM-001 (Knowledge Loss) and FM-002 (Provenance Fabrication) are addressed by harvest/seed,
not the coherence engine. FM-003 (Anchoring Bias) is partially addressed: engine operates
over full store, eliminating single-document anchoring.

### 13.2 New Failure Modes Introduced

| FM | Name | Description | Mitigation |
|---|---|---|---|
| FM-010 | LLM Translation Infidelity | Incorrect logical form extraction causes engine to reason over wrong data | Round-trip verification, confidence scoring |
| FM-011 | Fuel Exhaustion False Negatives | Low fuel limit causes real contradictions to be reported as `:timeout` | Configurable fuel, distinguish "confirmed safe" from "search incomplete" |
| FM-012 | Cascade Amplification | One incorrect meta-rule result cascades through fixed-point, producing chain of false findings | Conservative approach + LLM fidelity are the intended mitigations; single false input can still amplify |

---

## 14. Design Decision Inventory

### Firmly Settled (accepted into the design)

| # | Decision | Justification |
|---|---|---|
| 1 | Datalog for queries, not Prolog | CALM, termination, CRDT convergence depend on Datalog's restrictions |
| 2 | Two-layer architecture with FFI boundary | Datalog handles operational queries; Prolog handles logical reasoning |
| 3 | Cleanroom minimal engine (not Scryer/XSB) | Self-specifiable, no opaque deps, domain-specialized |
| 4 | Rules as datoms | Consistent with C3/C7/FD-012; enables temporal queries, CRDT merge |
| 5 | Fuel-bounded evaluation | Deterministic, reproducible, conservative (timeout != pass) |
| 6 | "Coherence engine" not "verification engine" | Detection + diagnosis, not proof |
| 7 | Logical forms for all seven primitives | Uniform extraction enables cross-element checking |
| 8 | "Datalog detects, Prolog diagnoses" | Most meta-rules are Datalog; only four predicates need Prolog |
| 9 | Coherence Datalog layer in Stage 0 | Self-bootstrap: spec is first data, must be coherent |

### Proposed but Pending Validation

| # | Decision | Blocking Risk |
|---|---|---|
| 1 | LLM-mediated translation as bridge | LLM translation fidelity (Open Question 1) |
| 2 | Round-trip verification as quality gate | Necessary but not sufficient |
| 3 | Ten specific meta-rules | Pending implementation experience |
| 4 | Four-layer predicate ontology | Pending schema design |
| 5 | Resolution generation with cost ranking | Heuristic without formal justification |
| 6 | F(S) formula and weights | Provisional (UNC-BILATERAL-001, confidence 0.6) |
| 7 | Cascade fixed-point behavior | Described but not tested |

### Explicitly Walked Back During Transcript

| Original | Revised To | Reason |
|---|---|---|
| Architecture B (unified tabled engine) | FFI integration via SQ-010 | SEED.md already has FFI; preserves Datalog simplicity |
| Full rule lifecycle for agent-written rules | Mechanical derivation from invariants | Overengineered for initial scope |
| "Verification engine" | "Coherence engine" | Names actual capability, not aspiration |
| "Prolog for formal verification" | "Prolog for divergence detection/diagnosis" | Category error corrected |
| Coherence engine at Stage 2+ | Coherence Datalog layer at Stage 0 | Self-bootstrap demands it |

---

## 15. Formal Claims: Verdict Table

| # | Claim | Source | Verdict | Notes |
|---|---|---|---|---|
| 1 | Store is (P(D), union) G-Set CvRDT | SEED S2 | **Sound** | Textbook result. Hash collision assumption should be stated. |
| 2 | CALM: monotonic queries coordination-free | SEED S2 | **Sound** | Standard result (Hellerstein 2010). |
| 3 | Semi-naive reaches fixed point | SEED S6 | **Sound** | Standard result for safe Datalog. |
| 4 | Cascade forms fixed-point system | SEED S6 | **Overstated** | Fuel-bounded approximation, not true least fixed point. |
| 5 | MR-6,7,9,10 are "pure Datalog" | SEED S5 | **Partially wrong** | MR-9 and MR-10 require stratified negation (non-monotonic). |
| 6 | Horn clauses for all seven primitives | SEED S4.3 | **Partially correct** | Structured metadata extraction yes; full logical reasoning no. |
| 7 | Slot-filling is reliable LLM capability | SEED S4.3 | **Pragmatically reasonable** | Not formalizable. Round-trip is necessary but not sufficient. |
| 8 | False positives impossible by construction | SEED S7 | **Partially correct** | True for logic engine. False end-to-end due to LLM layer. |
| 9 | Fuel bounding preserves soundness | SEED S7 | **Sound** | Timeout != passed. Standard bounded model checking. |
| 10 | Meta-rules well-founded (no cycles) | SEED S5 | **Correct per cascade pass** | Layer structure prevents intra-pass cycles. |
| 11 | Self-referential coherence | SEED S9 | **Correctly scoped** | Detects inconsistencies; cannot prove own soundness (Godel). |
| 12 | MR-1 and MR-4 mutually exclusive | SEED S5 | **Sound** | By construction of their preconditions. |
| 13 | Effective confidence = min over deps | SEED S5 | **Sound as design choice** | Conservative. Product or weighted average are alternatives. |
| 14 | F(S) = 0.20V + 0.25C + 0.20H + 0.15I + 0.20U | SEED S4.5 | **Not justified** | Weights provisional (confidence 0.6). Linear combination ad hoc. |
| 15 | LLM round-trip as quality gate | SEED S4.3 | **Necessary but insufficient** | Two different forms can round-trip to same prose. |
| 16 | Five axioms correctly stated | SEED S2 | **Correct** | Verified against main SEED and formal spec. |
| 17 | Seven primitives correctly described | SEED S3 | **Correct** | Minor: omits element-type vs. mechanism distinction. |
| 18 | Meta-verification semi-decidable | Transcript | **Correct** | Standard result for Horn clause entailment with function symbols. |

---

## 16. Unresolved Tensions and Open Problems

### 16.1 Critical (blocks implementation)

1. **LLM Translation Fidelity (Open Question 1)**: The entire architecture depends on LLMs
   reliably translating natural-language spec elements to structured logical forms. This is
   the central feasibility risk. The recommended feasibility experiment (Action 1 in the SEED)
   is the right next step.

2. **Property Vocabulary for `incompatible/2` and `entails/2` (Open Question 3)**: These
   predicates require a shared semantic model of property relationships. Without it, they are
   either undefined (rendering MR-1..4 inoperative) or LLM-mediated (inheriting LLM
   limitations). Decidability ranges from P (closed ontology) to undecidable (open first-order).

3. **Horn Clause BNF (Open Question 2)**: The syntactic definition of the logical form
   language is not specified. This is prerequisite to everything.

### 16.2 Significant (affects correctness guarantees)

4. **Staging Misalignment**: The coherence engine's three-stage plan does not map to the main
   SEED's five-stage roadmap. Needs reconciliation.

5. **FFI Scope Extension**: SQ-010 was designed for pure mathematical computations, not
   inference engines. The `DerivedFunction` trait needs generalization. The interaction between
   fuel-bounded Prolog results and Datalog fixpoint computation needs specification.

6. **MR-9/MR-10 Misclassification**: These are non-monotonic (gap detection requires
   negation-as-failure) but classified as "pure Datalog." Should be reclassified as
   frontier-relative queries.

7. **System Constraints (`system_constraint/1`)**: MR-4 (Pragmatic Contradiction) requires
   a predicate representing execution environment facts. Where do these come from? How are
   they formalized? Neither document addresses this.

### 16.3 Moderate (affects design quality)

8. **Confluence**: If two agents independently run the cascade on the same facts, do they
   get the same result? Requires determinism of Prolog and cascade iteration order.

9. **Bounded Latency**: Claimed "subseconds for hundreds of elements" but no complexity
   analysis provided. Worst case for pairwise MR-1 checks: O(N^2 * F) where N = elements,
   F = fuel. For N=1000, F=10000, this is 10^10 operations -- not subsecond.

10. **Cascade Termination with Negation**: Some meta-rules may use negation-as-failure.
    Stratified negation guarantees termination only if stratification is acyclic. The ten
    meta-rules' stratification is asserted but not formally verified.

### 16.4 Deferred (explicitly acknowledged)

11. When do agent-authored rules become necessary?
12. How does distributed coherence checking work?
13. Hard gate vs. soft warning on transact?
14. How are goals formalized as logical constraints?

---

## 17. Weakest Links in the Formal Chain

**Ranked by severity:**

1. **LLM Translation Fidelity** (Critical). The entire coherence engine sits atop
   LLM-extracted logical forms. This is an *oracle assumption* embedded in a system that
   claims formal properties. If the oracle is wrong, the system reasons over garbage.
   Round-trip verification is a heuristic, not a proof.

2. **Property Vocabulary / Ontology** (Critical). `incompatible/2` and `entails/2` require
   a semantic model that does not exist. Without it, the four Prolog-layer meta-rules
   (MR-1..4) -- which are the most valuable checks -- are inoperative.

3. **MR-9/MR-10 Monotonicity Misclassification** (Moderate). Practical impact: incorrect
   results when run without coordination per CALM. Fix: reclassify as stratified. Straightforward.

4. **Cascade Approximation vs. True Fixed Point** (Moderate). Affects interpretation of
   "no contradictions found." Should be "no contradictions found within fuel budget."

5. **Hash Collision Assumption** (Low). BLAKE3 makes this negligible (2^{-128}). Standard
   engineering approximation but should be stated in a cleanroom context.

6. **Self-Verification Ceiling** (Low). Godelian limit on self-verification is inherent.
   Practical impact minimal -- engine can still detect contradictions in its own rules.

---

## 18. Recommendations

### 18.1 Immediate Actions

1. **Execute the feasibility experiment** (SEED Action 1). Test LLM translation on instances
   of ALL SEVEN primitive types. This is the highest-risk, lowest-cost validation. Until this
   is done, the entire architecture is hypothetical.

2. **Define the Horn clause BNF** (SEED Open Question 2). The syntactic form of logical
   forms must be specified before anything else can be built. Consider whether pure Horn
   clauses are adequate or whether the language needs extensions (negation-as-failure, CLP,
   or constraint predicates).

3. **Reclassify MR-9 and MR-10** as stratified/frontier-relative queries, not "pure Datalog."
   This is a documentation fix with no implementation impact but corrects a formal error.

### 18.2 Specification Work

4. **Draft spec/18-coherence.md**. This should cover: logical form schema, predicate ontology,
   ten meta-rules with formal definitions, cascade semantics, Prolog layer contract, LLM
   translation protocol, integration points with BILATERAL and DELIBERATION.

5. **Reconcile staging**. Map the coherence engine's stages onto the main SEED's five-stage
   roadmap. Propose: Datalog layer -> Stage 0, Prolog layer -> Stage 1-2, agent-authored
   rules -> Stage 4.

6. **Extend SQ-010** for inference functions. Define an `InferenceFunction` trait alongside
   the existing `DerivedFunction` trait. Specify fuel as a query parameter. Reconcile
   INV-QUERY-002 (determinism).

7. **Add uncertainty markers** for: LLM translation fidelity (UNC-COHERENCE-001), fuel
   limit adequacy (UNC-COHERENCE-002), cascade amplification risk (UNC-COHERENCE-003).

### 18.3 Design Decisions Needed

8. **Property vocabulary strategy**: Closed ontology, LLM-mediated matching, or hybrid?
   This determines the decidability class of `incompatible/2` and `entails/2` and is the
   single most consequential design decision remaining.

9. **Cascade semantics under timeout**: When a Prolog predicate times out during cascade
   iteration, what happens? Options: (a) treat as "no finding" and continue, (b) mark
   the cascade result as approximate, (c) retry with higher fuel.

10. **Qualify end-to-end soundness claims**. Replace "false positives impossible by
    construction" with a precise characterization: "false positives in the logic engine
    are impossible by construction; end-to-end false positive rate depends on LLM
    translation fidelity, which is probabilistic."

### 18.4 Anti-Pattern Vigilance

The transcript's own anti-patterns (Section 15) remain relevant:

1. Don't let formalism outrun usability
2. Don't conflate Prolog search with formal proof
3. Don't build rule lifecycle before having rules
4. Don't optimize engine before validating translation layer
5. Don't pollute agent context with engine internals
6. Don't treat ten meta-rules as final
7. Don't underestimate `incompatible/2` and `entails/2`

To which this analysis adds:

8. Don't present fuel-bounded approximations as exact fixed points
9. Don't classify gap-detection (absence reasoning) as monotonic
10. Don't claim end-to-end soundness when the input pipeline is probabilistic

---

*This analysis was produced by a team of four Opus 4.6 subagents performing parallel
independent analysis, synthesized into a unified document. Each section traces to specific
lines in the source transcript and cross-references against the formal specification.*
