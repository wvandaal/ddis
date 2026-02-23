---
module: search-intelligence
domain: search
maintains: [APP-INV-004, APP-INV-005, APP-INV-008, APP-INV-012, APP-INV-014]
interfaces: [APP-INV-001, APP-INV-003, APP-INV-009, APP-INV-015, APP-INV-016]
implements: [APP-ADR-003, APP-ADR-006]
adjacent: [parse-pipeline, query-validation, lifecycle-ops]
negative_specs:
  - "Must NOT use online learning or mutable global state in search indices"
  - "Must NOT return results without a computable score derivation"
  - "Must NOT build context bundles that reference elements outside the spec index"
---

# Search Intelligence Module

This module specifies the hybrid search engine and context intelligence subsystem of the DDIS CLI. The search pipeline combines three independent relevance signals — BM25 lexical matching (via SQLite FTS5), Latent Semantic Indexing (via truncated SVD), and PageRank authority scoring (over the cross-reference graph) — into a single ranked result set using Reciprocal Rank Fusion (RRF). The context intelligence subsystem composes nine structured signals into self-contained bundles that serve as pre-flight briefings for LLM editors operating on spec elements.

The architectural principle: search and context are **read-only, deterministic projections** of the spec index. They never mutate the index, never introduce non-determinism, and never reference elements outside the parsed corpus. Every score is derivable from first principles given the input data.

**Invariants interfaced from other modules (INV-018 compliance — restated at point of use):**

- APP-INV-001: Round-Trip Fidelity — parse then render produces byte-identical output (maintained by parse-pipeline). *Search depends on parsed content being faithful to the source document; any parse corruption propagates silently into search rankings.*
- APP-INV-003: Cross-Reference Integrity — every resolved reference points to an existing element (maintained by query-validation). *Authority scoring depends on a correct cross-reference graph; dangling references produce phantom nodes in PageRank.*
- APP-INV-009: Monolith-Modular Equivalence — parsing a monolith produces the same index as parsing assembled modules (maintained by parse-pipeline). *Search results must be identical regardless of whether the source was monolith or modular.*
- APP-INV-015: Deterministic Hashing — SHA-256 with no salt produces identical hash for identical input (maintained by parse-pipeline). *Document identity in the search index relies on stable hashing.*
- APP-INV-016: Implementation Traceability — every invariant with implementation claims has valid Source/Tests/Validates-via paths (maintained by lifecycle-ops). *This module's own invariants carry Implementation Trace annotations that must resolve.*

---

## Invariants

This module maintains five invariants. Each invariant is fully specified with all six components: plain-language statement, semi-formal expression, violation scenario, validation method, WHY THIS MATTERS annotation, and implementation trace.

---

**APP-INV-004: Authority Monotonicity**

*Adding a relevant cross-reference to an element can only increase (or maintain) that element's authority score; it can never decrease it.*

```
∀ element e, ∀ cross-reference x where x.target = e:
  authority(graph ∪ {x → e}) ≥ authority(graph, e)

where authority(G, e) = PageRank(e) in directed graph G
  with damping factor d = 0.85
  and convergence threshold ε = 1e-6
```

Violation scenario: A specification author adds a new cross-reference from §3.2 to INV-006. After re-indexing, INV-006's authority score drops from 0.042 to 0.038 because the PageRank implementation incorrectly re-normalizes all scores by dividing by the new node count without accounting for the additional inbound edge. The author removes the cross-reference, thinking it "hurts" the element's ranking, when in fact the implementation has a normalization bug.

Validation: Construct a test graph of 10 nodes with known PageRank scores. Add one inbound edge to a target node. Recompute PageRank. Assert that the target node's score is greater than or equal to its previous score. Repeat for 5 different graph topologies including: (a) a star graph, (b) a chain, (c) a graph with a cycle, (d) a disconnected component, (e) a dense clique. The property holds because each inbound edge contributes a non-negative term `d * PR(source) / out_degree(source)` to the target's score in the PageRank iteration, and no existing inbound contributions are reduced by adding a new edge.

// WHY THIS MATTERS: Authority scores guide search ranking. If adding a legitimate cross-reference could *decrease* an element's authority, authors would be incentivized to minimize cross-references — exactly the opposite of what a well-connected specification needs (see INV-006 in the DDIS standard).

**Confidence:** property-derived (from PageRank theory)

**Implementation Trace:**
- Source: `internal/search/authority.go::ComputeAuthority`
- Tests: `tests/search_test.go::TestSearchAuthorityComputed`
- Validates-via: Property test — add edge, assert score non-decreasing

----

**APP-INV-005: Context Self-Containment**

*Every context bundle produced by `BuildContext` includes all nine intelligence signals. No signal may be structurally absent from the bundle; signals with no data emit their zero-value (empty list, nil) rather than being omitted from the structure.*

```
∀ bundle ∈ ContextBundles:
  bundle.has(target_content)        ∧   // signal 1: resolved fragment
  bundle.has(constraints)           ∧   // signal 2: invariants, gates, negspecs
  bundle.has(invariant_completeness)∧   // signal 3: 5-component check
  bundle.has(coverage_gaps)         ∧   // signal 4: missing requirements
  bundle.has(local_validation)      ∧   // signal 5: scoped checks
  bundle.has(reasoning_modes)       ∧   // signal 6: Formal/Causal/Practical/Meta tags
  bundle.has(related_elements)      ∧   // signal 7: LSI cosine similarity
  bundle.has(impact_radius)         ∧   // signal 8: forward+backward dependencies
  bundle.has(editing_guidance)          // signal 9: synthesized guidance + recent changes

where "has" means the field exists in the struct (may be empty/nil, must not be absent)
```

Violation scenario: A developer refactors `BuildContext` and accidentally moves the `tagReasoningModes` call inside a conditional block that only executes for section-type targets. When an LLM requests context for INV-006 (an invariant), the bundle arrives with `reasoning_mode_related: null`. The LLM, lacking reasoning-mode tags, fails to distinguish formal constraints from causal rationale and produces an edit that satisfies the invariant's letter but violates the ADR that motivated it.

Validation: For each of the six element types (section, invariant, adr, gate, glossary, negative_spec), build a context bundle and verify that the JSON serialization contains all nine top-level keys. Fields may be empty arrays or null, but must not be absent from the serialized output. Verify with `jq 'keys'` that the output contains exactly: `constraints`, `content`, `coverage_gaps`, `editing_guidance`, `element_type`, `impact_radius`, `invariant_completeness`, `line_end`, `line_start`, `local_validation`, `reasoning_mode_related`, `related`, `recent_changes`, `target`, `title`.

// WHY THIS MATTERS: Context bundles are the primary interface between the search engine and LLM editors. A missing signal is invisible — the LLM does not know what it does not know. Self-containment ensures every bundle is a complete briefing, not a partial one.

**Confidence:** structurally-verified

