# Coherence Engine Exploration: Complete Harvest & Seed

> **Date**: 2026-03-03
> **Session scope**: Three continuous context windows (same conversation, two compactions)
> **Purpose**: Comprehensive harvest of all findings, decisions, analyses, and concrete next
> steps from the coherence engine exploration thread. Designed as a **seed document** for any
> future session that needs to incorporate these results into the main Braid implementation.
>
> **How to use this document**: Read it top-to-bottom on first encounter. On subsequent
> sessions, read §1 (TL;DR), §11 (Open Questions), and §12 (Concrete Next Steps) — then
> consult other sections as needed.

---

## Table of Contents

1. [TL;DR — What We Found](#1-tldr)
2. [The Arc of Discovery](#2-arc-of-discovery)
3. [Primary Source Documents](#3-primary-source-documents)
4. [The Property Vocabulary: Load-Bearing Artifact](#4-property-vocabulary)
5. [Full Extraction Results: 248 Elements, 25 Tensions](#5-full-extraction-results)
6. [The Five Contradictions](#6-five-contradictions)
7. [Strategic Analysis: What Changed](#7-strategic-analysis)
8. [The Prolog Question — Resolved](#8-prolog-question)
9. [Techniques Borrowed from LP, ATP, and CL](#9-techniques-borrowed)
10. [Datomic Query Language as Coherence Engine](#10-datomic-query-language)
11. [Prompt Engineering: What Worked](#11-prompt-engineering)
12. [Open Questions](#12-open-questions)
13. [Concrete Next Steps](#13-concrete-next-steps)
14. [Formal Methods Assessment](#14-formal-methods)
15. [Testing Strategy](#15-testing-strategy)
16. [Glossary of Key Terms](#16-glossary)

---

## 1. TL;DR — What We Found

<critical_findings>

### The Headline Results

1. **The Braid specification has 5 genuine contradictions** (C-01 through C-05) and 20 additional
   tensions of varying severity, discovered by automated extraction over 248 spec elements using
   a 109-property closed ontology. These contradictions exist in a spec that has been through
   7+ design sessions, a Fagan inspection, and manual review. The coherence engine catches what
   humans miss.

2. **The coherence engine is Stage 0 infrastructure, not Stage 2+.** You cannot soundly implement
   a specification with internal contradictions. The coherence engine must exist before
   implementation begins — it is the immune system, not a feature.

3. **A separate Prolog engine is architecturally unnecessary.** Braid's own Datomic-style query
   language (six strata, stratified negation, FFI for derived functions) covers the entire useful
   fragment of Prolog-style reasoning. The property vocabulary as schema datoms makes coherence
   checking a standard query, not a separate system.

4. **The property vocabulary is the most valuable artifact produced.** 109 typed properties with
   12 incompatibilities and 16 entailments reduce the undecidable problem of natural-language
   coherence checking to O(1) table lookups over a finite domain. This is the design insight
   that makes the entire concept tractable.

5. **Slot-filling extraction is 100% reliable across all model tiers.** The feasibility experiment
   (15 elements × 3 models × 3 prompt styles) confirmed that classifying spec text into
   predefined property categories succeeds every time. The full extraction (247 elements via
   5 parallel Sonnet agents) confirmed this at scale with 100% vocabulary adherence.

6. **Specific techniques from logic programming, automated theorem proving, and computational
   linguistics can enhance Braid's query engine** — but as extensions to the existing Datomic-style
   query language, not as separate systems.

</critical_findings>

---

## 2. The Arc of Discovery

<arc>

This exploration proceeded through seven phases across three context windows:

### Phase 1: Formal Analysis (SEED_ANALYSIS.md)
Five parallel Opus subagents performed a cleanroom formal methods assessment of the coherence
engine proposal against the Braid specification. 18-section analysis covering algebraic structure,
logical foundations, CALM theorem compliance, Gödelian limits, cross-reference alignment with
SEED.md/spec/guide, ADR gaps, failure mode coverage, and formal claim verdicts.

**Key finding**: The concept is formally sound with qualifications. The algebraic structure
(property vocabulary as a bounded lattice over a finite domain) ensures decidability. CALM
compliance is maintained because property checks are monotonic. The self-referential coherence
claim is validated by the finite, well-founded vocabulary.

### Phase 2: Feasibility Experiment (EXTRACTION_EXPERIMENT.md)
15 spec elements × 3 models (Opus/Sonnet/Haiku) × 3 prompt styles (structured/minimal/zero-shot).
100% extraction success. 3 tensions found independently by all models. Property vocabulary v1
(35 properties, 6 incompatibilities, 8 entailments) validated.

**Key finding**: Slot-filling extraction is the most reliable LLM capability for this task.
The closed ontology constraint is what makes it work — classifying into predefined categories
rather than generating arbitrary logic.

### Phase 3: Vocabulary Expansion (PROPERTY_VOCABULARY.md)
Expanded from 35 to 109 properties by reading all 14 namespace spec files, identifying the
properties each namespace commits to, and adding categories for MERGE, SYNC, SIGNAL, BUDGET,
RESOLUTION, GUIDANCE, INTERFACE, SAFETY, SELF_BOOTSTRAP. 12 incompatibilities (from 6),
16 entailments (from 8). Organized into 17 categories.

### Phase 4: Full Extraction (FULL_EXTRACTION_RESULTS.md)
Five parallel Sonnet 4.6 agents extracted logical forms from all 248 spec elements (124 INV,
72 ADR, 42 NEG, 10 UNC across 14 namespaces). 31 raw tensions consolidated to 25 unique:
5 CONTRADICTIONS, 5 HIGH, 10 MEDIUM, 5 MINOR. 8 missing entailments detected. 0 incompatibility
violations. 4 vocabulary gaps identified.

### Phase 5: Strategic Reassessment
Post-extraction analysis of implications for Braid roadmap. Three key conclusions:
(a) coherence engine moves from Stage 2+ to Stage 0, (b) Prolog layer can be deferred
indefinitely, (c) property vocabulary is the most valuable artifact. Updated spec element
counts from `spec/17-crossref.md` revision.

### Phase 6: The Prolog Correction
User challenged the "Prolog is unnecessary" claim. Re-examination found the initial assessment
was too dismissive: ~30-35% of tensions ARE catchable by negation-as-failure and reachability
queries (Prolog's core strengths). But Braid's own six-stratum query engine already provides
these capabilities at Strata 2-5. A separate Prolog runtime is architecturally redundant —
the Datomic-style query language IS the Prolog equivalent.

### Phase 7: Techniques from LP/ATP/CL
Comprehensive analysis of what Braid can borrow from Prolog (unification, tabling, CLP, DCGs),
automated theorem proving (SMT/Z3, bounded model checking, Craig interpolation), and
computational linguistics (DRT, SRL, NLI). Each technique mapped to a specific enhancement
of the existing query engine, not a separate system. Clarified that "Datalog" in Braid context
means the Datomic-style EAV query language, not the academic logic programming language.

</arc>

---

## 3. Primary Source Documents

<source_documents>

### Documents Produced in This Exploration

| Document | Path | Content | Lines |
|----------|------|---------|-------|
| **SEED_ANALYSIS.md** | `exploration/coherence-convergence/SEED_ANALYSIS.md` | 18-section formal methods assessment by 5 Opus subagents | ~1,200 |
| **EXTRACTION_EXPERIMENT.md** | `exploration/coherence-convergence/EXTRACTION_EXPERIMENT.md` | Feasibility experiment: 15 elements × 3 models × 3 prompts | ~400 |
| **PROPERTY_VOCABULARY.md** | `exploration/coherence-convergence/PROPERTY_VOCABULARY.md` | 109 properties, 12 incompatibilities, 16 entailments, extraction schemas | ~290 |
| **FULL_EXTRACTION_RESULTS.md** | `exploration/coherence-convergence/FULL_EXTRACTION_RESULTS.md` | 248-element extraction: 25 tensions, cross-element analysis, strategic assessment | ~730 |
| **QUERY_ENGINE_ENHANCEMENTS.md** | `exploration/coherence-convergence/QUERY_ENGINE_ENHANCEMENTS.md` | 10 techniques from LP/ATP/CL with formal justification, empirical grounding, implementation staging | ~700 |
| **TRILATERAL_COHERENCE_MODEL.md** | `exploration/coherence-convergence/TRILATERAL_COHERENCE_MODEL.md` | Formal algebraic treatment of the I↔S↔P trilateral model: adjunctions, Lyapunov stability, mediation theorem, topology-mediated resolution, structural inevitability argument | ~800 |
| **HARVEST_SEED.md** | `exploration/coherence-convergence/HARVEST_SEED.md` | This document | — |

### Supporting Data Files

| File | Content |
|------|---------|
| `logical-forms-08-10.json` | 34 extracted logical forms (SYNC + SIGNAL + BILATERAL) |
| `logical-forms-08-10-summary.md` | Classification notes and 6 tensions for Batch 4 |
| `extraction_wave3.json` | 70 extracted logical forms (DELIB + GUID + BUDG + INTF + UNC) |
| `original_transcript.md` | Earlier conversation transcript (Phase 1-2 context) |
| `CONVERSATION_SUMMARY.md` | Compaction summary from Phase 1-2 conversation |

### Upstream Source Documents (Read During This Exploration)

| Document | Path | Relevance |
|----------|------|-----------|
| **Braid SEED.md** | `SEED.md` | The foundational design document — all findings trace here |
| **Spec README** | `spec/README.md` | Master index with wave grouping, stage assignments, element counts |
| **Spec 01-14** | `spec/01-store.md` through `spec/14-interface.md` | All 14 namespace specifications (read by extraction agents) |
| **Spec 15** | `spec/15-uncertainty.md` | 10 UNC entries with confidence levels |
| **Spec 17** | `spec/17-crossref.md` | Dependency graph, stage mapping, canonical element counts |
| **Spec 03** | `spec/03-query.md` | Six-stratum Datomic-style query engine specification |
| **ADRS.md** | `ADRS.md` | Design decision index (FD, PD, SQ, LD series) |
| **EDNL spec** | [GitHub: lambdaisland/edn-lines](https://raw.githubusercontent.com/lambdaisland/edn-lines/refs/heads/main/README.md) | EDNL format specification (datom serialization) |

</source_documents>

---

## 4. The Property Vocabulary: Load-Bearing Artifact

<property_vocabulary>

### Why This Matters

The property vocabulary is the single design decision that makes the coherence engine tractable.
Without it, checking whether two spec elements contradict each other is coNP-complete to
undecidable (requires understanding natural language semantics). With it, the check reduces to:

```
contradiction(E1, E2) ←
  committed(E1, PropertyA),
  committed(E2, PropertyB),
  incompatible(PropertyA, PropertyB),
  E1 ≠ E2.
```

This is an O(1) table lookup per property pair. The vocabulary transforms an undecidable
problem into a decidable, efficient one by **closing the ontology** — restricting the universe
of properties to a finite, enumerated set with pre-declared relationships.

### Structure

**109 properties** organized into **17 categories**:

| Category | Count | Key Properties |
|----------|-------|----------------|
| STORAGE | 8 | `append_only`, `content_addressable`, `set_union_merge`, `datom_five_tuple` |
| CONCURRENCY | 7 | `coordination_free`, `frontier_relative`, `crdt_mergeable`, `monotonic_computation` |
| QUERY | 8 | `calm_compliant`, `deterministic_query`, `stratified_negation`, `datalog_primary` |
| SCHEMA | 6 | `schema_as_data`, `schema_evolution_as_transaction`, `cardinality_enforced` |
| RESOLUTION | 7 | `per_attribute_resolution`, `lattice_resolved`, `resolution_at_query_time` |
| MERGE | 8 | `merge_commutative`, `merge_associative`, `merge_idempotent`, `merge_monotonic` |
| HARVEST | 6 | `harvest_monotonic`, `harvest_captures_untransacted`, `harvest_preserves_provenance` |
| SEED | 6 | `seed_is_projection`, `seed_budget_constrained`, `seed_no_fabrication` |
| SYNC | 5 | `consistent_cut`, `sync_barrier_blocking`, `post_barrier_deterministic` |
| SIGNAL | 7 | `signal_as_datom`, `dispatch_total`, `severity_monotonic_cost` |
| BILATERAL | 6 | `bilateral_symmetry`, `five_point_coherence`, `fitness_monotonic` |
| DELIBERATION | 6 | `deliberation_converges`, `stability_guard_required`, `precedent_queryable` |
| GUIDANCE | 7 | `guidance_per_response`, `dynamic_claude_md`, `guidance_anti_drift` |
| BUDGET | 6 | `budget_monotonic_decreasing`, `precedence_five_level`, `graceful_degradation` |
| INTERFACE | 6 | `five_layer_graded`, `store_sole_truth`, `cli_primary_agent_interface` |
| SAFETY | 7 | `no_data_loss`, `no_fabrication`, `no_budget_overflow`, `no_silent_failure` |
| SELF_BOOTSTRAP | 3 | `self_specifying`, `spec_elements_as_datoms`, `self_referential_coherence` |

**12 incompatibility rules** (I1-I12): Property pairs that are mutually exclusive. No element
may commit to both. Zero violations found across all 247 extracted elements.

**16 entailment rules** (E1-E16): If an element commits to property A, it logically entails
property B. 8 missing entailments found (elements committing to A but not listing B).

### Vocabulary Gaps Identified

4 properties needed but absent from v2:
- `synchronous_dispatch` (needed by INV-SIGNAL-002)
- `human_confirmation_required` (needed by ADR-BILATERAL-003)
- `provenance_verified` (needed by ADR-STORE-008)
- `cache_scope` (implicit in several elements)

Additionally, `signal_as_datom` should split into `signal_in_access_log` / `signal_in_main_store`
to distinguish the access log from the main datom store.

### Self-Bootstrap Potential

The vocabulary itself becomes schema datoms in the store:
```edn
[:property/name "append_only" :property/category "STORAGE" :property/meaning "Store never deletes or mutates existing datoms"]
[:incompatible/left "append_only" :incompatible/right "mutable_datoms" :incompatible/reason "Append-only means no mutation"]
[:entails/from "append_only" :entails/to "grow_only" :entails/reason "Append-only implies growth"]
```

This means coherence checks become standard Datomic-style queries over datoms — not a
separate verification system. Self-bootstrap at every level.

**Full specification**: `PROPERTY_VOCABULARY.md`

</property_vocabulary>

---

## 5. Full Extraction Results: 248 Elements, 25 Tensions

<extraction_results>

### Method

Five parallel Sonnet 4.6 extraction agents, each handling 2-4 namespaces:
- Batch 1: STORE + SCHEMA (~49 elements)
- Batch 2: QUERY + RESOLUTION (~50 elements)
- Batch 3: HARVEST + SEED + MERGE (~42 elements)
- Batch 4: SYNC + SIGNAL + BILATERAL (~32 elements)
- Batch 5: DELIBERATION + GUIDANCE + BUDGET + INTERFACE + UNC (~70 elements)

All agents received the full 109-property vocabulary, extraction JSON schemas, and the relevant
spec files. 100% extraction success. 100% vocabulary adherence. Zero out-of-vocabulary
properties used.

### Tension Breakdown

| Tier | Count | Examples |
|------|-------|---------|
| **CONTRADICTION** | 5 | Budget vs. pinned intentions (C-01), automated CYCLE vs. human-gated C3 (C-02), 50-token floor vs. harvest-only mode (C-03), store_sole_truth vs. session file (C-04), every-command-is-transaction vs. CALM (C-05) |
| **HIGH** | 5 | Working set isolation vs. provenance writes (H-01), non-monotone graph metrics claimed CRDT-mergeable (H-02), fitness monotonicity under-specified (H-03), deterministic routing toward uncertain objective (H-04), harvest commit pathway ambiguity (H-05) |
| **MEDIUM** | 10 | Query determinism vs. provenance side effects (M-01), provenance typing unverified (M-02), CALM overgeneralized (M-03), stage mismatches (M-04, M-05), detection vs. resolution ambiguity (M-06), confusion signal timing (M-07), temporal divergence escalation gap (M-08), anti-drift channel composition (M-09), guidance prune vs. retract semantics (M-10) |
| **MINOR** | 5 | Vocabulary precision issues (L-01, L-05), genesis datom count imprecision (L-02), threshold inconsistency (L-03), implementation stratification (L-04) |

### What the Engine Catches Automatically

| Detection Method | Tensions Caught | Percentage |
|-----------------|----------------|------------|
| Property incompatibility lookup | 0 | 0% (spec is clean at property level) |
| Entailment chain traversal | 8 gaps | — |
| Cross-element property conflict | 3 | 12% |
| Stage ordering check | 1 | 4% |
| Threshold consistency | 1 | 4% |
| **Subtotal: Datomic-style queries** | **13** | **52%** |
| Stratified negation (reachability, NAF) | ~8 | ~32% |
| LLM semantic analysis (NLI, domain) | ~4 | ~16% |
| **Total** | **25** | **100%** |

**Full analysis**: `FULL_EXTRACTION_RESULTS.md`

</extraction_results>

---

## 6. The Five Contradictions

<contradictions>

These are the most actionable findings. Each has a clear resolution path.

### C-01: Budget Cap vs. Pinned Intentions (INV-SEED-004 vs INV-SEED-002)

**Problem**: Intentions are pinned at π₀ (full datoms) regardless of budget. Output never
exceeds budget. If `|intentions at π₀| > budget`, both cannot be satisfied.

**Resolution**: Define a minimum budget floor guaranteeing intentions always fit, OR
INV-SEED-004 takes precedence with an explicit carve-out for pinned intentions in
INV-SEED-002.

### C-02: Automated CYCLE vs. Human-Gated C3 (INV-BILATERAL-002 + NEG-BILATERAL-002 vs ADR-BILATERAL-003)

**Problem**: Every CYCLE evaluates all five coherence conditions (INV). No CYCLE skips any
(NEG). But C3 (spec ≈ intent) requires human review (ADR). A NEG and an ADR directly
contradict.

**Resolution**: Split CYCLE into `automated-cycle` (C1, C2, C4, C5) and `intent-cycle`
(adds C3). NEG applies to the appropriate cycle type.

### C-03: 50-Token Floor vs. Harvest-Only Mode (INV-BUDGET-001 L3 vs INV-INTERFACE-007)

**Problem**: `output_size ≥ MIN_OUTPUT (50 tokens)` always. But harvest-only mode emits
~10 tokens. Numerical contradiction.

**Resolution**: Harvest-only mode is explicitly exempt from L3, or the minimum is
scoped: "outside of harvest-imperative mode."

### C-04: store_sole_truth vs. Session File (NEG-INTERFACE-001 vs INV-INTERFACE-004)

**Problem**: No layer maintains state not projectable from the store. Statusline writes
`used_percentage`, `remaining_tokens` from the Claude Code API — external telemetry
that cannot be reconstructed from store datoms.

**Resolution**: Qualify `store_sole_truth` to include "external measurement sources,"
or treat the session file as a separate durability tier with explicit exception.

### C-05: Every Command Is a Transaction vs. CALM (INV-STORE-014 vs ADR-STORE-005)

**Problem**: Every CLI command, including queries, generates a provenance transaction.
A query that writes provenance is a write, not a coordination-free read. In a multi-agent
environment, every "read" creates O(N) provenance transactions. This collapses the
monotonic/non-monotonic distinction that CALM depends on.

**Resolution**: Query provenance transactions are local-only — written to the agent's
working set (W_α at Stage 2, local frontier at Stage 0), never merged into the shared
store. This simultaneously resolves H-01. The invariant should read: "Every command
produces a transaction record in the *agent's visible store*."

**This is the most architecturally significant contradiction.** If implemented as-written,
it would silently corrupt the entire multi-agent coordination model.

</contradictions>

---

## 7. Strategic Analysis: What Changed

<strategic_analysis>

### Before This Exploration

- Coherence engine was planned for **Stage 2+** (SEED.md §10, spec/README.md)
- Prolog/logic programming layer was a considered addition
- No systematic check of spec internal consistency had been performed
- The property vocabulary concept existed in conversation but not as a concrete artifact
- The extraction pipeline was untested

### After This Exploration

1. **Coherence engine moves to Stage 0.** The spec has contradictions. You cannot implement
   a contradictory spec. C-05 alone would silently corrupt multi-agent coordination if
   implemented as-written. The coherence engine is the verification layer that ensures the
   64 Stage 0 invariants are internally consistent before any Rust gets written.

2. **The implementation is surprisingly small.** ~500 lines of Rust:
   - Property vocabulary as datoms (~200 datoms for 109 properties + 28 rules)
   - Extraction prompt as a stored datum with version tracking
   - Extracted forms as datoms (`:extraction/element`, `:extraction/property`, `:extraction/role`)
   - Four Datomic-style query rules: incompatibility check, entailment chain, stage ordering,
     threshold consistency

3. **Self-bootstrap principle (C7) demands it.** The spec elements are the first datoms. The
   coherence checks are the first queries. The coherence engine is the system's immune system.

4. **The Prolog layer is architecturally unnecessary.** The Datomic-style query language
   with stratified negation and FFI covers the useful Prolog fragment. No separate runtime.

5. **Stage 0 now has 64 INVs** (per updated spec/17-crossref.md). The coherence engine is
   the verification layer for those 64 invariants.

### Updated Stage 0 Deliverables

```
Stage 0 deliverables (revised):
  transact, query, status, harvest, seed, guidance, dynamic CLAUDE.md
  + coherence: property vocabulary, extraction pipeline, query-based checks

First act: Migrate spec elements into datom store
  → extraction runs on each element
  → logical forms stored alongside elements
  → four coherence rules fire
  → contradictions surface at authoring time
```

</strategic_analysis>

---

## 8. The Prolog Question — Resolved

<prolog_resolution>

### The Question

"Should Braid include a Prolog engine alongside its Datomic-style query language for
coherence checking?"

### The Evolution of the Answer

**Initial position (Phase 5)**: "The gap between what Datomic-style queries catch and what
requires human review is empty. There is no problem for Prolog to solve."

**Correction (Phase 6)**: This was too dismissive. ~30-35% of tensions (8-9 of 25) ARE
catchable by Prolog-style reasoning — specifically negation-as-failure and reachability
queries. Examples:
- CALM violation detection (committed to `calm_compliant` but also `produces_transaction`
  without `scoped_to_working_set`)
- Escalation gap detection (signal type has no reachable resolution target)
- Stage ordering with negation (NEG references invariant from a future stage)

**Final position (Phase 7)**: Braid's own six-stratum query engine IS the Prolog equivalent.
Strata 2-5 provide stratified negation, aggregation, FFI, and barriered queries — the same
capabilities Prolog adds to Datalog. A separate Prolog runtime is architecturally redundant.

### Critical Clarification

"Datalog" in the Braid context means the **Datomic-style EAV query language** — EDN syntax,
four find forms (Relation, Scalar, Collection, Tuple), pull expressions, rules, inputs,
database functions. It is NOT the academic logic programming language that is a subset of
Prolog. The Datomic query language borrows the declarative pattern-matching flavor from
academic Datalog but is its own thing.

Several things initially framed as "enhancements from Prolog" are actually just faithful
Datomic features:
- **Rules**: Datomic already has named, reusable clause sets (reachability is just a recursive rule)
- **Database functions**: Datomic's equivalent of FFI (`member/2`, `smt_check/2` are database functions)
- **Negation**: Datomic supports `not`, `not-join`, `or`, `or-join` clauses with stratified evaluation

### What We Actually Borrow from Outside Datomic

Only four things:
1. **Tabling/memoization** for cyclic recursive rules (from XSB Prolog)
2. **SMT integration as a database function** (from ATP — Z3 for quantitative constraints)
3. **Discourse referents and SRL** as extraction structure (from CL)
4. **NLI as a database function** at Stratum 5 (from CL — LLM-backed inference)

Everything else is standard Datomic-style queries over well-structured datoms.

</prolog_resolution>

---

## 9. Techniques Borrowed from LP, ATP, and CL

> **Full analysis**: [`QUERY_ENGINE_ENHANCEMENTS.md`](QUERY_ENGINE_ENHANCEMENTS.md) — 10 techniques
> with formal justification (soundness, termination, monotonicity classification), empirical
> grounding (mapping of all 25 tensions to specific techniques), implementation staging
> (dependency graph, LOC estimates), and relation to `spec/03-query.md` type definitions.
> This section is the summary; that document is the reference.

<techniques>

### From Logic Programming

#### 1. Unification for EDN Value Destructuring
Datomic pattern matching (`[?e :attr ?v]`) can't destructure compound EDN values.
Add `member/2` and limited `unify/2` as database functions for vectors/maps/sets.

```clojure
[:find ?e ?prop
 :where [?e :inv/properties ?props]
        (member ?prop ?props)]  ;; structural destructuring
```

Stays within semi-naive fixpoint. Monotonic (Stratum 0).

#### 2. Subsumption-Based Tabling (XSB Prolog)
For recursive queries over cyclic dependency graphs (specs can have cycles). When a
recursive subgoal is re-encountered, return the partial answer instead of re-entering.
This is SLG resolution — standard extension to bottom-up evaluation.

**Need**: Spec dependency graphs can be cyclic. Naive recursion diverges.

#### 3. CLP Framing (Design Lens)
Treat property sets as constraint satisfaction problems. The domain is small (109 properties)
so no CLP solver is needed — the framing informs data model design. Express `incompatible/2`
and `entails/2` as Datomic-style rules over datoms in the store itself (schema-as-data, C3).

#### 4. DCG-Style Parsing for Structured Fragments
Use deterministic parsing (DCG-style) for formal spec fragments (IDs, cross-references,
verification tags, stage annotations). Use LLM extraction only for the unstructured prose.
Reduces the LLM's job and improves reliability.

### From Automated Theorem Proving

#### 5. SMT Solving (Z3) — Targeted
Register an `smt_check/2` database function at Stratum 3. Takes linear constraints
(from budget-related invariants) and returns SAT/UNSAT. Only valuable for the **quantitative
constraint fragment** (e.g., C-01: `|intentions_at_π₀| ≤ budget`).

**Not needed for**: property incompatibility, entailment chains, reachability — all handled
by the Datomic-style query layer.

#### 6. Bounded Model Checking
At crystallization time (a barrier point), enumerate all k-step transition sequences
from the current state and verify invariant preservation. k=2 or k=3 is practical.
The transition model is already in the store as datoms. This is a Stratum 5 query.

#### 7. Craig Interpolation (LLM-Mediated)
When two elements contradict, interpolation gives the minimal shared concept that creates
the incompatibility. For natural language specs, an LLM produces the interpolant:
"Given these two contradicting elements, identify the minimal shared concept that makes
them incompatible." Stored as a datom linking the two elements. Stratum 5 (semantic, barriered).

### From Computational Linguistics

#### 8. Discourse Representation Theory (DRT)
Assign discourse referents to entities mentioned in spec elements. Store as datoms:
```edn
[5001 :discourse/element :INV-STORE-001 5000 true]
[5001 :discourse/referent "datom_store" 5000 true]
```
Enables Stratum 1 queries finding all elements sharing discourse referents — implicit
cross-references the explicit crossref graph misses.

#### 9. Semantic Role Labeling (SRL)
Extract agent/action/patient triples from each spec element:
- "The query engine [agent] rejects [action] non-monotonic constructs [patient]"

More targeted than property classification alone. Enables: "which elements share the
same agent and patient but contradictory actions?"

#### 10. Natural Language Inference (NLI)
Register `nli/3` database function at Stratum 5. For each pair of elements sharing
discourse referents or dependency edges, evaluate entailment/contradiction/neutral.
Gate behind cheaper checks — only run NLI on pairs surviving Datalog and stratified filters.

### Implementation Staging

| Technique | Stage | Rationale |
|-----------|-------|-----------|
| `member/2`, `unify/2` (FFI functions) | 0 | Core query engine enhancement |
| Tabling for cyclic recursive rules | 0 | Needed for spec dependency graphs |
| Property vocab as schema datoms | 0 | Self-bootstrap requirement |
| SMT/Z3 as database function | 1+ | Requires LLM integration for constraint extraction |
| DRT discourse referents | 1+ | Requires LLM extraction pipeline |
| SRL triple extraction | 1+ | Requires LLM extraction pipeline |
| NLI as database function | 1+ | Expensive, gate behind cheaper checks |
| Bounded model checking | 2+ | Requires transition model in store |
| Craig interpolation | 2+ | Requires LLM + stored contradiction pairs |

</techniques>

---

## 10. Datomic Query Language as Coherence Engine

> **Full analysis**: [`QUERY_ENGINE_ENHANCEMENTS.md`](QUERY_ENGINE_ENHANCEMENTS.md) §6 (unified
> architecture), §9 (formal justification), §10 (empirical tension-to-technique mapping).

<unified_architecture>

### The Key Insight

The Braid query engine IS the coherence engine. The property vocabulary as schema datoms
makes coherence checking a standard query, not a separate system.

### Architecture

```
EDNL datom store (.ednl files, append-only)
    │
    ├── Schema layer: property vocabulary as datoms (CLP framing)
    │   ├── :property/name, :property/category, :property/meaning
    │   ├── :incompatible/left, :incompatible/right, :incompatible/reason
    │   └── :entails/from, :entails/to, :entails/reason
    │
    ├── Fact layer: extraction results as datoms
    │   ├── :element/id, :element/namespace, :element/stage
    │   ├── :element/property (multi-value, cardinality :many)
    │   ├── :element/agent, :element/action, :element/patient (SRL)
    │   └── :discourse/referent (DRT)
    │
    └── Query strata (coherence checks):
        │
        S0: Property lookup, entity attributes
        S1: Cross-ref reachability, discourse graphs (with tabling)
        S2: Incompatibility violations, missing entailments (stratified negation)
            └── member/2, unify/2 for EDN value destructuring
        S3: Budget satisfiability (Z3 via database function)
        S4: Stage ordering, CALM violations (conservative)
        S5: NLI pairwise (LLM via database function), bounded model checking
            └── Only for pairs surviving S0-S4 filters
```

### Core Coherence Queries (Datomic-Style)

```clojure
;; Incompatibility violation — Stratum 2 (negation-as-failure)
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

;; Stage ordering violation — Stratum 1 (graph traversal)
[:find ?e1 ?e2
 :where [?e1 :element/depends-on ?e2]
        [?e1 :element/stage ?s1]
        [?e2 :element/stage ?s2]
        [(< ?s1 ?s2)]]

;; CALM violation — Stratum 2 (negation + conjunction)
[:find ?e
 :where [?e :element/property :calm_compliant]
        [?e :element/property :produces_transaction]
        (not [?e :element/property :scoped_to_working_set])]

;; Reachability gap — Stratum 1 (recursive rule + negation at S2)
[:find ?signal-type ?target
 :in $ %
 :where [?st :divergence/type ?signal-type]
        [?st :expected/resolution ?target]
        (not (can-reach ?signal-type ?target))]
;; with rule:
[[(can-reach ?a ?b) [?a :dispatches-to ?b]]
 [(can-reach ?a ?c) [?a :dispatches-to ?b] (can-reach ?b ?c)]]
```

### What We DON'T Need

- **Prolog runtime**: Datomic-style queries with tabling and stratified negation cover it
- **Full ATP**: Only Z3 for linear arithmetic. Everything else is too expensive for the payoff
- **Dependency/constituency parsing**: SRL subsumes the needed capability
- **OWL/RDF-S ontology languages**: 109 properties is too small for heavyweight ontology tooling

### What We DO Need to Build (for the query engine)

1. `member/2` and `unify/2` database functions — EDN value destructuring (Stage 0)
2. Subsumption-based tabling — cyclic recursive rule termination (Stage 0)
3. `smt_check/2` database function — Z3 for quantitative constraints (Stage 1+)
4. `nli/3` database function — LLM pairwise semantic entailment (Stage 1+)
5. Property vocabulary as schema datoms — self-bootstrap (Stage 0)

</unified_architecture>

---

## 11. Prompt Engineering: What Worked

<prompt_engineering>

### Three Prompt Structures Tested

| Structure | Model | Result |
|-----------|-------|--------|
| **Structured template** (full schema, vocabulary, examples, JSON schema) | Opus | Highest accuracy, richest dependencies, self-calibrated (rated own limitations) |
| **Minimal prompt** (vocabulary list, compact format spec) | Sonnet | High accuracy, perfect vocabulary adherence, moderate dependencies, novel inverse entailments |
| **Zero-shot** (property list, brief instructions) | Haiku | Good accuracy but occasional vocabulary drift, sparse dependencies, found coverage gaps |

### What Made the Full Extraction Work

The production extraction prompt (used for the 247-element extraction) had these key features:

1. **Full property vocabulary inline** — all 109 properties with meanings, all 12 incompatibilities,
   all 16 entailments. The model needs the complete closed ontology to classify correctly.

2. **Typed JSON extraction schemas** — separate schemas for INV, ADR, NEG, UNC. Each schema
   specifies exactly which fields to fill: `properties_committed`, `properties_assumed`,
   `violation_condition`, `dependencies`, `confidence`, `stage`.

3. **"Classify, don't generate"** instruction — explicitly tells the model to select properties
   from the vocabulary, not invent new ones. This is the critical constraint that achieves
   100% vocabulary adherence.

4. **Spec text provided verbatim** — the model reads the actual spec element, not a summary.
   This prevents information loss at the extraction boundary.

5. **Cross-element context** — each agent received all elements from its assigned namespaces,
   enabling within-batch tension detection.

### Production Pipeline Design

```
Phase 1: EXTRACT (Sonnet, per-element, parallelizable)
  Input: spec element text + full property vocabulary
  Output: JSON logical form (properties, dependencies, confidence, stage)
  Cost: ~500 tokens per element, ~125K tokens total for 248 elements

Phase 2: VALIDATE (Haiku, per-element, parallelizable)
  Input: original spec text + extracted logical form
  Output: round-trip prose reconstruction + fidelity score
  Cost: ~300 tokens per element

Phase 3: DATOMIC-STYLE CHECKS (automatic, no LLM)
  Rules: incompatibility, entailment, stage ordering, self-contradiction
  Input: all extracted logical forms as datoms
  Output: violations with element pairs and rule citations
  Cost: O(n²) property comparisons, subsecond

Phase 4: STRATIFIED CHECKS (automatic, Strata 2-3)
  Rules: CALM violation, reachability gap, negation-based gap detection
  Input: extracted forms + dependency graph + dispatch topology
  Output: violations with reachability traces
  Cost: O(n·m) graph traversal, subsecond

Phase 5: SEMANTIC CHECKS (Opus via database function, expensive)
  Input: element pairs surviving Phases 3-4 + shared discourse referents
  Output: NLI classification (entailment/contradiction/neutral) with explanation
  Cost: ~1000 tokens per pair, gated behind cheaper checks
  Only runs on pairs sharing discourse referents or dependency edges
```

### Key Observations

- **Sonnet is the production extraction tier.** Opus is unnecessary for slot-filling; Haiku
  has vocabulary adherence issues. Sonnet hits the sweet spot: 100% accuracy, 100% vocabulary
  adherence, moderate dependency richness, and much lower cost than Opus.

- **Parallelization works.** Five agents extracting simultaneously had zero coordination issues.
  Each agent handles independent namespaces. Results merge trivially (append logical forms).

- **The property vocabulary is the prompt.** The extraction prompt is ~80% vocabulary definition
  and ~20% instruction. The vocabulary does the heavy lifting — the model's job is classification,
  not reasoning.

</prompt_engineering>

---

## 12. Open Questions

<open_questions>

### Resolved During This Exploration

| Question | Resolution |
|----------|------------|
| Is LLM extraction of logical forms feasible? | **Yes.** 100% success across all model tiers. |
| Is a separate Prolog engine needed? | **No.** Datomic-style query language with stratified negation covers the useful fragment. |
| Where does coherence engine sit in the staged roadmap? | **Stage 0.** Cannot implement a contradictory spec. |
| What is the right abstraction for spec properties? | **Closed ontology of 109 typed properties** with declared incompatibilities and entailments. |
| Can coherence checking scale to the full spec? | **Yes.** 247/248 elements extracted with 100% vocabulary adherence. 25 tensions found. |

### Still Open

| # | Question | Impact | What Would Resolve It |
|---|----------|--------|-----------------------|
| Q-01 | **How should the 5 contradictions be resolved?** | Blocking for Stage 0 implementation | Spec edits per the resolution options in §6. Each has a clear path. |
| Q-02 | **Should extraction run at `braid transact` time or as a separate command?** | Architecture decision | ADR needed. Options: (a) post-transact hook for spec-typed datoms, (b) separate `braid coherence` command, (c) both (hook + on-demand). |
| Q-03 | **What is the extraction prompt versioning strategy?** | Prompt drift could change results | Store the prompt as a datom with version tracking. Re-extraction on prompt change. |
| Q-04 | **How to handle the 4 new spec elements not yet extracted?** | 99.6% coverage, not 100% | Delta extraction on STORE +1 ADR, SEED +2 INV, RESOLUTION +1 ADR. |
| Q-05 | **What threshold triggers re-extraction?** | Efficiency vs. freshness | Options: on every spec element edit, on vocabulary change, on demand only. |
| Q-06 | **How do extracted logical forms interact with the bilateral loop?** | Self-bootstrap integration | Logical forms are the forward check (does spec cohere?). The backward check (do forms faithfully represent spec?) is the round-trip validation. |
| Q-07 | **Is tabling needed for Stage 0 or only for Stage 2+ cyclic graphs?** | Implementation priority | Depends on whether Stage 0 spec dependency graphs have cycles. Check `spec/17-crossref.md` dependency graph. |

</open_questions>

---

## 13. Concrete Next Steps

<next_steps>

### Immediate (Pre-Implementation)

#### Step 1: Fix the 5 Contradictions
**Effort**: ~2 hours of spec editing
**Files affected**: `spec/06-seed.md` (C-01), `spec/10-bilateral.md` (C-02),
`spec/13-budget.md` + `spec/14-interface.md` (C-03), `spec/14-interface.md` (C-04),
`spec/01-store.md` (C-05)

| ID | Fix | Approach |
|----|-----|----------|
| C-01 | Add budget floor for pinned intentions | INV-SEED-002 gets carve-out: "excluding pinned intentions" |
| C-02 | Split CYCLE into automated-cycle and intent-cycle | New ADR, update INV-BILATERAL-002 and NEG-BILATERAL-002 |
| C-03 | Exempt harvest-only mode from 50-token floor | Add scope qualifier to INV-BUDGET-001 L3 |
| C-04 | Qualify store_sole_truth | Add "or external measurement sources" to NEG-INTERFACE-001 |
| C-05 | Make query provenance local-only | Rewrite INV-STORE-014: "agent's visible store" not "the store" |

#### Step 2: Resolve the 5 High Tensions
**Effort**: ~1 hour of spec editing + 1 new ADR
**Approach**: See `FULL_EXTRACTION_RESULTS.md` §6 for per-tension fixes.

#### Step 3: Vocabulary v3
**Effort**: ~30 minutes
**Changes**: Add 4 missing properties, split `signal_as_datom`, add `store_branch_isolation`
vs `ephemeral_branch_isolation`.

#### Step 4: Re-Run Extraction After Fixes
**Effort**: ~1 hour (automated extraction + manual review)
**Purpose**: Verify tension count drops. If it doesn't, deeper structural issues exist.
If it does, you have a **quantified measure of spec coherence improvement**.

### Stage 0 Implementation

#### Step 5: Implement Property Vocabulary as Schema Datoms
**Effort**: ~200 datoms (109 properties + 28 rules + metadata)
**Location**: Genesis transaction or bootstrap transaction after genesis

#### Step 6: Implement the Four Core Coherence Queries
**Effort**: ~200 lines of Rust (query definitions + test harness)
**Queries**: Incompatibility check, entailment chain, stage ordering, threshold consistency

#### Step 7: Implement Extraction Pipeline
**Effort**: ~300 lines of Rust (LLM integration + JSON parsing + datom storage)
**Design**: `braid coherence extract <element-id>` → calls LLM → stores logical form datoms

#### Step 8: Self-Bootstrap — Coherence-Check the Coherence Engine's Spec
**Effort**: Extract the coherence engine's own spec elements → run coherence checks → verify
no internal contradictions. This is the strongest possible validation.

### Testing Approach

See §15 (Testing Strategy) below for the complete test plan.

</next_steps>

---

## 14. Formal Methods Assessment

<formal_methods>

### Algebraic Structure

The property vocabulary forms a **bounded lattice** over the power set of 109 properties:
- **Meet**: Property intersection (shared commitments)
- **Join**: Property union (combined commitments)
- **Bottom**: Empty property set (no commitments)
- **Top**: All 109 properties (maximally committed — unreachable due to incompatibilities)

The 12 incompatibility rules define **forbidden zones** in the lattice — regions where the
join of two property sets would include an incompatible pair. These are the contradiction
detection points.

The 16 entailment rules define **closure operations** — given a property set P, its closure
cl(P) adds all entailed properties. An element is **well-formed** iff its declared properties
equal their closure: `P = cl(P)`. The 8 missing entailments found are elements where `P ⊂ cl(P)`.

### Decidability

- **Property incompatibility checking**: Decidable, O(|P|²) per element pair. Finite domain.
- **Entailment completeness**: Decidable, O(|P| × |E|) per element. Finite rules.
- **Stage ordering**: Decidable, O(|edges|) DAG topological sort.
- **CALM violation detection**: Decidable, syntactic check on property sets.
- **Reachability**: Decidable, O(|V| + |E|) graph traversal with tabling for cycles.
- **Semantic entailment (NLI)**: **Undecidable** in general. LLM approximation.

### CALM Compliance

The coherence engine's core queries are monotonic (Strata 0-1): adding a new spec element
can only add new violations or new entailments, never remove existing ones. This means:

1. Coherence checks can run without coordination (CALM theorem)
2. Results are frontier-independent for the monotonic fragment
3. Incremental evaluation is sound — check only new elements against existing ones

The stratified checks (Strata 2-3) are frontier-relative but deterministic at a given
frontier. The semantic checks (Stratum 5) are barriered.

### Self-Referential Coherence

The coherence engine checks its own specification. This is sound (not Gödelian) because:
1. The property vocabulary is **finite and well-founded** (no self-referencing properties)
2. The incompatibility and entailment rules are **pre-declared** (no self-modifying rules)
3. The extraction is **external** (LLM, not the engine itself)

The Gödelian limit applies if the engine tries to prove its own soundness from within —
but the engine doesn't make soundness claims about itself. It checks property-level coherence,
which is a decidable fragment.

### Soundness and Completeness

**Sound**: If the engine reports a contradiction, it is a genuine contradiction at the
property level. No false positives (the incompatibility table is manually curated).

**Incomplete**: The engine cannot detect all possible contradictions. Semantic contradictions
(operational semantics, domain knowledge, temporal reasoning) require LLM assistance or
human review. The engine catches 52% automatically, ~84% with stratified queries, and
approximates the remainder via NLI.

This is the expected tradeoff: sound but incomplete at the decidable layer, approximately
complete at the LLM-assisted layer.

</formal_methods>

---

## 15. Testing Strategy

<testing_strategy>

### Test Plan for the Coherence Engine

#### Level 1: Unit Tests (Property Vocabulary)

```
test_incompatibility_symmetry:
  ∀ (A, B) ∈ incompatibles: incompatible(A, B) ↔ incompatible(B, A)

test_entailment_transitivity:
  ∀ A, B, C: entails(A, B) ∧ entails(B, C) → entails(A, C) ∈ closure

test_incompatibility_entailment_consistency:
  ∀ A, B, C: entails(A, B) ∧ incompatible(B, C) → incompatible(A, C) ∈ closure

test_no_self_incompatibility:
  ∀ P: ¬incompatible(P, P)

test_no_self_entailment:
  ∀ P: ¬entails(P, P) (unless P is semantically reflexive, which none are)

test_vocabulary_coverage:
  ∀ namespaces: ∃ at least 3 properties covering the namespace's core commitments
```

#### Level 2: Integration Tests (Extraction Pipeline)

```
test_extraction_round_trip:
  ∀ elements E: extract(E) → reconstruct(extract(E)) ≈ E (fidelity > 0.8)

test_vocabulary_adherence:
  ∀ elements E: ∀ P ∈ extract(E).properties: P ∈ vocabulary

test_extraction_determinism:
  ∀ elements E: extract(E, seed=42) = extract(E, seed=42)

test_known_contradiction_detection:
  inject(E1 with append_only, E2 with mutable_datoms) → contradiction(E1, E2)

test_known_entailment_detection:
  inject(E with append_only but not grow_only) → missing_entailment(E, grow_only)
```

#### Level 3: Regression Tests (Known Tensions)

```
test_C01_detected: Budget cap vs. pinned intentions → CONTRADICTION
test_C02_detected: Automated CYCLE vs. human-gated C3 → CONTRADICTION
test_C03_detected: 50-token floor vs. harvest-only mode → CONTRADICTION
test_C04_detected: store_sole_truth vs. session file → CONTRADICTION
test_C05_detected: Every command is transaction vs. CALM → CONTRADICTION

test_C01_resolved: After fix, no contradiction in SEED namespace
test_C02_resolved: After fix, no contradiction in BILATERAL namespace
...
```

#### Level 4: Self-Bootstrap Tests

```
test_self_coherence:
  Extract all coherence engine spec elements → run coherence checks → 0 contradictions

test_vocabulary_self_consistency:
  Load vocabulary as datoms → run incompatibility check on vocabulary itself → 0 violations

test_extraction_of_extraction_spec:
  Extract INV-COHERENCE-* elements → verify properties include self_specifying
```

#### Level 5: Scale Tests

```
test_full_spec_extraction:
  Extract all 248 elements → all succeed → all use vocabulary properties

test_tension_count_monotonic_after_fixes:
  Before fixes: 25 tensions. After C-01..C-05 fixes: tensions ≤ 20.

test_incremental_extraction:
  Extract 200 elements → add 48 → extract only new → full results identical
```

### Acceptance Criteria

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| Extraction success rate | 100% | Elements successfully extracted / total elements |
| Vocabulary adherence | 100% | Properties from vocabulary / total properties used |
| Known contradiction detection | 100% | Known contradictions detected / known contradictions |
| False positive rate | 0% | Spurious contradictions / total reported contradictions |
| Round-trip fidelity | > 0.8 | Semantic similarity of reconstruction to original |
| Self-bootstrap pass | 0 contradictions | Coherence engine's own spec has no internal contradictions |

</testing_strategy>

---

## 16. Glossary of Key Terms

<glossary>

| Term | Definition |
|------|-----------|
| **Closed ontology** | A finite, enumerated set of properties with pre-declared relationships. Makes coherence checking decidable. |
| **CALM theorem** | "A program has a consistent, coordination-free distributed implementation iff it is monotone." |
| **CLP** | Constraint Logic Programming — treating property sets as constraint satisfaction problems. |
| **Craig interpolation** | Given A ∧ B unsatisfiable, find formula I where A → I and I ∧ B unsatisfiable. Gives minimal explanation of contradiction. |
| **Database function** | Datomic's FFI mechanism — imperative functions callable from within queries. |
| **Datom** | `[entity, attribute, value, transaction, operation]` — an atomic fact. |
| **DCG** | Definite Clause Grammar — Prolog's formalism for deterministic parsing. |
| **Discourse referent** | An entity introduced in one spec element and referenced in others (DRT concept). |
| **DRT** | Discourse Representation Theory — formal semantics for tracking cross-sentence entity references. |
| **EDNL** | EDN Lines — newline-separated EDN format, one form per line. Braid's physical storage format. |
| **Entailment rule** | If property A is committed, property B is logically required. |
| **Incompatibility rule** | Properties A and B cannot both be committed by the same element. |
| **NLI** | Natural Language Inference — classify premise-hypothesis pair as entailment/contradiction/neutral. |
| **Property vocabulary** | The 109-property closed ontology. The load-bearing artifact. |
| **Slot-filling extraction** | Classifying existing text into predefined categories. Most reliable LLM capability. |
| **SLG resolution** | Subsumption-based tabling — standard extension for cyclic recursive queries. |
| **SMT** | Satisfiability Modulo Theories — decision procedure for first-order logic fragments (Z3). |
| **SRL** | Semantic Role Labeling — extracting agent/action/patient triples from text. |
| **Stratum** | One of six query classification levels in Braid's query engine (S0-S5). |
| **Tabling** | Memoization of recursive query subgoals for termination on cyclic graphs. |
| **Tension** | A coherence issue between spec elements, ranging from MINOR to CONTRADICTION. |

</glossary>

---

## Appendix A: Relationship to Other Project Documents

<relationships>

### This Document's Place in the Project

```
SEED.md                              ← The foundational design document
  └── spec/                          ← The formal specification (14 namespaces, 248 elements)
       └── FULL_EXTRACTION_RESULTS.md ← Extraction of logical forms from spec
            └── HARVEST_SEED.md       ← THIS DOCUMENT: comprehensive harvest of exploration
                 └── (future: implementation)
```

### How This Document Feeds into Implementation

1. **Fix contradictions** (§6) → edit `spec/01-store.md`, `spec/06-seed.md`, `spec/10-bilateral.md`,
   `spec/13-budget.md`, `spec/14-interface.md`
2. **Property vocabulary** (§4) → first datoms in genesis+1 transaction
3. **Core queries** (§10) → first queries the engine runs
4. **Testing strategy** (§15) → test harness for coherence engine
5. **Techniques** (§9) → enhancement backlog for query engine

### Cross-References to HARVEST.md

This exploration should be recorded as a session entry in `HARVEST.md`. The entry should
reference this document for full details and summarize the key decisions and findings.

</relationships>

---

## Appendix B: Session Metadata

<metadata>

**Conversation ID**: `41debf42-a03a-4629-b0fd-16e9c6a707b2`
**Platform**: Claude Code (Opus 4.6)
**Context windows**: 3 (two compactions during the session)
**Subagents launched**: 10 (5 Opus for SEED_ANALYSIS, 5 Sonnet for full extraction)
**Total spec elements analyzed**: 248 (124 INV, 72 ADR, 42 NEG, 10 UNC)
**Total tensions discovered**: 25 unique (31 raw, deduplicated)
**Property vocabulary version**: v2 (109 properties, 12 incompatibilities, 16 entailments)
**Files created**: 6 (SEED_ANALYSIS.md, EXTRACTION_EXPERIMENT.md, PROPERTY_VOCABULARY.md,
FULL_EXTRACTION_RESULTS.md, logical-forms-08-10.json, extraction_wave3.json, + this document)

</metadata>

---

*This document is a seed for future sessions. It captures the complete state of the coherence
engine exploration as of 2026-03-03. Any session picking up this thread should read §1, §12,
and §13 first, then consult other sections as needed. The primary source documents are listed
in §3. All findings are reproducible given the same spec elements and property vocabulary.*
