# Query Engine Enhancements from LP, ATP, and Computational Linguistics

> **Date**: 2026-03-03
> **Status**: Design analysis (pre-implementation)
> **Scope**: Ten specific techniques from three formal traditions, mapped into
> the Braid six-stratum Datomic-style query engine for spec coherence checking
> **Dependency**: `spec/03-query.md` (query engine specification),
> `PROPERTY_VOCABULARY.md` (109-property closed ontology),
> `FULL_EXTRACTION_RESULTS.md` (248-element extraction, 25 tensions)
> **Consumed by**: `HARVEST_SEED.md` §9–10 (summary), future Stage 0 implementation

---

## Table of Contents

1. [Framing: Why Borrow from Other Traditions?](#1-framing)
2. [Terminological Ground Truth](#2-terminology)
3. [From Logic Programming](#3-logic-programming)
   - 3.1 [Unification for EDN Value Destructuring](#31-unification)
   - 3.2 [Subsumption-Based Tabling (XSB Prolog)](#32-tabling)
   - 3.3 [CLP Framing for Property Constraints](#33-clp)
   - 3.4 [DCG-Style Parsing for Structured Fragments](#34-dcg)
4. [From Automated Theorem Proving](#4-atp)
   - 4.1 [SMT Solving (Z3) as Database Function](#41-smt)
   - 4.2 [Bounded Model Checking at Crystallization Points](#42-bmc)
   - 4.3 [Craig Interpolation for Minimal Contradiction Explanations](#43-interpolation)
5. [From Computational Linguistics](#5-cl)
   - 5.1 [Discourse Representation Theory (DRT)](#51-drt)
   - 5.2 [Semantic Role Labeling (SRL)](#52-srl)
   - 5.3 [Natural Language Inference (NLI) as Database Function](#53-nli)
6. [Unified Architecture: How Everything Fits Together](#6-unified)
7. [What We Don't Need (and Why)](#7-exclusions)
8. [Implementation Staging and Dependencies](#8-staging)
9. [Formal Justification: Why These Techniques Are Sound](#9-formal)
10. [Empirical Grounding: What Our Extraction Found](#10-empirical)

---

## 1. Framing: Why Borrow from Other Traditions?

<framing>

The Braid specification defines a six-stratum query engine (`spec/03-query.md` §3.2) that
classifies queries by power and cost. The engine operates over `[entity, attribute, value,
transaction, operation]` datoms in a grow-only set-union CRDT store `(P(D), ∪)`.

The core engine — Datomic-style Datalog with semi-naive evaluation, stratified negation,
and FFI for derived functions — already covers the vast majority of what coherence checking
requires. This document does NOT argue for replacing that engine. It identifies ten specific
techniques from three traditions (logic programming, automated theorem proving, and
computational linguistics) that enhance the engine at precise points where vanilla Datomic
semantics fall short.

### The Decision Criterion

A technique is worth borrowing if and only if:
1. It solves a problem the current engine specification cannot express
2. The problem is encountered in the coherence checking use case (empirically validated)
3. The technique maps cleanly into the existing stratum classification
4. The implementation cost is proportional to the checking value gained

We validated criterion #2 empirically. The full extraction of 248 spec elements
(`FULL_EXTRACTION_RESULTS.md`) found 25 tensions. Analyzing which tensions each
technique would catch gives us a concrete value estimate:

| Detection Method | Tensions Caught | Percentage |
|-----------------|----------------|------------|
| Property incompatibility lookup (vanilla Datomic) | 0 | 0% |
| Entailment chain traversal (vanilla Datomic) | 8 gaps | — |
| Cross-element property conflict (vanilla Datomic) | 3 | 12% |
| Stage ordering check (vanilla Datomic) | 1 | 4% |
| Threshold consistency (vanilla Datomic) | 1 | 4% |
| **Subtotal: vanilla Datomic queries** | **13** | **52%** |
| Stratified negation (negation-as-failure + reachability) | ~8 | ~32% |
| LLM semantic analysis (NLI, domain reasoning) | ~4 | ~16% |
| **Total** | **25** | **100%** |

The techniques in this document target the 48% that vanilla Datomic queries cannot reach.

</framing>

---

## 2. Terminological Ground Truth

<terminology>

### "Datalog" in This Document

"Datalog" in the Braid context means the **Datomic-style EAV query language**. It is NOT
the academic logic programming language that is a subset of Prolog. Specifically:

**Braid's query language** uses:
- EDN (Extensible Data Notation) syntax, not Horn clause syntax
- Four find forms: Relation (`[:find ?x ?y]`), Scalar (`[:find ?x .]`),
  Collection (`[:find [?x ...]]`), Tuple (`[:find [?x ?y]]`)
- Pull expressions for entity-centric tree retrieval
- Named rules (recursive clause sets) for transitive closure
- Inputs (`:in $ %` for database and rules injection)
- Database functions (the FFI boundary, `DerivedFunction` trait in `spec/03-query.md` §3.3)
- `not`, `not-join`, `or`, `or-join` clauses with stratified evaluation

**What Braid's query language borrows from academic Datalog**: the declarative
pattern-matching flavor, bottom-up evaluation, and the fixpoint semantics
(`T_P^ω(D)` per `spec/03-query.md` §3.1).

**What it does NOT share**: Prolog's top-down SLD resolution, cut operator, assert/retract
side effects, meta-predicates, or unification over function symbols.

### Consequence for This Analysis

Several things initially framed as "enhancements from Prolog" are actually standard Datomic
features that Braid's spec already includes:

| Feature | Status in Braid |
|---------|----------------|
| Named rules (recursive clause sets) | Standard Datomic feature. `spec/03-query.md` §3.3: `Clause::RuleApplication` |
| Database functions (FFI) | Standard Datomic feature. `spec/03-query.md` §3.3: `DerivedFunction` trait |
| Negation-as-failure | Standard Datomic feature. `spec/03-query.md` §3.3: `Clause::NotClause` |
| Disjunction (`or`, `or-join`) | Standard Datomic feature. `spec/03-query.md` §3.3: `Clause::OrClause` |

The techniques below are things that go **beyond** what Datomic provides.

### "Database Function" vs "FFI"

These terms are used interchangeably. In Datomic parlance, a "database function" is a
function stored in the database and invocable from queries. In Braid's spec, the equivalent
is a `DerivedFunction` registered via `register_ffi()`. When this document says "register
`smt_check/2` as a database function," it means implement it as a `DerivedFunction` and
register it with the query engine.

</terminology>

---

## 3. From Logic Programming

<logic_programming>

### 3.1 Unification for EDN Value Destructuring

<technique_unification>

#### The Problem

Datomic-style pattern matching operates at the datom level: `[?e :attr ?v]` binds `?v` to
the entire value. But EDN values can be compound structures — vectors, maps, sets, symbols.
The property vocabulary stores properties as multi-valued sets per element:

```edn
[1042 :inv/properties [:append_only :grow_only :immutable_datoms] 1001 true]
```

Standard Datomic matching binds `?v` to `[:append_only :grow_only :immutable_datoms]` as
an opaque blob. To check whether `:append_only` is among the properties, we need to
destructure the vector.

#### The Technique

Prolog's full unification algorithm matches arbitrary term structures with variables. We
don't need the full algorithm — no function symbols, no occurs check — just structural
descent into EDN collections. Two database functions suffice:

**`member/2`**: Given a collection (vector/set) and an element, succeeds if the element
is a member. This is the critical one for property vocabulary checks.

```clojure
;; "Find all elements that commit to :append_only"
[:find ?e
 :where [?e :element/properties ?props]
        [(member :append_only ?props)]]
```

**`unify/2`**: Given two EDN values, unifies them structurally — descends into vectors
(position-matching) and maps (key-matching), binding variables to sub-values.

```clojure
;; "Find elements whose violation condition mentions 'mutation'"
[:find ?e ?condition
 :where [?e :element/violation-condition ?vc]
        [(unify {:keyword ?kw :condition ?condition} ?vc)]
        [(re-matches #".*mutation.*" ?condition)]]
```

#### Formal Justification

Prolog's unification is sound and complete for first-order terms (Robinson 1965). Our
restriction to EDN structures is a strict subset — no function symbols beyond the fixed
set of EDN constructors (vector, map, set, list). This makes unification:

1. **Always terminating**: EDN structures are finite trees. No occurs check needed
   (EDN cannot express infinite terms).
2. **Deterministic**: EDN maps have unique keys. Vector unification is position-based.
3. **Monotonic**: Adding a new datom can only add new `member/2` results, never remove
   existing ones. Therefore `member/2` preserves the CALM property.

Both functions map to **Stratum 0** (monotonic, no coordination needed).

#### Implementation Sketch

```rust
/// EDN member check — registered as database function "member"
pub struct MemberFunction;
impl DerivedFunction for MemberFunction {
    fn name(&self) -> &str { "member" }
    fn evaluate(&self, inputs: &[Value]) -> Result<Value, FfiError> {
        let (element, collection) = (&inputs[0], &inputs[1]);
        match collection {
            Value::Vector(v) => Ok(Value::Boolean(v.contains(element))),
            Value::Set(s)    => Ok(Value::Boolean(s.contains(element))),
            _ => Err(FfiError::TypeMismatch {
                expected: "vector or set",
                got: collection.type_name(),
            }),
        }
    }
}
```

#### What This Catches from the Extraction

The 8 entailment gaps found in `FULL_EXTRACTION_RESULTS.md` §1.2 are all cases where
`member/2` is needed to check property membership in multi-valued attribute datoms.
Without `member/2`, the entailment check query cannot inspect individual properties
within a committed property set. This directly enables the entailment chain query
from `HARVEST_SEED.md` §10.

</technique_unification>

---

### 3.2 Subsumption-Based Tabling (XSB Prolog)

<technique_tabling>

#### The Problem

Braid's query engine uses semi-naive evaluation for fixpoint computation. This handles
the simple case of recursive rules over acyclic graphs. But the spec dependency graph
can contain cycles:

- `spec/17-crossref.md` documents the invariant dependency graph, which has 5 depth
  levels. While no cycles were found in the current spec, the data model does not
  prevent them — and for general coherence checking over arbitrary DDIS specifications,
  cycles are expected.
- `spec/03-query.md` §3.3 includes `GraphError::CycleDetected(SCCResult)`, which uses
  Tarjan's algorithm for SCC detection. This tells us cycles are an anticipated case.

A naive recursive Datalog query over a cyclic graph either diverges (infinite loop) or
requires the graph algorithm to detect and short-circuit the cycle. Tabling provides a
principled solution.

#### The Technique

XSB Prolog's tabling (a.k.a. SLG resolution) memoizes subgoal results. When a recursive
call re-encounters a subgoal already under evaluation, it returns the partial answer
computed so far instead of re-entering the recursion. After all paths complete, the
system iterates to a fixpoint over the tabled results.

For Braid's bottom-up evaluation, the corresponding technique is **subsumption-based
tabling**: maintain a table of `(subgoal → result-set)` entries. When a recursive rule
fires and produces a subgoal already in the table:

1. Return the current result-set from the table (partial answer)
2. Continue evaluation
3. If the result-set grows, re-enter dependent rules (fixpoint iteration)
4. Terminate when no table changes

#### Formal Justification

SLG resolution is proven to:
- **Terminate for all Datalog programs** (including those with cycles through negation
  under well-founded semantics). Reference: Chen & Warren 1996,
  "Tabled Evaluation with Delaying for General Logic Programs."
- **Compute the well-founded model**: The well-founded semantics is the unique intended
  semantics for Datalog with negation. SLG resolution computes it correctly for all
  stratifiable programs, and provides a three-valued answer (true/false/undefined) for
  unstratifiable programs.
- **Preserve soundness**: Every answer computed via tabling is a logical consequence of
  the program. Tabling never produces unsound answers — it only adds termination
  guarantees.

The key insight: semi-naive evaluation and tabling are not alternatives. Semi-naive is
an optimization of naive bottom-up evaluation. Tabling is a mechanism for handling
recursive subgoals. The two compose: use semi-naive evaluation with tabled subgoals.

#### Interaction with Braid's Stratum Classification

Tabling is needed at **Stratum 1** (graph traversal, transitive closure). The canonical
use case is the reachability query:

```clojure
;; "Can signal-type reach its resolution target?"
;; This rule is recursive and may encounter cycles in the dispatch topology.
[[(can-reach ?a ?b) [?a :dispatches-to ?b]]
 [(can-reach ?a ?c) [?a :dispatches-to ?b] (can-reach ?b ?c)]]
```

Without tabling, this diverges on a cycle `A → B → C → A`. With tabling:
1. First call: `can-reach(A, ?)` — starts computing, enters table
2. Reaches `can-reach(A, ?)` again via `A → B → C → A` — finds it in table,
   returns partial result `{B, C}` instead of re-entering
3. No new facts added — fixpoint reached, terminates with `{B, C}`

This stays monotonic (Stratum 1) because the reachability relation is monotonic:
adding edges can only add reachable nodes, never remove them.

#### When It's Actually Needed

**Question Q-07 from HARVEST_SEED.md**: "Is tabling needed for Stage 0 or only for
Stage 2+ cyclic graphs?"

**Answer**: Needed at Stage 0 if the spec dependency graph (from `spec/17-crossref.md`)
has cycles. The current dependency graph has depth 5 and is acyclic, but:
1. The coherence engine should handle arbitrary DDIS specs, not just the current one
2. Adding a single backward dependency creates a cycle
3. The dispatch topology (`signal → resolution`) is also queried for reachability
   and could contain cycles in more complex specifications

**Recommendation**: Implement tabling at Stage 0 as a defensive measure. The cost is
small (a hash table of subgoal → result-set), the benefit is correctness for all
future specifications.

</technique_tabling>

---

### 3.3 CLP Framing for Property Constraints

<technique_clp>

#### The Problem

The property vocabulary defines 109 properties with 12 incompatibility constraints
and 16 entailment constraints. A spec element must satisfy all constraints simultaneously:
no committed property may be incompatible with any other committed property, and every
entailed property must be present.

#### The Technique

Constraint Logic Programming (CLP) reframes this as a constraint satisfaction problem
(CSP) over finite domains. In CLP(FD) terms:

```prolog
:- use_module(library(clpfd)).
check_coherent(Element) :-
    committed_properties(Element, Props),
    all_distinct_domains(Props),
    forall(incompatible(A, B),
           \+ (member(A, Props), member(B, Props))),
    forall((member(P, Props), entails(P, Q)),
           member(Q, Props)).
```

#### Why CLP Is a Framing, Not an Implementation

The domain is small: 109 properties, 12 incompatibilities, 16 entailments. A full CLP
solver (like SICStus CLP(FD) or Gecode) is architectural overkill. The incompatibility
check is O(|P|² × |I|) where |P| ≤ 109, |I| = 12 — at most ~143,000 comparisons,
which takes microseconds.

The CLP framing is valuable not as an implementation strategy but as a **design lens**:

1. **Constraint propagation**: If an element commits to `append_only`, propagate the
   entailments (`grow_only`, `immutable_datoms`, `retraction_as_assertion`) immediately.
   The extraction pipeline should output the transitive closure of entailments, not
   just the directly stated properties.

2. **Arc consistency**: Before running expensive semantic checks, verify that every
   property in the committed set is arc-consistent with every other property —
   no pair triggers an incompatibility. This is a necessary condition that can be
   checked in O(1) per pair (table lookup).

3. **Constraint composition**: When checking cross-element coherence, the constraint
   set is the union of both elements' property sets. Two elements individually coherent
   may be jointly incoherent. CLP formalizes this as constraint intersection.

#### Implementation in Datomic-Style Queries

Express `incompatible/2` and `entails/2` as datoms in the store itself (schema-as-data, C3):

```edn
;; Incompatibility rules as datoms
[10001 :incompatible/left :append_only 10000 true]
[10001 :incompatible/right :mutable_datoms 10000 true]
[10001 :incompatible/reason "Append-only means no mutation" 10000 true]

;; Entailment rules as datoms
[10013 :entails/from :append_only 10000 true]
[10013 :entails/to :grow_only 10000 true]
[10013 :entails/reason "Append-only implies growth" 10000 true]
```

Then the coherence check is a standard Datomic-style query — no external solver:

```clojure
;; Incompatibility violation — Stratum 2 (uses not= which is negation)
[:find ?e1 ?e2 ?pa ?pb
 :where [?e1 :element/property ?pa]
        [?e2 :element/property ?pb]
        [?r :incompatible/left ?pa]
        [?r :incompatible/right ?pb]
        [(not= ?e1 ?e2)]]

;; Missing entailment — Stratum 2 (negation-as-failure)
[:find ?e ?pb
 :where [?e :element/property ?pa]
        [?r :entails/from ?pa]
        [?r :entails/to ?pb]
        (not [?e :element/property ?pb])]
```

This is the **self-bootstrap move**: the property vocabulary becomes schema datoms,
the constraints become datoms, and coherence checking becomes a standard query. The
coherence engine is not a separate system — it is the query engine operating on the
property vocabulary stored as its own data (C3, C7).

</technique_clp>

---

### 3.4 DCG-Style Parsing for Structured Fragments

<technique_dcg>

#### The Problem

The coherence engine needs to parse spec elements — extracting structured claims from
markdown prose. Currently, LLM extraction handles all parsing. But spec elements contain
both structured and unstructured portions:

**Structured** (deterministic, parseable):
- Element IDs: `INV-STORE-001`, `ADR-QUERY-003`
- Cross-references: `"Traces to": SEED.md §4`, `"Dependencies": [INV-STORE-001]`
- Verification tags: `V:PROP`, `V:KANI`, `V:TEST`
- Stage annotations: `Stage: 0`, `Stage: 1+`
- Falsification headers: `"Falsification":`

**Unstructured** (semantic, requires LLM):
- The invariant statement itself (natural language)
- The rationale section of an ADR
- Confidence assessments

#### The Technique

Definite Clause Grammars (DCGs) are Prolog's formalism for parsing — essentially
context-free grammars with logic variable threading. For the structured portions
of spec elements, DCG-style deterministic parsing can replace LLM extraction:

```prolog
% Prolog-style DCG (for design clarity; implementation is in Rust)
invariant(inv(Id, Statement, Falsification)) -->
    header(Id), newline,
    statement_block(Statement), newline,
    falsification_block(Falsification).

header(Id) --> "### ", inv_id(Id), ": ", rest_of_line(_Title).
inv_id(id(Namespace, Number)) -->
    "INV-", namespace(Namespace), "-", digits(Number).
```

#### Why This Is Better Than Pure LLM Extraction

1. **Deterministic**: A regex or parser combinator extracts `INV-STORE-001` with 100%
   accuracy. An LLM extracts it with ~99.5% accuracy. Over 248 elements, that's the
   difference between 0 errors and ~1 error.

2. **Fast**: Parsing is O(n) in input length. LLM extraction is O(API_latency).
   Structured fields can be extracted in microseconds vs. seconds.

3. **Verifiable**: A parser's behavior is testable with unit tests. An LLM's behavior
   is stochastic and requires statistical validation.

4. **Budget-preserving**: Every token sent to the LLM costs attention budget. Extracting
   structured fields mechanically preserves LLM budget for the hard problem (semantic
   property classification).

#### Implementation

In Rust, this maps to parser combinators (nom, pest, or winnow) or regex:

```rust
// Not a full implementation — structural sketch
lazy_static! {
    static ref INV_ID: Regex = Regex::new(
        r"INV-(?P<ns>[A-Z]+)-(?P<num>\d{3})"
    ).unwrap();
    static ref TRACES_TO: Regex = Regex::new(
        r"\*\*Traces to\*\*:\s*(.+)"
    ).unwrap();
    static ref STAGE: Regex = Regex::new(
        r"\*\*Stage\*\*:\s*(\d+\+?)"
    ).unwrap();
}
```

The structured extraction outputs datoms directly:

```edn
[2001 :element/id :INV-STORE-001 2000 true]
[2001 :element/namespace :STORE 2000 true]
[2001 :element/traces-to "SEED.md §4" 2000 true]
[2001 :element/verification [:V:PROP :V:KANI] 2000 true]
[2001 :element/stage 0 2000 true]
```

The LLM then receives only the unstructured portions (statement text, rationale,
falsification condition prose) and classifies them into the property vocabulary.
This hybrid approach reduces LLM token consumption by ~40% (structured fields are
~40% of a typical spec element's text).

#### Interaction with the Extraction Pipeline

This technique modifies Phase 1 of the production pipeline
(from `HARVEST_SEED.md` §11):

```
Phase 1a: STRUCTURAL EXTRACT (deterministic, per-element, instantaneous)
  Parser: regex/combinator on markdown structure
  Output: element ID, namespace, stage, traces-to, verification tags, dependencies
  Cost: ~0 tokens, microseconds

Phase 1b: SEMANTIC EXTRACT (Sonnet, per-element, parallelizable)
  Input: unstructured prose portions + full property vocabulary
  Output: properties_committed, properties_assumed, violation_condition, confidence
  Cost: ~300 tokens per element (down from ~500)
```

</technique_dcg>

</logic_programming>

---

## 4. From Automated Theorem Proving

<automated_theorem_proving>

### 4.1 SMT Solving (Z3) as Database Function

<technique_smt>

#### The Problem

Some coherence checks involve **quantitative constraints** that Datalog cannot express.
The most critical example is contradiction C-01 from the extraction results:

> **C-01**: INV-SEED-004 pins intentions at π₀ (full datoms, no degradation). INV-SEED-002
> limits output to ≤ budget. If `|intentions_at_π₀| > budget`, both cannot be satisfied.

This is a linear arithmetic problem: given a set of constraint inequalities extracted
from budget-related invariants, determine whether they are simultaneously satisfiable.
Datalog has no arithmetic reasoning beyond simple comparisons (`<`, `>`, `=`).

#### The Technique

SMT (Satisfiability Modulo Theories) solving extends SAT solving with domain-specific
theories. Z3 is the standard solver. For Braid's coherence checks, the relevant theory
is **QF_LIA** (Quantifier-Free Linear Integer Arithmetic):

```smt2
; C-01: Budget cap vs. pinned intentions
(declare-const intentions_size Int)
(declare-const budget Int)
(declare-const min_output Int)
(assert (> intentions_size 0))         ; from INV-SEED-004: intentions exist
(assert (<= min_output budget))        ; from INV-SEED-002: output ≤ budget
(assert (= min_output intentions_size)); from INV-SEED-004: intentions at π₀
(assert (> intentions_size budget))    ; contradiction condition
(check-sat)
; Result: sat — the contradiction is realizable
```

#### Why Z3 and Not Full ATP

Full ATP (resolution-based, superposition, etc.) operates on first-order logic and is
semi-decidable — it may not terminate. For the coherence checking use case:

| Fragment | Decidability | Tool | Braid Use Case |
|----------|-------------|------|----------------|
| Propositional (SAT) | Decidable, NP-complete | MiniSat | Property incompatibility (but O(1) table lookup is simpler) |
| QF_LIA (linear arithmetic) | Decidable, NP-hard | Z3 | Budget constraints, threshold consistency |
| QF_UF (uninterpreted functions) | Decidable | Z3 | Abstract function composition checks |
| Full FOL | Semi-decidable | Vampire, E | Not needed — spec elements don't require FOL |

Only QF_LIA adds value beyond what Datalog already provides. Property incompatibility
is a finite-domain problem handled by table lookup. Function composition (QF_UF) is
theoretically useful but no extraction results required it. Budget constraints are the
sweet spot: too complex for Datalog, perfectly suited for SMT.

#### Implementation as Database Function

```rust
/// SMT satisfiability check — registered as database function "smt_check"
pub struct SmtCheckFunction;
impl DerivedFunction for SmtCheckFunction {
    fn name(&self) -> &str { "smt_check" }
    fn evaluate(&self, inputs: &[Value]) -> Result<Value, FfiError> {
        // inputs[0]: SMT-LIB2 formula as string
        // inputs[1]: timeout in milliseconds
        let formula = inputs[0].as_string()?;
        let timeout_ms = inputs[1].as_integer()? as u64;

        // Z3 as subprocess (same pattern as existing Go CLI: internal/consistency/smt.go)
        let result = z3_subprocess::check_sat(formula, timeout_ms)?;
        Ok(Value::Keyword(match result {
            SatResult::Sat => "sat",
            SatResult::Unsat => "unsat",
            SatResult::Unknown => "unknown",
        }))
    }
}
```

#### Stratum Classification

SMT checking maps to **Stratum 3** (authority computation, FFI to external solver).
It is NOT monotonic — adding constraints can change SAT to UNSAT — so it requires
a specific frontier (`Stratified` mode).

#### Graceful Degradation

Following the existing Go CLI pattern (`internal/consistency/smt.go`), Z3 availability
is checked at startup via `exec::LookPath("z3")`. If Z3 is not installed, the database
function returns `"unknown"` for all queries, and the coherence engine degrades
gracefully — skipping quantitative checks but still performing all Datalog-based checks.

#### Empirical Grounding

From the 25 tensions found in the extraction:
- **C-01** (budget cap vs. pinned intentions): SMT-checkable
- **C-03** (50-token floor vs. harvest-only mode): SMT-checkable (numerical contradiction)
- **L-03** (threshold inconsistency): SMT-checkable (numerical comparison)

That's 3 out of 25 tensions — **12%** of the total. Small but high-severity: C-01 and
C-03 are CONTRADICTION tier (the highest).

</technique_smt>

---

### 4.2 Bounded Model Checking at Crystallization Points

<technique_bmc>

#### The Problem

The specification defines 22 state transitions (the CLI commands, per
`spec/00-preamble.md`). An invariant should hold after ANY sequence of transitions,
not just the sequence tested by the test suite. Model checking asks: starting from
any valid state, after any sequence of k transitions, do all invariants hold?

#### The Technique

Full model checking (TLA+, Spin, UPPAAL) explores the entire state space. This is
impractical for Braid: the state space is infinite (the datom store can grow without
bound). **Bounded model checking** restricts exploration to k-step transition sequences.

For Braid, the natural boundary is the **crystallization point** — a barrier where
the bilateral loop evaluates coherence conditions. At crystallization:

1. The store is at a consistent cut (all agents synchronized per `spec/08-sync.md`)
2. The transition model is already in the store as datoms (commands = entities with
   preconditions and postconditions)
3. k=2 or k=3 is practical: check that any 2-3 consecutive commands preserve invariants

#### Formal Basis

Bounded model checking is sound for the property "no invariant violation exists within
k steps." It is NOT complete — violations at step k+1 are missed. This is acceptable
because:

1. The coherence engine is a **diagnostic tool**, not a formal verifier. False negatives
   (missed violations) are tolerable; false positives (spurious violations) are not.
2. The Kani harnesses in `spec/16-verification.md` provide deeper (though still bounded)
   verification at CI time. The coherence engine's BMC is a fast pre-check.
3. k=2 catches most composition errors: invariants that hold individually but break when
   two operations compose. This is the most common failure mode in practice.

#### Interaction with Braid's Architecture

BMC runs at **Stratum 5** (bilateral loop, barriered). It requires a consistent cut
and operates on the full transition model. The transition model datoms are:

```edn
[3001 :command/name :transact 3000 true]
[3001 :command/precondition [:valid_datoms] 3000 true]
[3001 :command/postcondition [:store_grows :tx_recorded] 3000 true]
[3002 :command/name :query 3000 true]
[3002 :command/precondition [:valid_expression :frontier_available] 3000 true]
[3002 :command/postcondition [:result_deterministic :provenance_recorded] 3000 true]
```

The BMC query: "for all pairs of commands (c1, c2), does executing c1 then c2 from
any state satisfying c1's precondition produce a state satisfying all invariants?"

This is a Stratum 5 query because it involves state enumeration (non-monotonic) and
requires barrier synchronization for correctness.

#### Implementation Staging

**Stage 2+.** BMC requires:
1. The full transition model as datoms (depends on all 22 commands being specified)
2. Invariant postcondition extraction (requires a complete property vocabulary mapping
   from invariants to postconditions)
3. A state representation that can be symbolically manipulated

This is not Stage 0 work. The immediate value (catching composition errors) is lower
than the cost of building the full transition model. Deferred.

</technique_bmc>

---

### 4.3 Craig Interpolation for Minimal Contradiction Explanations

<technique_interpolation>

#### The Problem

When two spec elements contradict, the diagnosis needs to explain WHY. Currently, the
extraction results list the two elements and their conflicting properties. But a property
list doesn't explain the shared concept that creates the incompatibility.

Example from C-05:
> INV-STORE-014 commits to `every_command_is_transaction`. ADR-STORE-005 commits to
> `calm_compliant`. These are not directly incompatible properties — the incompatibility
> arises through the shared concept of "provenance writes as side effects."

#### The Technique

Craig's interpolation theorem (1957): if A ∧ B is unsatisfiable, there exists a formula
I (the interpolant) such that:
1. A → I (I follows from A alone)
2. I ∧ B is unsatisfiable (I alone is enough to contradict B)
3. I uses only symbols common to both A and B

The interpolant is the **minimal shared concept** that makes A and B incompatible.

For formal (logical) contradictions, interpolation can be computed mechanically by
SMT solvers (Z3 supports interpolation). For natural-language spec elements, the
interpolation is an **LLM task**: "Given these two contradicting elements, identify
the minimal shared concept that makes them incompatible."

#### Why LLM-Mediated Interpolation

The spec elements are natural language. Mechanical interpolation requires a formal
representation of each element's meaning, which is exactly what the property vocabulary
provides — but only partially. The property vocabulary captures WHAT is committed
(`:append_only`, `:calm_compliant`), not the full semantic content.

The interpolation prompt:

```
Given two spec elements that contradict:

Element A: [full text of element A]
Properties: [committed properties]

Element B: [full text of element B]
Properties: [committed properties]

Identified conflict: [property or tension description]

Task: Identify the MINIMAL shared concept between A and B that creates
the incompatibility. The interpolant should:
1. Follow logically from A alone
2. Be sufficient to contradict B
3. Use only concepts mentioned in both A and B
4. Be as concise as possible (1-2 sentences)
```

#### Storage as Datoms

The interpolant is stored as a datom linking the two elements:

```edn
[4001 :interpolant/element-a :INV-STORE-014 4000 true]
[4001 :interpolant/element-b :ADR-STORE-005 4000 true]
[4001 :interpolant/concept "provenance writes from read-only queries" 4000 true]
[4001 :interpolant/direction "A implies provenance for all commands; B requires
       CALM-compliant reads; provenance writes make reads non-monotonic" 4000 true]
```

This enriches the contradiction record beyond "these two elements conflict" to
"these two elements conflict because of THIS shared concept."

#### Implementation Staging

**Stage 2+.** Craig interpolation requires:
1. Detected contradictions (the input)
2. LLM integration (for natural-language interpolation)
3. Stored contradiction pairs (the context)

The value is diagnostic — it helps humans understand contradictions, not detect them.
Detection is handled by cheaper mechanisms (property incompatibility, entailment gaps).
Interpolation adds explanatory power. Deferred to Stage 2+ when LLM integration
is mature.

</technique_interpolation>

</automated_theorem_proving>

---

## 5. From Computational Linguistics

<computational_linguistics>

### 5.1 Discourse Representation Theory (DRT)

<technique_drt>

#### The Problem

Spec elements reference shared entities implicitly. When INV-STORE-001 says "the datom
store" and INV-QUERY-001 says "queries over the store," the connection between these
two references is implicit — they both refer to the same conceptual entity. Currently,
these connections depend on explicit cross-references (`spec/17-crossref.md`).

The full extraction found 5 medium-severity tensions (M-06 through M-10) involving
implicit dependencies not captured by the explicit crossref graph. These are exactly
the cases where DRT discourse referent tracking would add value.

#### The Technique

DRT (Kamp 1981, Kamp & Reyle 1993) is a formal semantics framework that tracks how
meaning accumulates across discourse. Its key construct is the **discourse referent**
— a variable introduced by one sentence that subsequent sentences can reference.

For spec coherence, discourse referents are the entities mentioned across multiple
spec elements. During extraction, each entity mentioned in a spec element gets a
discourse referent:

```edn
;; INV-STORE-001 mentions "the datom store"
[5001 :discourse/element :INV-STORE-001 5000 true]
[5001 :discourse/referent :datom_store 5000 true]
[5001 :discourse/surface-form "the datom store" 5000 true]

;; INV-QUERY-001 also mentions "the store"
[5002 :discourse/element :INV-QUERY-001 5000 true]
[5002 :discourse/referent :datom_store 5000 true]
[5002 :discourse/surface-form "the store" 5000 true]

;; INV-MERGE-001 mentions "merging two stores"
[5003 :discourse/element :INV-MERGE-001 5000 true]
[5003 :discourse/referent :datom_store 5000 true]
[5003 :discourse/surface-form "stores" 5000 true]
```

#### What This Enables

A Stratum 1 query can now find all elements sharing discourse referents — implicit
cross-references that the explicit crossref graph misses:

```clojure
;; Find all elements that share a discourse referent with a given element
[:find ?other ?shared-referent
 :in $ ?target
 :where [?d1 :discourse/element ?target]
        [?d1 :discourse/referent ?shared-referent]
        [?d2 :discourse/referent ?shared-referent]
        [?d2 :discourse/element ?other]
        [(not= ?target ?other)]]
```

This query is **monotonic** (Stratum 1): adding new elements can only add new shared
referents, never remove them. It discovers implicit dependency links that help
focus expensive Stratum 5 checks.

#### Coreference Resolution

The challenge is **coreference**: "the datom store," "the store," "stores," and
"the append-only store" all refer to the same concept. DRT handles this via
coreference chains. For Braid, the LLM extraction pipeline performs coreference
resolution during extraction:

```
Extraction prompt addition:
"For each entity mentioned in this spec element, identify it with a canonical
referent name from this list: [datom_store, query_engine, frontier, transaction,
harvest_pipeline, seed_assembler, merge_function, guidance_system, ...].
If an entity is not in the list, propose a new canonical name."
```

The canonical referent list is itself stored as datoms (schema-as-data, C3), and
extends as new concepts are encountered.

#### Implementation Staging

**Stage 1+.** Discourse referent extraction requires LLM integration (for
coreference resolution). The canonical referent vocabulary needs to be defined.
This is not Stage 0 work, but it should be designed to integrate with the
Stage 0 datom schema so that discourse referents can be added later without
schema migration.

**Stage 0 preparation**: Reserve the `:discourse/element`, `:discourse/referent`,
and `:discourse/surface-form` attributes in the schema. Leave them unpopulated
until LLM integration is available.

</technique_drt>

---

### 5.2 Semantic Role Labeling (SRL)

<technique_srl>

#### The Problem

Property classification tells you WHAT an element commits to (`:append_only`,
`:calm_compliant`). But it doesn't tell you HOW the commitment is made — who
does what to whom. Two elements may commit to the same property but through
contradictory mechanisms.

Example from C-05:
- INV-STORE-014: "Every CLI command **produces** a transaction record."
  Agent: CLI command. Action: produces. Patient: transaction record.
- ADR-STORE-005: "Monotonic queries **run** without coordination."
  Agent: query. Action: runs. Patient: (without coordination).

The property-level check sees both elements commit to properties involving
transactions. The SRL check reveals the agent-action-patient mismatch: one
says "commands produce transactions" (including queries), the other says
"queries run coordination-free" (implying no writes).

#### The Technique

Semantic Role Labeling (Gildea & Jurafsky 2002) identifies:
- **Agent**: WHO performs the action
- **Action**: WHAT is done (the predicate)
- **Patient/Theme**: WHAT is acted upon
- **Instrument/Manner**: HOW it is done
- **Condition**: WHEN/IF it applies

For spec elements, SRL extraction produces triples stored as datoms:

```edn
;; INV-STORE-001: "The datom store never deletes or mutates an existing datom"
[6001 :srl/element :INV-STORE-001 6000 true]
[6001 :srl/agent "datom store" 6000 true]
[6001 :srl/action "never deletes or mutates" 6000 true]
[6001 :srl/patient "existing datom" 6000 true]

;; INV-QUERY-001: "The query engine rejects non-monotonic constructs in Monotonic mode"
[6002 :srl/element :INV-QUERY-001 6000 true]
[6002 :srl/agent "query engine" 6000 true]
[6002 :srl/action "rejects" 6000 true]
[6002 :srl/patient "non-monotonic constructs" 6000 true]
[6002 :srl/condition "in Monotonic mode" 6000 true]
```

#### What This Enables

A more targeted contradiction detector than property-based incompatibility:

```clojure
;; Elements sharing agent and patient but with contradictory actions
[:find ?e1 ?e2 ?agent ?patient ?a1 ?a2
 :where [?s1 :srl/element ?e1]
        [?s1 :srl/agent ?agent]
        [?s1 :srl/patient ?patient]
        [?s1 :srl/action ?a1]
        [?s2 :srl/element ?e2]
        [?s2 :srl/agent ?agent]
        [?s2 :srl/patient ?patient]
        [?s2 :srl/action ?a2]
        [(not= ?e1 ?e2)]
        [(not= ?a1 ?a2)]]
```

This query finds pairs where the same agent acts on the same patient but with
different actions. These are high-probability contradiction candidates — worth
escalating to the expensive NLI check at Stratum 5.

#### Why SRL Over Full Dependency Parsing

Full syntactic analysis (constituency parsing, dependency parsing) produces
parse trees with 30+ relation types. For spec coherence, we need only 5 roles
(agent, action, patient, instrument, condition). SRL directly produces these
without the intermediate syntactic representation. This is strictly more
efficient and equally informative for the coherence checking use case.

#### Implementation Staging

**Stage 1+.** SRL extraction requires LLM integration. The extraction prompt
extends naturally:

```
For each spec element, in addition to property classification, extract:
- Agent: the entity performing the action
- Action: the verb/predicate (what is done)
- Patient: the entity being acted upon
- Condition: any qualifying conditions (if/when/unless)
```

</technique_srl>

---

### 5.3 Natural Language Inference (NLI) as Database Function

<technique_nli>

#### The Problem

~16% of tensions found in the extraction (4 of 25) require semantic reasoning that
goes beyond property checking, graph reachability, and SRL matching. These are cases
where two elements' statements interact in ways that only emerge from understanding
the natural-language meaning.

Example: H-03 (fitness monotonicity under-specified). INV-BILATERAL-003 claims
`F(S) ≥ F(S_prev)` (monotonic improvement). But "improvement" depends on the
weights of the seven fitness components (UNC-BILATERAL-001, confidence 0.6).
No property-level check catches this — it requires understanding that the
monotonicity claim is contingent on unstated weight assumptions.

#### The Technique

Natural Language Inference (NLI) classifies a (premise, hypothesis) pair as:
- **Entailment**: The premise logically implies the hypothesis
- **Contradiction**: The premise logically contradicts the hypothesis
- **Neutral**: Neither entailment nor contradiction

This maps directly to the coherence checking task: for each pair of spec elements
that share discourse referents or dependency edges, classify whether one entails,
contradicts, or is neutral with respect to the other.

#### Implementation as Database Function

```rust
/// NLI classification — registered as database function "nli"
pub struct NliFunction {
    llm: Box<dyn LlmProvider>,
}
impl DerivedFunction for NliFunction {
    fn name(&self) -> &str { "nli" }
    fn evaluate(&self, inputs: &[Value]) -> Result<Value, FfiError> {
        // inputs[0]: premise text (spec element A's statement)
        // inputs[1]: hypothesis text (spec element B's statement)
        // inputs[2]: context (shared properties, discourse referents)
        let premise = inputs[0].as_string()?;
        let hypothesis = inputs[1].as_string()?;
        let context = inputs[2].as_string()?;

        let prompt = format!(
            "Classify the relationship between these two specification statements.\n\n\
             Context: {context}\n\n\
             Premise: {premise}\n\n\
             Hypothesis: {hypothesis}\n\n\
             Classification (one of: entailment, contradiction, neutral):\n\
             Confidence (0.0 to 1.0):\n\
             Explanation (1-2 sentences):"
        );

        let response = self.llm.complete(&prompt)?;
        Ok(Value::Map(parse_nli_response(&response)?))
    }
}
```

#### Cost Control

NLI is expensive: each call invokes an LLM. For 248 elements, the pairwise check is
O(248² / 2) ≈ 30,628 pairs — each costing ~1000 tokens. This is prohibitive.

The cost control strategy uses the stratum hierarchy as a filter cascade:

```
Stage 0-4 filters (subsecond, no LLM cost):
  248 elements → remove pairs with no shared properties → ~2000 pairs
  → remove pairs with no shared discourse referents → ~500 pairs
  → remove pairs with no dependency edges → ~200 pairs
  → remove pairs passing all Stratum 0-4 checks → ~50 pairs

Stage 5 NLI (expensive, LLM cost):
  ~50 pairs × 1000 tokens = ~50K tokens
  Expected contradictions: ~4 (based on extraction results)
```

This reduces the NLI cost from ~30M tokens to ~50K tokens — a 600x reduction.

#### Stratum Classification

NLI maps to **Stratum 5** (bilateral loop, barriered). It is inherently non-monotonic
(adding new information can change an entailment to a contradiction) and requires a
consistent snapshot of the store for correctness.

#### Calibration

NLI confidence thresholds need calibration against the known contradictions:

| Contradiction | Expected NLI Result | Threshold |
|--------------|-------------------|-----------|
| C-01 (budget vs. intentions) | Contradiction, high confidence | ≥ 0.9 |
| C-02 (CYCLE vs. human-gated) | Contradiction, high confidence | ≥ 0.9 |
| C-03 (token floor vs. harvest-only) | Contradiction, high confidence | ≥ 0.9 |
| C-04 (store_sole_truth vs. session) | Contradiction, medium confidence | ≥ 0.7 |
| C-05 (every-command-tx vs. CALM) | Contradiction, high confidence | ≥ 0.9 |

The 5 known contradictions serve as the calibration set. If the NLI function fails
to classify any of them as "contradiction" with sufficient confidence, the threshold
or prompt needs adjustment.

#### Implementation Staging

**Stage 1+.** NLI requires LLM integration, which is budgeted for Stage 1.
The calibration set (5 known contradictions) provides a built-in acceptance test.

</technique_nli>

</computational_linguistics>

---

## 6. Unified Architecture: How Everything Fits Together

<unified_architecture>

### The Key Insight

The Braid query engine IS the coherence engine. Every technique in this document
maps into the existing six-stratum classification. No new query engine architecture
is needed — only specific enhancements at precise stratum boundaries.

### Architecture Diagram

```
EDNL datom store (.ednl files, append-only, G-set CvRDT)
    │
    ├── Schema layer: property vocabulary as datoms (CLP framing, §3.3)
    │   ├── :property/name, :property/category, :property/meaning
    │   ├── :incompatible/left, :incompatible/right, :incompatible/reason
    │   └── :entails/from, :entails/to, :entails/reason
    │
    ├── Fact layer: extraction results as datoms
    │   ├── :element/id, :element/namespace, :element/stage
    │   ├── :element/property (multi-value, cardinality :many)
    │   ├── :element/agent, :element/action, :element/patient (SRL, §5.2)
    │   ├── :discourse/referent (DRT, §5.1)
    │   └── :interpolant/concept (Craig interpolation, §4.3)
    │
    └── Query strata (coherence checks):
        │
        S0: Property lookup, entity attributes (basic Datomic queries)
            └── member/2, unify/2 for EDN value destructuring (§3.1)
        │
        S1: Cross-ref reachability, discourse referent graphs (§5.1)
            └── Subsumption-based tabling for cyclic rules (§3.2)
        │
        S2: Incompatibility violations, missing entailments (stratified negation)
            └── CLP-framed constraints as standard queries (§3.3)
        │
        S3: Budget satisfiability, threshold consistency
            └── smt_check/2 via Z3 subprocess (§4.1)
        │
        S4: Stage ordering, CALM violations (conservative)
            └── SRL-based agent/action/patient conflict detection (§5.2)
        │
        S5: Bilateral loop checks, NLI pairwise (expensive, gated)
            └── nli/3 via LLM (§5.3)
            └── Bounded model checking at crystallization (§4.2)
            └── Craig interpolation for explanations (§4.3)
```

### Cost Profile

The stratum hierarchy doubles as a cost filter:

| Stratum | Cost per Check | Techniques Used | Tensions Caught |
|---------|---------------|----------------|-----------------|
| S0 | O(1), microseconds | Property lookup, member/2 | ~5 of 25 (20%) |
| S1 | O(n·m), milliseconds | Reachability with tabling | ~3 of 25 (12%) |
| S2 | O(n²), milliseconds | Incompatibility/entailment queries | ~5 of 25 (20%) |
| S3 | O(SMT), seconds | Z3 for quantitative constraints | ~3 of 25 (12%) |
| S4 | O(n²), milliseconds | Stage ordering, CALM, SRL matching | ~5 of 25 (20%) |
| S5 | O(LLM), seconds-minutes | NLI, BMC, interpolation | ~4 of 25 (16%) |

**84% of tensions are caught by checks costing less than 1 second.** Only 16%
require the expensive LLM-backed Stratum 5 checks.

### Self-Bootstrap Verification

This architecture satisfies constraint C7 (self-bootstrap) at three levels:

1. **Schema is data**: The property vocabulary (109 properties, 12 incompatibilities,
   16 entailments) is stored as datoms in the store. Schema evolution is a transaction.

2. **Checks are queries**: Every coherence check is a standard Datomic-style query.
   No separate checking engine or external system.

3. **Results are datoms**: Every check result (violation found, entailment gap detected,
   NLI classification) is recorded as a datom in the store. The coherence engine's
   output is the same type as its input.

</unified_architecture>

---

## 7. What We Don't Need (and Why)

<exclusions>

### Prolog Runtime

The six-stratum query engine with tabling and stratified negation covers the useful
fragment of Prolog. Specifically:

| Prolog Capability | Braid Equivalent | Notes |
|-------------------|-----------------|-------|
| Horn clause resolution | Datomic rules (Clause::RuleApplication) | Standard Datomic feature |
| Negation-as-failure (\+) | Clause::NotClause (Stratum 2+) | Standard Datomic feature |
| assert/retract | transact() | Structural — datoms are immutable (C1) |
| Unification | member/2, unify/2 database functions | Restricted to EDN structures (§3.1) |
| Tabling (XSB) | Subsumption-based tabling (§3.2) | Needed enhancement |
| Cut (!) | Not needed | Braid uses bottom-up, not top-down |
| Meta-predicates | Not needed | No meta-programming in coherence checks |

Adding a Prolog runtime would introduce a second query language, a second evaluation
strategy, and a second data model — violating the principle that the store is the sole
source of truth (INV-INTERFACE-001).

### Full First-Order ATP

Full ATP operates on first-order logic (quantifiers, function symbols, equality). Spec
coherence checking does not require this power:

- **Quantifiers**: Not needed. Properties are finite sets. Universal quantification over
  properties is iteration over a 109-element domain.
- **Function symbols**: Not needed. EDN has fixed constructors (vector, map, set). No
  arbitrary function terms.
- **Equality**: Handled by Datomic's built-in equality (`=`, `not=`).

Z3 (SMT) is sufficient for the one case that needs arithmetic reasoning (budget
constraints). Full ATP (Vampire, E, SPASS) is architecturally overkill.

### Constituency / Dependency Parsing

Full syntactic analysis produces parse trees with 30+ relation types (nsubj, dobj, nmod,
amod, etc.). For spec coherence, SRL's 5 roles (agent, action, patient, instrument,
condition) capture all the information needed. Syntactic analysis is wasted computation.

### OWL / RDF-S Ontology Languages

The property vocabulary has 109 entries. OWL is designed for ontologies with thousands
to millions of concepts, class hierarchies, property restrictions, and open-world
reasoning. Using OWL for 109 properties would be like using a relational database for
a grocery list.

### Graph Neural Networks

GNNs can learn representations over graph-structured data. The spec dependency graph
is small enough (248 nodes, ~1200 edges) that explicit graph algorithms (Tarjan's SCC,
BFS reachability, topological sort) are more interpretable and more reliable.

</exclusions>

---

## 8. Implementation Staging and Dependencies

<staging>

### Stage 0 (Current Phase — Pre-Implementation)

These techniques are prerequisites for Stage 0 implementation:

| Technique | Component | Estimated LOC | Dependency |
|-----------|----------|---------------|------------|
| Property vocab as schema datoms | Store initialization | ~100 datoms | None |
| `member/2` database function | Query engine FFI | ~50 Rust | DerivedFunction trait |
| `unify/2` database function | Query engine FFI | ~100 Rust | DerivedFunction trait |
| Subsumption-based tabling | Query engine evaluation | ~200 Rust | Semi-naive evaluator |
| CLP constraint queries | Query catalog | ~4 queries | Property vocab datoms |
| DCG-style structural parsing | Extraction pipeline | ~150 Rust | Spec element schema |

**Total Stage 0 cost**: ~600 lines of Rust + ~100 datoms + ~4 query definitions

### Stage 1 (Budget-Aware Output + Guidance Injection)

These techniques require LLM integration:

| Technique | Component | Estimated LOC | Dependency |
|-----------|----------|---------------|------------|
| SMT/Z3 as database function | Query engine FFI | ~200 Rust | Z3 subprocess |
| DRT discourse referent extraction | LLM extraction pipeline | ~100 Rust + prompt | LLM provider |
| SRL triple extraction | LLM extraction pipeline | ~100 Rust + prompt | LLM provider |
| NLI as database function | Query engine FFI | ~150 Rust + prompt | LLM provider |

**Total Stage 1 cost**: ~550 lines of Rust + extraction prompt updates

### Stage 2+ (Branching + Deliberation)

| Technique | Component | Estimated LOC | Dependency |
|-----------|----------|---------------|------------|
| Bounded model checking | Crystallization pipeline | ~300 Rust | Transition model datoms |
| Craig interpolation | Contradiction enrichment | ~100 Rust + prompt | LLM provider + detected contradictions |

**Total Stage 2+ cost**: ~400 lines of Rust + prompts

### Dependency Graph

```
              ┌── member/2 ─────────────────────────────────────────┐
              │                                                     │
Property ─────┼── CLP constraint queries ──── Incompatibility check │
vocab datoms  │                               Entailment check      │
              │                                                     │
              └── unify/2 ──────────────────── Value destructuring  │
                                                                    │
Tabling ────────── Reachability queries ──── Dispatch gap detection  │
                                                                    │
DCG parsing ────── Structural extraction ──── Element datom creation │
                                                    │               │
                                            ┌───────┘               │
                                            ▼                       │
                                     LLM integration (Stage 1)      │
                                            │                       │
                              ┌─────────────┼──────────────┐        │
                              ▼             ▼              ▼        │
                           DRT/SRL       SMT/Z3          NLI/3      │
                           extraction    db function     db function │
                              │             │              │        │
                              └─────────────┼──────────────┘        │
                                            ▼                       │
                                  Contradiction enrichment ─────────┘
                                  (Craig interpolation)
                                  BMC at crystallization
```

</staging>

---

## 9. Formal Justification: Why These Techniques Are Sound

<formal_justification>

### Preservation of Store Invariants

Every technique must preserve the core algebraic properties of the datom store:

| Property | Requirement | All 10 Techniques Comply? |
|----------|-----------|--------------------------|
| Append-only (C1) | No datom is deleted or mutated | Yes — all techniques ADD datoms (check results, discourse referents, SRL triples) |
| Content-addressable (C2) | Identity by [e,a,v,tx,op] | Yes — datoms produced by checks follow the same identity rules |
| Schema-as-data (C3) | Schema defined as datoms | Yes — property vocabulary is stored as datoms, not config |
| CRDT merge (C4) | Merge = set union | Yes — all output datoms merge by set union. Two agents running the same check produce identical datoms (content-addressed) |
| Traceability (C5) | Every artifact traces to spec element | Yes — check results include `:check/source-element` and `:check/rule-applied` |
| Falsifiability (C6) | Every invariant has falsification condition | Yes — each technique includes specific violation conditions |
| Self-bootstrap (C7) | DDIS specifies itself | Yes — the coherence engine checks its own specification as first data |

### Monotonicity Classification

| Technique | Monotonic? | Stratum | CALM Status |
|-----------|-----------|---------|-------------|
| member/2 | Yes — adding values to a set can only add members | S0 | Coordination-free |
| unify/2 | Yes — adding structures can only add unification results | S0 | Coordination-free |
| Tabling | Preserves monotonicity of underlying rule | S1 | Coordination-free |
| CLP queries | Uses negation-as-failure (entailment gap detection) | S2 | Requires frontier |
| DCG parsing | Pure function (deterministic, no side effects) | S0 | Coordination-free |
| SMT/Z3 | Non-monotonic (adding constraints can change SAT → UNSAT) | S3 | Requires frontier |
| DRT referents | Yes — adding elements can only add referent links | S1 | Coordination-free |
| SRL triples | Yes — adding elements can only add triples | S1 | Coordination-free |
| NLI | Non-monotonic (new info can change entailment → contradiction) | S5 | Requires barrier |
| BMC | Non-monotonic (state space exploration) | S5 | Requires barrier |
| Interpolation | Non-monotonic (explanation depends on full context) | S5 | Requires barrier |

**7 of 10 techniques are monotonic and can run coordination-free.** Only SMT, NLI,
and BMC/interpolation require synchronization — and all three are already classified
at strata that require it (S3, S5).

### Termination Guarantees

| Technique | Terminates? | Bound |
|-----------|-----------|-------|
| member/2 | Always | O(|collection|) |
| unify/2 | Always | O(depth × breadth of EDN structure) |
| Tabling | Always for Datalog (Chen & Warren 1996) | Polynomial in |database| |
| CLP queries | Always (finite property domain) | O(|elements|² × |properties|) |
| DCG parsing | Always (finite input) | O(|element text|) |
| SMT/Z3 | Decidable for QF_LIA (with timeout) | NP-hard but bounded by timeout |
| DRT referents | Always (finite extraction) | O(|elements| × |referents|) |
| SRL triples | Always (finite extraction) | O(|elements|) |
| NLI | Always (LLM with token limit) | Bounded by LLM max tokens |
| BMC | Always (bounded by k) | O(|transitions|^k) |
| Interpolation | Always (LLM with token limit) | Bounded by LLM max tokens |

All techniques terminate. No technique introduces unbounded computation.

</formal_justification>

---

## 10. Empirical Grounding: What Our Extraction Found

<empirical_grounding>

### Mapping Tensions to Techniques

Each of the 25 tensions found in `FULL_EXTRACTION_RESULTS.md` can be checked by
a specific technique at a specific stratum:

| Tension | Tier | Checking Technique | Stratum |
|---------|------|-------------------|---------|
| C-01 | CONTRADICTION | SMT (budget arithmetic) | S3 |
| C-02 | CONTRADICTION | CLP query (incompatible commitments) | S2 |
| C-03 | CONTRADICTION | SMT (numerical threshold) | S3 |
| C-04 | CONTRADICTION | CLP query (property incompatibility) | S2 |
| C-05 | CONTRADICTION | SRL + NLI (agent/action conflict) | S4/S5 |
| H-01 | HIGH | CLP query (working_set_private vs write) | S2 |
| H-02 | HIGH | CLP query (property incompatibility) | S2 |
| H-03 | HIGH | NLI (under-specification detection) | S5 |
| H-04 | HIGH | NLI (deterministic routing + uncertainty) | S5 |
| H-05 | HIGH | Reachability with tabling (commit pathway) | S1 |
| M-01 | MEDIUM | SRL (query determinism vs provenance writes) | S4 |
| M-02 | MEDIUM | Entailment chain (missing entailment) | S2 |
| M-03 | MEDIUM | CLP query (CALM overgeneralization) | S2 |
| M-04 | MEDIUM | Stage ordering query | S1 |
| M-05 | MEDIUM | Stage ordering query | S1 |
| M-06 | MEDIUM | DRT (implicit cross-reference) | S1 |
| M-07 | MEDIUM | SRL (timing ambiguity in agent/action) | S4 |
| M-08 | MEDIUM | Reachability with tabling (escalation gap) | S1 |
| M-09 | MEDIUM | DRT (channel composition across elements) | S1 |
| M-10 | MEDIUM | DRT (guidance prune semantics) | S1 |
| L-01 | MINOR | CLP query (vocabulary precision) | S2 |
| L-02 | MINOR | SMT (numerical imprecision) | S3 |
| L-03 | MINOR | SMT (threshold comparison) | S3 |
| L-04 | MINOR | Stage ordering query | S1 |
| L-05 | MINOR | CLP query (vocabulary precision) | S2 |

### Technique Coverage Summary

| Technique | Tensions It Would Catch | % of Total |
|-----------|------------------------|------------|
| CLP constraint queries (§3.3) | C-02, C-04, H-01, H-02, M-02, M-03, L-01, L-05 | 32% |
| Reachability with tabling (§3.2) | H-05, M-04, M-05, M-08, L-04 | 20% |
| SMT/Z3 (§4.1) | C-01, C-03, L-02, L-03 | 16% |
| SRL (§5.2) | C-05 (partial), M-01, M-07 | 12% |
| DRT (§5.1) | M-06, M-09, M-10 | 12% |
| NLI (§5.3) | C-05 (final), H-03, H-04 | 12% |

Note: C-05 requires both SRL (to detect the agent/action conflict) and NLI (to
confirm the semantic contradiction). The percentages sum to >100% due to this overlap.

### Key Finding

**No single technique catches more than 32% of tensions.** The value comes from the
**composition** of techniques across strata, organized by the cost filter. The stratum
hierarchy ensures that cheap techniques run first, reducing the work for expensive ones.

</empirical_grounding>

---

## Appendix A: Relation to `spec/03-query.md` Type Definitions

The techniques in this document introduce new database functions that extend the
`DerivedFunction` trait defined in `spec/03-query.md` §3.3:

| Database Function | Input Types | Output Type | Stratum |
|-------------------|------------|-------------|---------|
| `member` | (Value, Collection) | Boolean | S0 |
| `unify` | (Value, Pattern) | BindingSet | S0 |
| `smt_check` | (String, Integer) | Keyword (sat/unsat/unknown) | S3 |
| `nli` | (String, String, String) | Map (classification, confidence, explanation) | S5 |

These extend the three core derived functions already specified:
1. `aleatory_uncertainty` (entropy computation)
2. `consequential_uncertainty` (DAG traversal)
3. `spectral_authority` (SVD)

The total registered function count at full implementation: 7.

## Appendix B: References

| Reference | Relevance |
|-----------|-----------|
| Robinson 1965, "A Machine-Oriented Logic Based on the Resolution Principle" | Unification algorithm (§3.1) |
| Chen & Warren 1996, "Tabled Evaluation with Delaying for General Logic Programs" | SLG resolution / tabling (§3.2) |
| Jaffar & Lassez 1987, "Constraint Logic Programming" | CLP framing (§3.3) |
| Pereira & Warren 1980, "Definite Clause Grammars for Language Analysis" | DCG parsing (§3.4) |
| de Moura & Bjørner 2008, "Z3: An Efficient SMT Solver" | SMT checking (§4.1) |
| Biere et al. 1999, "Symbolic Model Checking without BDDs" | Bounded model checking (§4.2) |
| Craig 1957, "Linear Reasoning" | Craig interpolation theorem (§4.3) |
| Kamp & Reyle 1993, "From Discourse to Logic" | Discourse Representation Theory (§5.1) |
| Gildea & Jurafsky 2002, "Automatic Labeling of Semantic Roles" | Semantic Role Labeling (§5.2) |
| Bowman et al. 2015, "A Large Annotated Corpus for Learning Natural Language Inference" | NLI (§5.3) |
| Hellerstein 2010, "The Declarative Imperative" | CALM theorem (throughout) |