**Implementation Trace:**
- Source: `internal/search/context.go::BuildContext`
- Source: `internal/search/context.go::findConstraints`
- Source: `internal/search/context.go::checkInvariantCompleteness`
- Source: `internal/search/context.go::findCoverageGaps`
- Source: `internal/search/context.go::runLocalValidation`
- Source: `internal/search/context.go::tagReasoningModes`
- Source: `internal/search/context.go::generateGuidance`
- Tests: `tests/search_test.go::TestContextBundleSection`
- Tests: `tests/search_test.go::TestContextBundleInvariant`
- Tests: `tests/search_test.go::TestContextBundleJSON`
- Tests: `tests/search_test.go::TestContextEditingGuidance`
- Tests: `tests/search_test.go::TestContextCoverageGaps`
- Tests: `tests/search_test.go::TestContextInvariantCompleteness`
- Tests: `tests/search_test.go::TestContextReasoningModeTags`
- Validates-via: `internal/validator/checks.go::checkXRefIntegrity`

----

**APP-INV-008: RRF Fusion Correctness**

*The Reciprocal Rank Fusion score for every document equals the correctly computed weighted sum of 1/(K + rank_r(d)) x weight_r across all ranking signals r, followed by a type-boost multiplier.*

```
∀ doc ∈ SearchResults, ∀ signals ∈ {BM25, LSI, PageRank}:
  raw_score(doc) = Σ_r (weight_r / (K + rank_r(doc)))
  where K = 60 (APP-ADR-003)
  ∧ rank_r(doc) ∈ [1, |corpus|] (1-indexed)
  ∧ weight_r ∈ {bm25: 1.0, lsi: 1.0, authority: 0.5}
  ∧ rank_r(doc) = 0 means signal r did not rank doc (term excluded from sum)

  score(doc) = raw_score(doc) × type_boost(doc.element_type)
  where type_boost ∈ {invariant: 1.2, adr: 1.1, gate: 1.1,
                       section: 1.0, negative_spec: 0.9, glossary: 0.8}
```

Violation scenario: A document ranked #1 by BM25 and #3 by LSI receives a fusion score computed with 0-indexed ranks (using rank 0 and 2 instead of 1 and 3), producing `raw_score = 1.0/60 + 1.0/62 = 0.03280` instead of the correct `raw_score = 1.0/61 + 1.0/63 = 0.03227`. The document may be incorrectly ranked above a document with truly higher combined relevance. The error is small per-document but compounds across the result set, potentially reordering the top-5 results that the user actually reads.

Validation: For a known corpus of N documents with predetermined BM25, LSI, and PageRank orderings, compute RRF scores independently (hand-calculated reference) and compare against CLI output. All scores must match to float64 precision. Test with edge cases: (a) single-signal results (document appears in BM25 only), (b) tied ranks, (c) K boundary behavior (rank = 1 produces 1/(K+1)), (d) all three signals present, (e) type-boost interaction. See the worked example in the RRF Fusion chapter below.

// WHY THIS MATTERS: RRF is the fusion point where three independent relevance signals merge into a single ranking. An off-by-one in rank indexing or a weight miscalculation silently degrades every search result. Because the differences are small (fractions of a percent), the bug is invisible to casual testing but systematically biases rankings.

**Confidence:** property-checked

**Implementation Trace:**
- Source: `internal/search/engine.go::Search` (lines 218–257: RRF computation loop)
- Tests: `tests/search_test.go::TestSearchRRFFusion`
- Tests: `tests/search_test.go::TestSearchExactMatch`
- Tests: `tests/search_test.go::TestRRFFormulaCorrectness`
- Tests: `tests/search_test.go::TestRRFRankIndexing`
- Tests: `tests/search_test.go::TestRRFTypeBoosts`
- Validates-via: `internal/validator/checks.go::checkXRefIntegrity`

----

**APP-INV-012: LSI Dimension Bound**

*The LSI truncation dimension k never exceeds the document count, and every document vector and query vector has exactly k dimensions.*

```
∀ LSIIndex built from corpus of n documents with v vocabulary terms:
  k ≤ min(n, v)
  ∧ k ≤ 50 (configured ceiling)
  ∧ ∀ vec ∈ LSIIndex.DocVectors: len(vec) = k
  ∧ ∀ qvec returned by QueryVec: len(qvec) = k
```

Violation scenario: A small specification contains only 8 parsed elements (sections + invariants + ADRs). The LSI builder is called with k=50 (the default ceiling). Without the dimension bound, SVD attempts to extract 50 singular values from an 8-column matrix, producing a runtime panic in the `gonum/mat` SVD factorization. With the bound, k is clamped to min(50, 8) = 8, and all vectors are 8-dimensional.

Validation: (1) Build LSI from a corpus of 5 documents with k=50. Assert that the resulting `LSIIndex.K` equals 5 (or less, if vocabulary is smaller). Assert `len(DocVectors[i]) == K` for all i. (2) Build LSI from a corpus of 200 documents with k=50. Assert K=50 (ceiling applies). (3) Build LSI from a corpus of 3 documents with 2 unique terms. Assert K = min(3, 2) = 2. (4) Call `QueryVec` on each index and assert `len(result) == K`.

// WHY THIS MATTERS: SVD truncation to k dimensions is the mathematical core of LSI. If k exceeds the matrix rank, the factorization either fails (runtime panic) or produces garbage vectors filled with floating-point noise. The dimension bound is a hard safety constraint, not a performance optimization.

**Confidence:** property-checked

**Implementation Trace:**
- Source: `internal/search/lsi.go::BuildLSI` (lines 72–82: k-clamping logic)
- Tests: `tests/search_test.go::TestSearchLSIBuild`
- Tests: `tests/search_test.go::TestLSIDimensionBound`
- Tests: `tests/search_test.go::TestLSIDimensionStability`
- Validates-via: Assertion `len(DocVectors[i]) == K` in test

----

**APP-INV-014: Glossary Expansion Bound**

*Query expansion via glossary terms adds at most 5 additional terms to the original query. The original query terms are never removed or modified.*

```
∀ query q, ∀ expansions returned by expandQuery(q):
  |expandQuery(q)| ≤ 5
  ∧ fullQuery = q + " " + join(expandQuery(q))
  ∧ q ⊂ fullQuery  (original terms preserved verbatim)
```

Violation scenario: A glossary contains a term "cross-reference" whose definition is a 50-word paragraph. Without the expansion bound, `expandQuery` extracts every significant word (length > 4) from the definition, adding 30+ terms to the query. The expanded query overwhelms BM25 scoring — the added terms dilute the original query's signal, and a section about "reference architectures" outranks the section about actual cross-references because it matches more of the expansion terms.

Validation: (1) Seed a glossary with a term whose definition contains 20+ significant words. Call `expandQuery` with that term. Assert `len(result) <= 5`. (2) Call `expandQuery` with a query matching no glossary terms. Assert `len(result) == 0`. (3) Call `expandQuery` with a query matching multiple glossary terms. Assert `len(result) <= 5` (global cap, not per-term). (4) Verify that the returned terms do not include the original query words (no duplication).

// WHY THIS MATTERS: Query expansion is a double-edged sword. Too many expansion terms dilute the original query signal and produce results that match the expansion but not the user's intent. The bound of 5 keeps expansion helpful without hijacking the search.

**Confidence:** structurally-verified

**Implementation Trace:**
- Source: `internal/search/engine.go::expandQuery` (lines 273–315: expansion loop with cap at 5)
- Tests: `tests/search_test.go::TestSearchGlossaryExpansion`
- Validates-via: Test assertion `len(expansions) <= 5`

----

## Architecture Decision Records

---

### APP-ADR-003: BM25 + LSI + PageRank with RRF Fusion (K=60)

#### Problem

How should the CLI rank search results when multiple relevance signals are available? A single signal (e.g., BM25 alone) misses semantic similarity (synonyms, related concepts) and structural importance (heavily-referenced elements).

#### Options

A) **BM25 only** — SQLite FTS5 provides BM25 scoring out of the box.
- Pros: Zero additional dependencies; fast; well-understood. FTS5's BM25 handles term frequency and document length normalization.
- Cons: Purely lexical — "falsifiable invariant" does not match "testable constraint." No structural awareness — a heavily-referenced invariant ranks the same as an orphan section if they contain the same terms.

B) **BM25 + neural reranking** — Use BM25 as a first pass, then rerank top-N results with an embedding model.
- Pros: Captures semantic similarity. State-of-the-art retrieval quality.
- Cons: Requires an embedding model (either local or API). Non-deterministic across model versions (violates APP-INV-002: Validation Determinism, maintained by query-validation). Latency penalty. Dependency on external service or large binary.

C) **Hybrid RRF fusion (BM25 + LSI + PageRank)** — Three independent offline-computable signals fused via Reciprocal Rank Fusion.
- Pros: All signals are deterministic and offline-computable. LSI captures semantic similarity without a neural model. PageRank captures structural importance from the cross-reference graph. RRF is a proven fusion method requiring no training data. Each signal is independently auditable.
- Cons: LSI quality depends on corpus size (degrades below ~20 documents). RRF weights are heuristic, not learned. PageRank requires a well-connected cross-reference graph.

D) **Learned-to-rank** — Train a ranking model on query-relevance judgments.
- Pros: Optimal ranking for the specific domain.
- Cons: No training data exists for specification search. Model training introduces non-determinism. Requires ongoing maintenance. Violates APP-INV-002.

#### Decision

**Option C: Hybrid RRF fusion with K=60.** Three signals are computed offline at index time (BM25 via FTS5, LSI via truncated SVD, PageRank via iterative computation) and fused at query time using RRF with K=60 (the standard RRF constant from Cormack, Clarke & Butt, 2009).

Signal weights: `{bm25: 1.0, lsi: 1.0, authority: 0.5}`. Authority is weighted lower because it measures structural importance (how referenced), not query relevance (how related). A post-fusion type-boost multiplier adjusts scores by element type: `{invariant: 1.2, adr: 1.1, gate: 1.1, section: 1.0, negative_spec: 0.9, glossary: 0.8}`.

// WHY NOT Option B (neural reranking)? Non-deterministic across model versions. The CLI must produce identical rankings for identical input (APP-INV-002). Introducing a model dependency trades reproducibility for marginal quality gains on small corpora where LSI already captures the most important semantic relationships.

// WHY NOT Option D (learned-to-rank)? No training data, non-deterministic, and maintenance burden. The specification search domain is narrow enough that hand-tuned weights plus three complementary signals achieve sufficient quality.

#### Consequences

- All ranking is offline-deterministic: given the same spec index and query, results are identical.
- No model training, no API keys, no network dependencies.
- LSI degrades gracefully on small corpora (k is clamped to document count, APP-INV-012).
- Authority signal is only useful when the spec has a well-connected cross-reference graph; for specs with few cross-references, BM25 and LSI dominate the ranking.
- The K=60 constant and signal weights are tunable per APP-ADR-003 without changing the fusion algorithm.

#### Tests

- (Validated by APP-INV-008) RRF scores match hand-computed reference values.
- (Validated by APP-INV-012) LSI dimension bound prevents SVD failure on small corpora.
- `tests/search_test.go::TestSearchRRFFusion` verifies score monotonicity and multi-signal presence.
- `tests/search_test.go::TestSearchLexicalOnly` verifies that `--lexical-only` mode excludes LSI signal.

---

### APP-ADR-006: Context Bundles as Compound Intelligence (9 Signals)

#### Problem

How should the CLI present specification intelligence to LLM editors? The search engine finds *relevant* elements; the context system must present *actionable* intelligence about a single target element — enough for an LLM to make correct edits without reading the entire spec.

#### Options

A) **Raw search results** — Return the top-N search matches for the target element.
- Pros: Simple; reuses existing search infrastructure.
- Cons: Search results are ranked by relevance to a *query*, not by usefulness for *editing*. An LLM editing INV-006 needs to know what constraints it, what depends on it, and what gaps exist — not the top-10 results for "INV-006."

B) **Curated summaries** — Generate a prose summary of the target's context.
- Pros: Natural language is easy for LLMs to consume.
- Cons: Summary generation requires an LLM (circular dependency). Prose summaries are not auditable — the user cannot verify which intelligence was included or excluded.

C) **Structured 9-signal bundles** — Compose nine distinct intelligence signals into a structured bundle with named fields.
- Pros: Each signal is independently auditable. The structure is machine-parseable (JSON) and human-readable (formatted output). Signals cover all four reasoning modes (Formal, Causal, Practical, Meta). Self-contained: an LLM receiving the bundle has everything it needs.
- Cons: More complex to implement. The nine signals must be maintained as the index schema evolves.

#### Decision

**Option C: Structured 9-signal bundles.** Each context bundle contains:

| # | Signal | Source Function | Purpose |
|---|--------|-----------------|---------|
| 1 | Target content | `query.QueryTarget` | The element's full text with resolved references |
| 2 | Constraints | `findConstraints` | Invariants, gates, and negative specs constraining the target |
| 3 | Invariant completeness | `checkInvariantCompleteness` | Whether constraining invariants have all 5 required components |
| 4 | Coverage gaps | `findCoverageGaps` | Missing glossary terms (INV-009), missing verification prompts (INV-017) |
| 5 | Local validation | `runLocalValidation` | Scoped checks: cross-ref resolution, glossary coverage |
| 6 | Reasoning modes | `tagReasoningModes` | Elements tagged Formal/Causal/Practical/Meta within the target's region |
| 7 | Related elements | `LSIIndex.RankAll` | Semantically similar elements via LSI cosine similarity |
| 8 | Impact radius | `impact.Analyze` | Forward and backward dependency analysis (BFS, depth-bounded) |
| 9 | Editing guidance | `generateGuidance` + `getRecentChanges` | Synthesized guidance from constraints, gaps, and recent oplog changes |

// WHY NOT Option A (raw search)? Search answers "what is relevant to query X?" Context answers "what does an editor need to know about element Y?" Different questions, different data structures.

// WHY NOT Option B (curated summaries)? Requires an LLM to generate, creating a circular dependency. Prose summaries cannot be mechanically audited for completeness. Structured signals can.

#### Consequences

- Every context bundle is self-contained (APP-INV-005): an LLM receiving it has sufficient information to make constrained edits.
- The nine signals provide coverage across all four reasoning modes: Formal (invariant completeness), Causal (constraints from ADRs), Practical (related elements, coverage gaps), Meta (editing guidance, negative specs).
- Adding a tenth signal in the future requires updating `BuildContext`, `ContextBundle` struct, `RenderContext`, and the APP-INV-005 definition.
- Context bundles reference only elements present in the spec index (no external lookups), satisfying the negative specification "Must NOT build context bundles that reference elements outside the spec index."

#### Tests

- (Validated by APP-INV-005) Every bundle has all nine signal fields present.
- `tests/search_test.go::TestContextBundleSection` and `TestContextBundleInvariant` verify bundle assembly.
- `tests/search_test.go::TestContextBundleJSON` verifies JSON round-trip fidelity.
- `tests/search_test.go::TestContextEditingGuidance` verifies guidance synthesis.

---

## Implementation

### Chapter: BM25 via FTS5

**Preserves:** APP-INV-008 (RRF Fusion Correctness — BM25 provides the lexical ranking signal), APP-INV-014 (Glossary Expansion Bound — expanded query feeds into FTS5).

The BM25 signal is provided by SQLite's FTS5 extension, which implements the Okapi BM25 ranking function natively. The CLI does not reimplement BM25; it delegates to FTS5 and consumes the ranked results.

#### Document Extraction

`ExtractDocuments` pulls all indexable elements from the spec database into `SearchDocument` structs. Six element types are extracted:

| Element Type | Document Content | ElementID Format |
|---|---|---|
| section | Title + RawText | Section path (e.g., `§0.5`) |
| invariant | Title + Statement + SemiFormal + ViolationScenario + ValidationMethod + WhyThisMatters | Invariant ID (e.g., `INV-006`) |
| adr | Title + Problem + DecisionText + Consequences | ADR ID (e.g., `ADR-003`) |
| gate | Title + Predicate | Gate ID (e.g., `Gate-1`) |
| glossary | Term + Definition | `glossary:<Term>` |
| negative_spec | ConstraintText + Reason | `neg-spec:<ID>` |

Each document receives a 0-based `DocID` for internal indexing. The `SectionID` field links back to the owning section for scoped queries.

#### FTS5 Population

`PopulateFTS` clears the existing FTS5 index (via `storage.ClearFTSIndex`) and inserts each document as a row in the `fts_index` virtual table with columns: `element_type`, `element_id`, `title`, `content`. The FTS5 tokenizer is SQLite's default Unicode61 tokenizer.

#### Query Sanitization

`sanitizeFTSQuery` prepares user input for FTS5:

1. **Element ID detection**: If the query matches an element ID pattern (`INV-`, `ADR-`, `§`, `Gate-`, `APP-INV-`, `APP-ADR-`, `PART-`, `Chapter-`, `Appendix-`), wrap in double quotes for exact phrase matching.
2. **Quoted passthrough**: If already double-quoted, pass through unchanged.
3. **Term splitting**: Split on whitespace, strip leading hyphens (FTS5 NOT operator), remove bare FTS5 operators (`OR`, `AND`, `NOT`), join remaining terms with implicit AND (FTS5 default).

This prevents FTS5 syntax errors from user input while preserving the ability to search for element IDs containing hyphens.

#### Glossary Expansion

`expandQuery` enriches the user's query with domain-specific synonyms from the glossary:

```
Algorithm: Glossary-Based Query Expansion
Input: query string q, glossary entries G
Output: expansion terms E (|E| ≤ 5)

1. E ← {}
2. For each glossary entry (term, definition) in G:
   a. If lowercase(q) contains lowercase(term):
      - Extract words from definition where len(word) > 4
      - For each word not already in q:
        - Add word to E
        - If |E| ≥ 5: return E       // APP-INV-014 enforced here
   b. For each word w in q where len(w) > 3:
      - If lowercase(definition) contains w AND q does not contain term:
        - Add term to E
        - If |E| ≥ 5: return E       // APP-INV-014 enforced here
        - Break (one expansion per glossary entry for reverse matching)
3. Return E
```

**Complexity:** O(|G| x max(|definition_words|, |query_words|)). For a glossary of 50 terms with 20-word definitions and a 5-word query, this is ~5,000 comparisons — negligible.

**Worked example:**

Given glossary: `{ "cross-reference": "An explicit identifier (§X.Y, INV-NNN, ADR-NNN) linking one section to another section or element" }`

Query: `"cross-reference density"`

Step 2a matches: `"cross-reference"` is in the query. Extract significant words from definition: `["explicit", "identifier", "linking", "section", "another", "section", "element"]`. Filter for length > 4 and not in query: `["explicit", "identifier", "linking", "section", "another"]`. The first 5 are added.

Result: `expandQuery(q) = ["explicit", "identifier", "linking", "section", "another"]`

Full query sent to FTS5: `"cross-reference density explicit identifier linking section another"`

**Edge cases:**
- Empty glossary: returns nil (no expansion).
- Query matches no terms: returns nil.
- Short query words (length ≤ 3): skipped in reverse matching to avoid noise.

**Implementation Trace:**
- Source: `internal/search/engine.go::expandQuery`
- Source: `internal/search/fts.go::PopulateFTS`
- Source: `internal/search/fts.go::SearchFTS`
- Source: `internal/search/fts.go::sanitizeFTSQuery`
- Source: `internal/search/documents.go::ExtractDocuments`
- Tests: `tests/search_test.go::TestSearchGlossaryExpansion`
- Tests: `tests/search_test.go::TestSearchExactMatch`

---

### Chapter: Latent Semantic Indexing (LSI)

**Preserves:** APP-INV-012 (LSI Dimension Bound — k never exceeds document count), APP-INV-008 (RRF Fusion Correctness — LSI provides the semantic ranking signal).

LSI discovers latent semantic relationships between terms and documents by applying Singular Value Decomposition (SVD) to a TF-IDF weighted term-document matrix, then truncating to k dimensions. Documents that share related concepts (but not identical terms) become neighbors in the reduced k-dimensional space.

#### TF-IDF Matrix Construction

`BuildLSI` constructs the term-document matrix in three steps:

1. **Tokenization**: Each document's content is tokenized by `tokenize()`, which extracts lowercase alphanumeric sequences (including `§` and `-` for element IDs) via the regex `[a-zA-Z0-9§\-]+`.

2. **Vocabulary construction**: Build a global vocabulary mapping each unique term to a column index. Simultaneously compute per-document term frequencies and document frequencies.

3. **TF-IDF weighting**: For each (term, document) pair:
   ```
   tf_weight(t, d)  = 1 + log(count(t, d))    // sublinear TF
   idf(t)           = log(N / df(t))           // inverse document frequency
   tfidf(t, d)      = tf_weight(t, d) × idf(t)
   ```

   The matrix is oriented as `(nTerms × nDocs)` — terms are rows, documents are columns.

#### SVD Truncation

The TF-IDF matrix A is factorized as A = U x S x V^T using `gonum/mat.SVD` (thin factorization):

- **U** (nTerms x r): term-concept matrix. Each column is a latent concept axis.
- **S** (r x r, diagonal): singular values in decreasing order. Magnitude indicates concept importance.
- **V** (nDocs x r): document-concept matrix. Each row is a document's position in concept space.

Truncation to k dimensions retains only the first k columns of U, the first k singular values, and the first k columns of V:

```
k = min(50, nDocs, nTerms)     // APP-INV-012 enforced
Uk = U[:, :k]                  // nTerms × k
Sk = S[:k]                     // k singular values
Vk = V[:, :k]                  // nDocs × k
```

**Document vectors** are computed as `DocVectors[i] = Vk[i, :] * Sk` (each row of Vk scaled by singular values), producing k-dimensional vectors that embed each document in the latent semantic space.

#### Query Projection

`QueryVec` projects a query string into the LSI space:

```
Algorithm: LSI Query Projection
Input: query string q, LSIIndex (Uk, Sk, TermIndex, IDF)
Output: k-dimensional query vector qk

1. Tokenize q into terms
2. Compute TF-IDF vector qvec (length = |vocabulary|):
   For each term t in q:
     if t ∈ TermIndex:
       qvec[TermIndex[t]] = (1 + log(count(t))) × IDF[TermIndex[t]]
3. Project into LSI space:
   qk[j] = Σ_i (qvec[i] × Uk[i, j])  for j = 0..k-1
   (skip division by Sk to maintain consistency with document vectors)
4. Return qk
```

Note: The projection omits the `Sk^{-1}` division that appears in standard LSI literature. This is intentional — since document vectors already incorporate Sk scaling (`Vk * Sk`), the query vector must also remain unscaled for cosine similarity to produce meaningful results. The projection effectively computes `q^T * Uk`, not `q^T * Uk * Sk^{-1}`.

#### Cosine Similarity and Ranking

`CosineSimilarity` computes the cosine between a query vector and a document vector:

```
cosine(a, b) = (a · b) / (||a|| × ||b||)

where a · b = Σ_i a[i] × b[i]
      ||a|| = sqrt(Σ_i a[i]²)
```

Guard: if either vector has zero norm, return 0 (avoids division by zero for documents with no matching terms).

`RankAll` computes cosine similarity for all documents and sorts descending. Documents with similarity ≤ 0 are included in the ranking (they may still contribute to RRF fusion if they have BM25 or authority signal) but are typically filtered out in the context bundle builder (threshold: 0.1).

#### Worked Example: 3-Document LSI

**Corpus:**

| Doc | Content |
|---|---|
| D0 | "invariant falsifiability verification test" |
| D1 | "cross-reference density graph verification" |
| D2 | "invariant violation scenario counterexample" |

**Step 1: Vocabulary and TF**

Vocabulary (alphabetical order, 0-indexed): `{counterexample:0, cross-reference:1, density:2, falsifiability:3, graph:4, invariant:5, scenario:6, test:7, verification:8, violation:9}`

Term frequencies (each term appears once per document, so tf_weight = 1 + log(1) = 1.0):

| Term | D0 | D1 | D2 | df | IDF = log(3/df) |
|---|---|---|---|---|---|
| counterexample | 0 | 0 | 1 | 1 | 1.099 |
| cross-reference | 0 | 1 | 0 | 1 | 1.099 |
| density | 0 | 1 | 0 | 1 | 1.099 |
| falsifiability | 1 | 0 | 0 | 1 | 1.099 |
| graph | 0 | 1 | 0 | 1 | 1.099 |
| invariant | 1 | 0 | 1 | 2 | 0.405 |
| scenario | 0 | 0 | 1 | 1 | 1.099 |
| test | 1 | 0 | 0 | 1 | 1.099 |
| verification | 1 | 1 | 0 | 2 | 0.405 |
| violation | 0 | 0 | 1 | 1 | 1.099 |

**Step 2: TF-IDF matrix** (10 terms x 3 docs, showing non-zero entries):

```
A = [[0,     0,     1.099],   // counterexample
     [0,     1.099, 0    ],   // cross-reference
     [0,     1.099, 0    ],   // density
     [1.099, 0,     0    ],   // falsifiability
     [0,     1.099, 0    ],   // graph
     [0.405, 0,     0.405],   // invariant
     [0,     0,     1.099],   // scenario
     [1.099, 0,     0    ],   // test
     [0.405, 0.405, 0    ],   // verification
     [0,     0,     1.099]]   // violation
```

**Step 3: SVD and truncation** to k = min(50, 3) = 3 dimensions. The SVD produces U (10x3), S (3 diagonal), V (3x3). Document vectors = rows of V scaled by S.

**Step 4: Query** `"invariant verification"`

Query TF-IDF vector has non-zero entries at positions 5 (invariant: 0.405) and 8 (verification: 0.405). Project into 3D space via `q^T * Uk`. Compute cosine similarity against all three document vectors.

Expected result: D0 ranks highest (shares both "invariant" and "verification"), D2 second (shares "invariant"), D1 third (shares "verification" only).

**Complexity:**
- Index build: O(nTerms x nDocs x min(nTerms, nDocs)) — dominated by SVD
- Query: O(nTerms x k + nDocs x k) — matrix-vector multiply + cosine for all docs
- Storage: O(nTerms x k + nDocs x k) — Uk matrix + document vectors

**Edge cases:**
- Single document: k = 1, all similarities are 1.0 or 0.0.
- Empty vocabulary (all stop words): BuildLSI returns an empty index; QueryVec returns nil.
- SVD factorization failure: returns empty LSIIndex with TermIndex and IDF but no Uk. QueryVec returns nil, and the LSI signal is gracefully excluded from RRF fusion.

**Implementation Trace:**
- Source: `internal/search/lsi.go::BuildLSI`
- Source: `internal/search/lsi.go::QueryVec`
- Source: `internal/search/lsi.go::CosineSimilarity`
- Source: `internal/search/lsi.go::RankAll`
- Source: `internal/search/lsi.go::tokenize`
- Source: `internal/search/lsi.go::cosine`
- Tests: `tests/search_test.go::TestSearchLSIBuild`
- Tests: `tests/search_test.go::TestSearchSemanticMatch`

---

### Chapter: PageRank Authority

**Preserves:** APP-INV-004 (Authority Monotonicity — adding an edge cannot decrease score), APP-INV-008 (RRF Fusion Correctness — authority provides the structural ranking signal).

**Interfaces:** APP-INV-003 (Cross-Reference Integrity — the authority graph is built from resolved cross-references; dangling references produce phantom nodes).

PageRank computes a global authority score for each element in the specification based on the cross-reference graph. Elements that are heavily referenced by other heavily-referenced elements receive higher scores. This captures the structural insight that some spec elements are more "important" than others — INV-006 (Cross-Reference Density), for example, is referenced by many sections and therefore has high authority.

#### Graph Construction

`ComputeAuthority` builds a directed graph from the `cross_references` table:

1. Query all resolved cross-references (`resolved = 1`) for the spec.
2. Map `source_section_id` to section paths via `storage.ListSections`.
3. Create directed edges: `source_path → ref_target`.
4. Collect all unique nodes (both sources and targets).

Nodes are section paths (e.g., `§0.5`) and element IDs (e.g., `INV-006`, `ADR-003`). The graph is heterogeneous — sections reference invariants, invariants reference sections, ADRs reference both.

#### PageRank Iteration

```
Algorithm: PageRank (iterative power method)
Input: directed graph G = (V, E), damping factor d = 0.85,
       max iterations = 100, convergence threshold ε = 1e-6
Output: score map PR: V → [0, 1]

1. n ← |V|
2. Initialize: PR(v) ← 1/n for all v ∈ V
3. Compute out_degree(v) for all v ∈ V
4. Repeat up to 100 iterations:
   a. new_PR(v) ← (1 - d) / n for all v ∈ V      // teleportation base
   b. For each edge (u → v) ∈ E:
        new_PR(v) += d × PR(u) / out_degree(u)     // rank propagation
   c. Compute dangling node contribution:
        dangling_sum ← Σ PR(v) for all v where out_degree(v) = 0
        For all v ∈ V:
          new_PR(v) += d × dangling_sum / n         // distribute dangling mass
   d. diff ← Σ |new_PR(v) - PR(v)| for all v ∈ V
   e. PR ← new_PR
   f. If diff < ε: break                           // converged
5. Persist: for each v ∈ V, insert (specID, v, PR(v)) into authority_scores
6. Return PR
```

**Complexity:** O(iterations x (|V| + |E|)). For a spec with 200 elements and 500 cross-references, each iteration touches 700 items. Convergence typically occurs in 20-40 iterations, so total work is ~15,000-28,000 operations — sub-millisecond.

**Dangling node handling:** Nodes with no outgoing edges (e.g., glossary terms that are referenced but never reference anything) accumulate PageRank but never distribute it along edges. Without correction, their rank "leaks" out of the system. The dangling mass redistribution (step 4c) ensures the total rank sums to 1.0 across all iterations.

**Worked example:**

Graph with 4 nodes:
```
§0.5 → INV-006
§0.5 → ADR-003
INV-006 → §0.7
ADR-003 → INV-006
```

out_degree: `{§0.5: 2, INV-006: 1, ADR-003: 1, §0.7: 0 (dangling)}`

Initial: PR = {§0.5: 0.25, INV-006: 0.25, ADR-003: 0.25, §0.7: 0.25}

Iteration 1:
- Base: all nodes get (1-0.85)/4 = 0.0375
- Edge contributions:
  - INV-006 gets 0.85 x 0.25/2 (from §0.5) + 0.85 x 0.25/1 (from ADR-003) = 0.10625 + 0.2125 = 0.31875
  - ADR-003 gets 0.85 x 0.25/2 (from §0.5) = 0.10625
  - §0.7 gets 0.85 x 0.25/1 (from INV-006) = 0.2125
  - §0.5 gets 0 from edges
- Dangling: §0.7 has out_degree 0, dangling_sum = 0.25, contrib = 0.85 x 0.25/4 = 0.053125
- Final iteration 1: §0.5 = 0.0906, INV-006 = 0.4094, ADR-003 = 0.1969, §0.7 = 0.3031

INV-006 emerges with the highest score because it receives edges from both §0.5 and ADR-003.

**APP-INV-004 verification in this example:** If we add edge `§0.7 → INV-006`, INV-006 gains an additional inbound contribution `d × PR(§0.7) / out_degree(§0.7)`. Since this is strictly positive (PR and d are positive, out_degree ≥ 1), INV-006's score can only increase.

**Implementation Trace:**
- Source: `internal/search/authority.go::ComputeAuthority`
- Tests: `tests/search_test.go::TestSearchAuthorityComputed`
- Validates-via: Property test — add edge, recompute, assert target score non-decreasing

---

### Chapter: RRF Fusion

**Preserves:** APP-INV-008 (RRF Fusion Correctness — this chapter IS the fusion specification).

Reciprocal Rank Fusion (RRF) merges multiple ranked lists into a single ranking without requiring score normalization. Each signal contributes `weight / (K + rank)` to a document's fused score. The constant K = 60 prevents top-ranked documents from dominating excessively (without K, rank #1 contributes 1.0 while rank #2 contributes 0.5 — too steep). With K = 60, rank #1 contributes 1/61 and rank #2 contributes 1/62 — a gentle slope.

#### Fusion Algorithm

```
Algorithm: Weighted RRF with Type Boost
Input: ranked lists R_bm25, R_lsi, R_auth; weights W; type_boost map; K = 60
Output: fused results sorted by score descending

Constants:
  W = {bm25: 1.0, lsi: 1.0, authority: 0.5}
  type_boost = {invariant: 1.2, adr: 1.1, gate: 1.1,
                section: 1.0, negative_spec: 0.9, glossary: 0.8}

1. elements ← {}  // map from elementID to info
2. For each (i, result) in enumerate(R_bm25):
     elements[result.id].bm25_rank ← i + 1           // 1-indexed! (APP-INV-008)
3. For each (i, result) in enumerate(R_lsi):
     elements[result.id].lsi_rank ← i + 1
4. For each (i, result) in enumerate(R_auth):
     elements[result.id].auth_rank ← i + 1
5. For each element e in elements:
     raw_score ← 0
     if e.bm25_rank > 0:  raw_score += W.bm25     / (K + e.bm25_rank)
     if e.lsi_rank > 0:   raw_score += W.lsi      / (K + e.lsi_rank)
     if e.auth_rank > 0:  raw_score += W.authority / (K + e.auth_rank)
     e.score ← raw_score × type_boost[e.element_type]
6. Sort elements by score descending
7. Return top opts.Limit elements
```

**Critical invariant enforcement (APP-INV-008):** Ranks are 1-indexed. In the implementation, BM25 results from FTS5 arrive sorted by BM25 score; the loop index `i` starts at 0, so the rank is assigned as `i + 1`. The same pattern applies to LSI (sorted by descending cosine similarity) and authority (sorted by descending PageRank score). A 0-indexed rank would produce `1/(K + 0) = 1/60` instead of the correct `1/(K + 1) = 1/61` for the top-ranked document.

#### Worked Example: 5-Document RRF Fusion

Five documents ranked by three signals:

| Document | BM25 Rank | LSI Rank | Authority Rank | Element Type |
|---|---|---|---|---|
| INV-006 | 1 | 3 | 1 | invariant |
| §0.5 | 2 | 1 | 2 | section |
| ADR-003 | 3 | 2 | — | adr |
| Gate-5 | — | 4 | 3 | gate |
| §8.3 | 4 | — | 4 | section |

"—" means the signal did not rank this document (rank = 0, excluded from sum).

**Score computation (K = 60):**

| Document | BM25 term | LSI term | Auth term | Raw Score | Type Boost | Final Score |
|---|---|---|---|---|---|---|
| INV-006 | 1.0/(60+1) = 0.01639 | 1.0/(60+3) = 0.01587 | 0.5/(60+1) = 0.00820 | 0.04046 | x 1.2 | **0.04855** |
| §0.5 | 1.0/(60+2) = 0.01613 | 1.0/(60+1) = 0.01639 | 0.5/(60+2) = 0.00806 | 0.04059 | x 1.0 | **0.04059** |
| ADR-003 | 1.0/(60+3) = 0.01587 | 1.0/(60+2) = 0.01613 | 0 | 0.03200 | x 1.1 | **0.03520** |
| Gate-5 | 0 | 1.0/(60+4) = 0.01563 | 0.5/(60+3) = 0.00794 | 0.02356 | x 1.1 | **0.02592** |
| §8.3 | 1.0/(60+4) = 0.01563 | 0 | 0.5/(60+4) = 0.00781 | 0.02344 | x 1.0 | **0.02344** |

**Final ranking:** INV-006 (0.04855) > §0.5 (0.04059) > ADR-003 (0.03520) > Gate-5 (0.02592) > §8.3 (0.02344)

Observations:
- INV-006 wins despite §0.5 having a slightly higher raw score (0.04059 vs 0.04046) because the invariant type boost (1.2) amplifies the score.
- ADR-003 outranks Gate-5 despite missing the authority signal, because it ranks highly in both BM25 and LSI.
- §8.3 ranks last with two signals but low ranks in both.

**Edge cases:**
- Single signal: A document appearing in only one ranked list still receives a score. With BM25 rank 1 only: `1.0/61 x type_boost ≈ 0.016-0.020`.
- All signals tied at rank 1: `raw_score = 1.0/61 + 1.0/61 + 0.5/61 = 2.5/61 = 0.04098`.
- Empty result from one signal: Signal is silently excluded from the sum (rank = 0 means "not ranked").

**Implementation Trace:**
- Source: `internal/search/engine.go::Search` (lines 92–270: full search orchestration)
- Source: `internal/search/engine.go::BuildIndex` (lines 32–89: index construction pipeline)
- Tests: `tests/search_test.go::TestSearchRRFFusion`
- Tests: `tests/search_test.go::TestSearchExactMatch`
- Tests: `tests/search_test.go::TestSearchJSON`

---

### Chapter: Context Intelligence Bundles

**Preserves:** APP-INV-005 (Context Self-Containment — all 9 signals present), APP-INV-012 (LSI Dimension Bound — related-elements signal uses LSI).

**Interfaces:** APP-INV-003 (Cross-Reference Integrity — constraints and impact radius depend on resolved references), APP-INV-016 (Implementation Traceability — context bundles reference source functions that must exist).

The context intelligence system answers the question: "Given that I want to edit element X, what do I need to know?" The answer is a structured bundle of nine signals, each computed independently and assembled by `BuildContext`.

#### Signal Assembly Pipeline

`BuildContext(db, specID, target, lsi, oplogPath, depth, relatedLimit)` orchestrates signal assembly:

```
Algorithm: Context Bundle Assembly
Input: target element ID, LSIIndex, oplog path, depth (default 2), relatedLimit (default 5)
Output: ContextBundle with 9 signals

1. Resolve target → Fragment via query.QueryTarget (with ResolveRefs=true, Backlinks=true)
2. Initialize bundle with target metadata (ID, type, title, content, line range)

Signal assembly (each step populates one bundle field):
3. constraints       ← findConstraints(db, specID, fragment)
4. inv_completeness  ← checkInvariantCompleteness(db, specID, fragment, constraints)
5. coverage_gaps     ← findCoverageGaps(db, specID, fragment)
6. local_validation  ← runLocalValidation(db, specID, fragment)
7. reasoning_modes   ← tagReasoningModes(db, specID, fragment)
8. related_elements  ← LSI cosine similarity (top relatedLimit, similarity > 0.1, excluding self)
9. impact_radius     ← impact.Analyze(db, specID, target, direction="both", maxDepth=depth)
10. recent_changes   ← getRecentChanges(oplogPath, fragment.ID)  // only if oplogPath non-empty
11. editing_guidance  ← generateGuidance(bundle)

Return bundle
```

#### Signal 2: Constraint Discovery

`findConstraints` identifies all invariants, quality gates, and negative specifications that constrain the target element:

1. **Invariant matching**: Scan all invariants. If the target's content (case-insensitive) mentions an invariant ID, it is a constraint. Also check backlinks for `INV-` or `APP-INV-` prefixes.
2. **Quality gate matching**: If the target's content mentions a gate ID, or the gate's raw text mentions the target's ID, the gate is a constraint.
3. **Negative spec matching**: Negative specs whose `LineNumber` falls within the target's `[LineStart, LineEnd]` range are constraints.

Each constraint is tagged with type (`invariant`, `gate`, `negative_spec`), ID, and a truncated description.

#### Signal 3: Invariant Completeness

`checkInvariantCompleteness` checks each constraining invariant for the five required components:

| Component | Field | Check |
|---|---|---|
| Statement | `inv.Statement` | Non-empty after trimming whitespace |
| Semi-formal predicate | `inv.SemiFormal` | Non-empty after trimming |
| Validation method | `inv.ValidationMethod` | Non-empty after trimming |
| WHY THIS MATTERS | `inv.WhyThisMatters` | Non-empty after trimming |
| Violation scenario | `inv.ViolationScenario` | Non-empty after trimming |

An invariant is `Complete` if all of Statement, SemiFormal, Validation, and WhyThisMatters are present. Missing fields are listed in `MissingFields` for guidance generation.

#### Signal 4: Coverage Gap Detection

`findCoverageGaps` identifies structural deficiencies:

1. **Glossary coverage (INV-009)**: Extract bold terms (`**term**`) from the target's content. Check each against the glossary. Report missing terms as a warning.
2. **Verification prompt coverage (INV-017)**: For chapter-level sections (`Chapter-*` or `PART-*`), check whether a verification prompt exists within the target's line range. Report absence as informational.

#### Signal 5: Local Validation

`runLocalValidation` runs scoped checks within the target's line range:

1. **Cross-reference resolution**: Query all cross-references with `source_line` in the target's range. Report `resolved/total` as passed/failed.
2. **Glossary coverage**: Count bold terms and check against the glossary. Report `defined/total`.

#### Signal 6: Reasoning Mode Tags

`tagReasoningModes` classifies related elements by their reasoning mode:

| Mode | Element Types | Discovery Method |
|---|---|---|
| Formal | invariant, state_machine | Line range overlap with target |
| Causal | adr | Referenced from target (via resolved refs) |
| Practical | worked_example | Line range overlap with target |
| Meta | negative_spec, why_not, comparison | Line range overlap with target |

This tagging enables LLM editors to distinguish *what must hold* (Formal), *why it was chosen* (Causal), *how it looks in practice* (Practical), and *what must not happen* (Meta).

#### Signal 7: Related Elements via LSI

If an LSI index is available, compute `QueryVec` from the target's title + content, then `RankAll` to find semantically similar elements. Filter: exclude self, exclude similarity < 0.1, limit to `relatedLimit` (default 5). Each related element includes type, ID, title, and similarity score.

#### Signal 8: Impact Radius

`impact.Analyze` performs a bounded BFS over the cross-reference graph in both directions (forward: "what does this affect?" and backward: "what affects this?"). Depth is configurable (default 2). Each node in the result includes distance from the target, element ID, and title. This signal tells the LLM: "if you change this element, these other elements may need updates."

#### Signal 9: Editing Guidance and Recent Changes

`generateGuidance` synthesizes actionable guidance from the bundle's constraints, gaps, and completeness data:

1. For each invariant constraint: "Ensure compliance with {ID} ({description})"
2. For each gate constraint: "Satisfy {ID} before proceeding"
3. For each negative spec constraint: the constraint text verbatim
4. For each coverage gap: the gap description
5. For each incomplete invariant: "Add missing {fields} to {ID}"
6. Always append: "Run `ddis validate` after changes"

`getRecentChanges` scans the oplog (if provided) for entries matching the target's ID. It checks both diff records (element-level changes) and transaction records (description mentions). Each matching entry becomes a `ChangeRecord` with timestamp, type, and summary.

#### Rendering

`RenderContext` produces either JSON (via `json.MarshalIndent`) or human-readable formatted output. The human-readable format groups signals under labeled headers: CONTENT, CONSTRAINTS, INVARIANT COMPLETENESS, COVERAGE GAPS, LOCAL VALIDATION, RELATED BY REASONING MODE, RELATED (via LSI), IMPACT RADIUS, RECENT CHANGES, EDITING GUIDANCE.

**Implementation Trace:**
- Source: `internal/search/context.go::BuildContext`
- Source: `internal/search/context.go::findConstraints`
- Source: `internal/search/context.go::checkInvariantCompleteness`
- Source: `internal/search/context.go::findCoverageGaps`
- Source: `internal/search/context.go::runLocalValidation`
- Source: `internal/search/context.go::tagReasoningModes`
- Source: `internal/search/context.go::generateGuidance`
- Source: `internal/search/context.go::getRecentChanges`
- Source: `internal/search/context.go::RenderContext`
- Source: `internal/search/context.go::renderHumanContext`
- Tests: `tests/search_test.go::TestContextBundleSection`
- Tests: `tests/search_test.go::TestContextBundleInvariant`
- Tests: `tests/search_test.go::TestContextBundleJSON`
- Tests: `tests/search_test.go::TestContextEditingGuidance`
- Tests: `tests/search_test.go::TestContextCoverageGaps`
- Tests: `tests/search_test.go::TestContextInvariantCompleteness`
- Tests: `tests/search_test.go::TestContextReasoningModeTags`

---

## Negative Specifications

These constraints prevent the most likely implementation errors and LLM hallucination patterns for the search and context subsystems. Each addresses a failure mode that an LLM, given only the positive specification, would plausibly introduce.

**DO NOT** use online learning or mutable global state in search indices. The search index is built once at `BuildIndex` time and is immutable until the next explicit rebuild. No query may modify the index state. No global variables may cache search state across queries. Rationale: mutability introduces non-determinism. Two identical queries must produce identical results (APP-INV-002 interface).

**DO NOT** return results without a computable score derivation. Every `SearchResult.Score` must be reproducible from the formula `Σ weight_r / (K + rank_r) × type_boost`. No result may have a score assigned by heuristic, rounded to a fixed precision, or sourced from a cache without revalidation. Rationale: opaque scores are unauditable and untestable.

**DO NOT** build context bundles that reference elements outside the spec index. `BuildContext` may only include elements that exist in the spec database for the given `specID`. If an element ID appears in a cross-reference but was not parsed (dangling reference), it must not appear in the context bundle's constraints, related elements, or impact radius. Rationale: referencing non-existent elements causes LLM hallucination — the LLM trusts the bundle and produces edits referencing phantom elements.

**DO NOT** use 0-indexed ranks in RRF fusion. Ranks in the RRF formula are 1-indexed: the top-ranked document has rank 1, not rank 0. The implementation assigns rank as `loop_index + 1`. Using 0-indexed ranks would produce `1/(K + 0) = 1/60` for the top document instead of `1/(K + 1) = 1/61`, violating APP-INV-008. Rationale: this is the single most likely off-by-one error in the fusion algorithm.

**DO NOT** allow LSI k to exceed the document count or vocabulary size. The SVD truncation dimension k must satisfy `k ≤ min(doc_count, vocabulary_size)`. Requesting k > rank(matrix) causes the SVD to produce vectors padded with zeros or garbage, violating APP-INV-012. Rationale: the clamping logic in `BuildLSI` is the critical guard; removing or weakening it causes silent data corruption.

**DO NOT** omit dangling-node redistribution in PageRank. Nodes with zero out-degree accumulate rank mass but never distribute it. Without dangling-node redistribution (step 4c of the PageRank algorithm), the total rank across all nodes decreases with each iteration, eventually converging to near-zero for all nodes. The result would be a flat, useless authority signal. Rationale: this is a well-known PageRank implementation pitfall that produces silently degraded results rather than an error.

**DO NOT** divide query vector by singular values in LSI projection. The implementation computes `q^T × Uk` without dividing by Sk. Since document vectors are computed as `Vk × Sk`, the scaling is consistent. Dividing by Sk would double-penalize low-variance dimensions, degrading similarity quality. Rationale: the standard LSI formulation varies between sources; the implementation must be internally consistent, not textbook-correct.

**DO NOT** filter out negative-similarity documents before RRF fusion. A document may have negative cosine similarity in LSI (rare, but possible for documents antithetical to the query) while still ranking highly in BM25. Filtering negative-similarity documents from the LSI ranked list before fusion would remove them from the RRF candidate set entirely, causing a document relevant by keyword to vanish from results. LSI similarity filtering (threshold 0.1) is appropriate only in the context bundle builder, not in the search pipeline.

---

## Verification Prompt

Use this self-check after implementing or modifying the search-intelligence subsystem.

**Positive checks (DOES the implementation...):**

- DOES the `BuildIndex` pipeline execute all three signal builders in sequence: `PopulateFTS`, `BuildLSI`, `ComputeAuthority`? (APP-INV-008)
- DOES `BuildLSI` clamp k to `min(50, doc_count, vocab_size)` before calling SVD? (APP-INV-012)
- DOES the RRF loop assign ranks as `loop_index + 1` (1-indexed) for all three signals? (APP-INV-008, NEG-SEARCH-004)
- DOES `expandQuery` enforce `len(expansions) >= 5` as an early-return guard? (APP-INV-014)
- DOES `BuildContext` populate all nine bundle fields regardless of element type? (APP-INV-005)
- DOES `ComputeAuthority` redistribute dangling-node mass at each iteration? (NEG-SEARCH-006)
- DOES `QueryVec` skip division by Sk, consistent with DocVectors = Vk x Sk? (NEG-SEARCH-007)
- DOES the RRF score equal `raw_score x type_boost` with no other adjustments? (APP-INV-008)

**Negative checks (does NOT the implementation...):**

- Does NOT modify the FTS5 index, LSI model, or authority scores during a search query? (NEG-SEARCH-001)
- Does NOT return a score that cannot be recomputed from the RRF formula + type boost? (NEG-SEARCH-002)
- Does NOT include phantom elements (unresolved references) in context bundles? (NEG-SEARCH-003)
- Does NOT use 0-indexed ranks anywhere in the RRF computation? (NEG-SEARCH-004)
- Does NOT request SVD dimensions exceeding document count? (NEG-SEARCH-005, APP-INV-012)
- Does NOT filter LSI results by similarity threshold before fusion? (NEG-SEARCH-008)
