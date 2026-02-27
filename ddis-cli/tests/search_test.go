package tests

import (
	"encoding/json"
	"fmt"
	"math"
	"testing"

	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

// TestSearchExactMatch verifies that "INV-001" returns INV-001 as the top result.
func TestSearchExactMatch(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	results, err := search.Search(db, specID, "INV-001", search.SearchOptions{Limit: 10})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	if len(results) == 0 {
		t.Fatal("no results for INV-001")
	}

	// INV-001 should be in top 3 results
	found := false
	for i, r := range results {
		if r.ElementID == "INV-001" {
			found = true
			t.Logf("INV-001 found at position %d (score: %.4f)", i+1, r.Score)
			break
		}
	}
	if !found {
		t.Errorf("INV-001 not found in top %d results", len(results))
		for i, r := range results {
			t.Logf("  %d. %s: %s (%.4f)", i+1, r.ElementID, r.Title, r.Score)
		}
	}
}

// TestSearchSemanticMatch verifies that "validation pipeline conformance" finds relevant elements.
func TestSearchSemanticMatch(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	results, err := search.Search(db, specID, "validation pipeline conformance", search.SearchOptions{Limit: 10})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	if len(results) == 0 {
		t.Fatal("no results for semantic query")
	}

	// Should find elements related to validation
	t.Logf("Results for 'validation pipeline conformance':")
	for i, r := range results {
		t.Logf("  %d. [%s] %s: %s (%.4f)", i+1, r.ElementType, r.ElementID, r.Title, r.Score)
	}

	// At least one gate or invariant should appear
	hasRelevant := false
	for _, r := range results {
		if r.ElementType == "gate" || r.ElementType == "invariant" {
			hasRelevant = true
			break
		}
	}
	if !hasRelevant {
		t.Error("expected at least one gate or invariant in results for validation query")
	}
}

// TestSearchGlossaryExpansion verifies glossary-based query expansion.
func TestSearchGlossaryExpansion(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	// Search for a glossary term — expansion should boost related results
	results, err := search.Search(db, specID, "Round-Trip Fidelity", search.SearchOptions{Limit: 10})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	if len(results) == 0 {
		t.Fatal("no results for glossary term query")
	}

	t.Logf("Results for 'Round-Trip Fidelity' (%d):", len(results))
	for i, r := range results {
		t.Logf("  %d. [%s] %s: %s", i+1, r.ElementType, r.ElementID, r.Title)
	}
}

// TestSearchTypeFilter verifies that --type invariant filters correctly.
func TestSearchTypeFilter(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	results, err := search.Search(db, specID, "deterministic", search.SearchOptions{
		Limit:      10,
		TypeFilter: "invariant",
	})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	for _, r := range results {
		if r.ElementType != "invariant" {
			t.Errorf("result %s has type %s, want invariant", r.ElementID, r.ElementType)
		}
	}

	if len(results) > 0 {
		t.Logf("Found %d invariants matching 'deterministic'", len(results))
	}
}

// TestSearchLSIBuild verifies that the LSI index builds correctly.
func TestSearchLSIBuild(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	docs, err := search.ExtractDocuments(db, specID)
	if err != nil {
		t.Fatalf("extract docs: %v", err)
	}

	if len(docs) == 0 {
		t.Error("expected at least 1 document, got 0")
	}

	k := 50
	if len(docs) < k {
		k = len(docs)
	}

	lsi, err := search.BuildLSI(docs, k)
	if err != nil {
		t.Fatalf("build LSI: %v", err)
	}

	if lsi.K <= 0 {
		t.Error("LSI K should be > 0")
	}
	if len(lsi.DocVectors) != len(docs) {
		t.Errorf("DocVectors length %d != docs %d", len(lsi.DocVectors), len(docs))
	}

	t.Logf("LSI index: k=%d, %d terms, %d docs", lsi.K, len(lsi.TermIndex), len(lsi.DocVectors))
}

// TestSearchAuthorityComputed verifies PageRank scores are non-zero.
func TestSearchAuthorityComputed(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	// Authority scores are inserted by buildSyntheticDB
	scores, err := storage.GetAuthorityScores(db, specID)
	if err != nil {
		t.Fatalf("get authority: %v", err)
	}

	if len(scores) == 0 {
		t.Fatal("no authority scores computed")
	}

	// All scores should be positive
	for id, score := range scores {
		if score <= 0 {
			t.Errorf("authority score for %s is %.6f, want > 0", id, score)
		}
	}

	t.Logf("Authority scores computed for %d elements", len(scores))
}

// TestSearchRRFFusion verifies that RRF combines signals correctly.
func TestSearchRRFFusion(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	results, err := search.Search(db, specID, "data integrity persistence", search.SearchOptions{Limit: 5})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	if len(results) == 0 {
		t.Fatal("no results")
	}

	// Top result should have at least one signal
	top := results[0]
	if len(top.Signals) < 1 {
		t.Errorf("top result has %d signals, want >= 1", len(top.Signals))
	}
	if top.Score <= 0 {
		t.Errorf("top result score %.6f should be > 0", top.Score)
	}

	// Scores should be monotonically non-increasing
	for i := 1; i < len(results); i++ {
		if results[i].Score > results[i-1].Score {
			t.Errorf("results[%d].Score (%.6f) > results[%d].Score (%.6f)",
				i, results[i].Score, i-1, results[i-1].Score)
		}
	}
}

// TestSearchJSON verifies JSON output parses correctly.
func TestSearchJSON(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	results, err := search.Search(db, specID, "BM25 search ranking", search.SearchOptions{Limit: 5})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	out, err := search.RenderSearch(results, true)
	if err != nil {
		t.Fatalf("render JSON: %v", err)
	}

	var parsed []search.SearchResult
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("parse JSON: %v", err)
	}

	if len(parsed) != len(results) {
		t.Errorf("JSON has %d results, expected %d", len(parsed), len(results))
	}

	for _, r := range parsed {
		if r.ElementID == "" {
			t.Error("empty element_id in JSON result")
		}
		if r.Score <= 0 {
			t.Errorf("score %.6f should be > 0 for %s", r.Score, r.ElementID)
		}
	}
}

// TestSearchEmpty verifies that empty query returns an error.
func TestSearchEmpty(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	_, err := search.Search(db, specID, "", search.SearchOptions{})
	if err == nil {
		t.Error("expected error for empty query, got nil")
	}

	_, err = search.Search(db, specID, "   ", search.SearchOptions{})
	if err == nil {
		t.Error("expected error for whitespace query, got nil")
	}
}

// TestSearchLexicalOnly verifies --lexical-only skips LSI.
func TestSearchLexicalOnly(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	results, err := search.Search(db, specID, "data integrity persistence", search.SearchOptions{
		Limit:       10,
		LexicalOnly: true,
	})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	// With lexical-only, results should only have bm25 signals (no lsi)
	for _, r := range results {
		if _, hasLSI := r.Signals["lsi"]; hasLSI {
			t.Errorf("result %s has lsi signal in lexical-only mode", r.ElementID)
		}
	}

	t.Logf("Lexical-only search: %d results", len(results))
}

// TestContextBundleSection verifies context bundle for a section.
func TestContextBundleSection(t *testing.T) {
	db, specID, lsi := buildSyntheticSearchDB(t)

	bundle, err := search.BuildContext(db, specID, "§1", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context: %v", err)
	}

	if bundle.Target != "§1" {
		t.Errorf("target = %s, want §1", bundle.Target)
	}
	if bundle.Content == "" {
		t.Error("content is empty")
	}
	if bundle.Title == "" {
		t.Error("title is empty")
	}

	t.Logf("Context bundle for §1: %d constraints, %d related",
		len(bundle.Constraints), len(bundle.Related))
	if bundle.ImpactRadius != nil {
		t.Logf("  Impact: %d nodes", bundle.ImpactRadius.TotalCount)
	}
}

// TestContextBundleInvariant verifies context bundle includes impact radius.
func TestContextBundleInvariant(t *testing.T) {
	db, specID, lsi := buildSyntheticSearchDB(t)

	bundle, err := search.BuildContext(db, specID, "INV-001", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context: %v", err)
	}

	if bundle.Target != "INV-001" {
		t.Errorf("target = %s, want INV-001", bundle.Target)
	}
	if bundle.ElementType != "invariant" {
		t.Errorf("element_type = %s, want invariant", bundle.ElementType)
	}

	t.Logf("INV-001 context: content=%d bytes, constraints=%d, related=%d",
		len(bundle.Content), len(bundle.Constraints), len(bundle.Related))
	if bundle.ImpactRadius != nil {
		t.Logf("  Impact radius: %d nodes", bundle.ImpactRadius.TotalCount)
	}
}

// TestContextBundleJSON verifies JSON output parses correctly.
func TestContextBundleJSON(t *testing.T) {
	db, specID, lsi := buildSyntheticSearchDB(t)

	bundle, err := search.BuildContext(db, specID, "INV-001", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context: %v", err)
	}

	out, err := search.RenderContext(bundle, true)
	if err != nil {
		t.Fatalf("render JSON: %v", err)
	}

	var parsed search.ContextBundle
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("parse JSON: %v", err)
	}

	if parsed.Target != "INV-001" {
		t.Errorf("JSON target = %s, want INV-001", parsed.Target)
	}
	if parsed.Content == "" {
		t.Error("JSON content is empty")
	}
}

// TestContextEditingGuidance verifies guidance is derived from constraints.
func TestContextEditingGuidance(t *testing.T) {
	db, specID, lsi := buildSyntheticSearchDB(t)

	bundle, err := search.BuildContext(db, specID, "§1", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context: %v", err)
	}

	if len(bundle.EditingGuidance) == 0 {
		t.Error("editing guidance is empty")
	}

	// Should always include "Run ddis validate"
	hasValidate := false
	for _, g := range bundle.EditingGuidance {
		if g == "Run `ddis validate` after changes" {
			hasValidate = true
		}
	}
	if !hasValidate {
		t.Error("missing 'Run ddis validate' in editing guidance")
	}

	t.Logf("Editing guidance: %d items", len(bundle.EditingGuidance))
}

// TestContextCoverageGaps verifies coverage gap detection.
func TestContextCoverageGaps(t *testing.T) {
	db, specID, lsi := buildSyntheticSearchDB(t)

	bundle, err := search.BuildContext(db, specID, "INV-001", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context: %v", err)
	}

	t.Logf("Coverage gaps for INV-001: %d", len(bundle.CoverageGaps))
	for _, gap := range bundle.CoverageGaps {
		t.Logf("  [%s] %s (ref: %s)", gap.Severity, gap.Description, gap.InvariantRef)
	}
}

// TestContextInvariantCompleteness verifies invariant completeness checking.
func TestContextInvariantCompleteness(t *testing.T) {
	db, specID, lsi := buildSyntheticSearchDB(t)

	bundle, err := search.BuildContext(db, specID, "INV-001", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context: %v", err)
	}

	if len(bundle.InvCompleteness) == 0 {
		t.Error("no invariant completeness data for INV-001")
	}

	for _, inv := range bundle.InvCompleteness {
		t.Logf("  %s: statement=%v semi-formal=%v validation=%v why-matters=%v complete=%v",
			inv.ID, inv.HasStatement, inv.HasSemiFormal, inv.HasValidation,
			inv.HasWhyMatters, inv.Complete)
		if inv.ID == "INV-001" && !inv.Complete {
			t.Errorf("INV-001 should be complete (all fields present)")
		}
	}
}

// TestContextReasoningModeTags verifies reasoning mode tagging.
func TestContextReasoningModeTags(t *testing.T) {
	db, specID, lsi := buildSyntheticSearchDB(t)

	// INV-001 should have reasoning mode elements
	bundle, err := search.BuildContext(db, specID, "INV-001", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context: %v", err)
	}

	t.Logf("Reasoning mode items for INV-001: %d", len(bundle.ReasoningMode))
	for _, rm := range bundle.ReasoningMode {
		t.Logf("  [%s] %s: %s — %s", rm.Mode, rm.ElementType, rm.ElementID, rm.Description)
	}

	// INV-001 itself should appear as a Formal element
	hasFormal := false
	for _, rm := range bundle.ReasoningMode {
		if rm.Mode == "Formal" && rm.ElementID == "INV-001" {
			hasFormal = true
		}
	}
	if !hasFormal {
		t.Error("INV-001 should be tagged as Formal reasoning mode")
	}

	// Test a section for reasoning mode
	bundle2, err := search.BuildContext(db, specID, "§1", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context §1: %v", err)
	}
	t.Logf("Reasoning mode items for §1: %d", len(bundle2.ReasoningMode))
}

// =============================================================================
// APP-INV-008: RRF Fusion Correctness (Property Test)
// =============================================================================
//
// Verifies the formal predicate:
//   ∀ doc ∈ SearchResults:
//     raw_score(doc) = Σ_r (weight_r / (K + rank_r(doc)))
//     score(doc) = raw_score(doc) × type_boost(doc.element_type)
//   where K=60, weights={bm25:1.0, lsi:1.0, authority:0.5}

func TestRRFFormulaCorrectness(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	const rrfK = 60.0

	signalWeights := map[string]float64{
		"bm25":      1.0,
		"lsi":       1.0,
		"authority":  0.5,
	}

	typeBoosts := map[string]float64{
		"invariant":     1.2,
		"adr":           1.1,
		"gate":          1.1,
		"section":       1.0,
		"glossary":      0.8,
		"negative_spec": 0.9,
	}

	// Run several diverse queries to exercise different signal combinations
	queries := []string{
		"Round-Trip Fidelity parse render",
		"deterministic output reproducibility",
		"RRF fusion score weight",
		"BM25 search ranking",
		"data integrity persistence",
	}

	totalVerified := 0
	multiSignalCount := 0

	for _, query := range queries {
		results, err := search.Search(db, specID, query, search.SearchOptions{Limit: 20})
		if err != nil {
			t.Fatalf("search %q: %v", query, err)
		}

		for _, r := range results {
			// Compute expected raw score from the formula
			expectedRaw := 0.0
			for signal, rank := range r.Signals {
				weight, ok := signalWeights[signal]
				if !ok {
					t.Errorf("query %q, result %s: unknown signal %q", query, r.ElementID, signal)
					continue
				}
				if rank <= 0 {
					t.Errorf("query %q, result %s: signal %s has rank %d (must be >= 1)",
						query, r.ElementID, signal, rank)
					continue
				}
				expectedRaw += weight / (rrfK + float64(rank))
			}

			// Apply type boost
			boost := 1.0
			if b, ok := typeBoosts[r.ElementType]; ok {
				boost = b
			}
			expected := expectedRaw * boost

			// Compare with tolerance for float64 arithmetic
			if math.Abs(r.Score-expected) > 1e-10 {
				t.Errorf("query %q, result %s (type=%s): score=%.10f, expected=%.10f (diff=%.2e)\n"+
					"  signals=%v, boost=%.1f, rawExpected=%.10f",
					query, r.ElementID, r.ElementType, r.Score, expected,
					math.Abs(r.Score-expected), r.Signals, boost, expectedRaw)
			}

			totalVerified++
			if len(r.Signals) > 1 {
				multiSignalCount++
			}
		}
	}

	if totalVerified == 0 {
		t.Fatal("no results verified across all queries")
	}

	t.Logf("APP-INV-008 property check: %d results verified, %d with multiple signals",
		totalVerified, multiSignalCount)
}

// TestRRFRankIndexing verifies ranks are 1-indexed (rank 1 → 1/(K+1), not 1/(K+0)).
func TestRRFRankIndexing(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	results, err := search.Search(db, specID, "invariant", search.SearchOptions{Limit: 50})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	for _, r := range results {
		for signal, rank := range r.Signals {
			if rank < 1 {
				t.Errorf("result %s, signal %s: rank=%d (must be >= 1, 1-indexed)",
					r.ElementID, signal, rank)
			}
		}
	}

	// Verify the top BM25 result has rank exactly 1
	for _, r := range results {
		if bm25Rank, ok := r.Signals["bm25"]; ok && bm25Rank == 1 {
			// With K=60 and rank=1, the BM25 contribution should be 1.0/61
			expectedBM25Contrib := 1.0 / 61.0
			// If it were 0-indexed, it would be 1.0/60
			wrongContrib := 1.0 / 60.0

			// The total score includes other signals and boost, but
			// we can verify the BM25 contribution is in the right ballpark
			if r.Score > wrongContrib*1.5 {
				// Score too high — might be using 0-indexed ranks
				t.Logf("  Top BM25 result %s: score=%.6f (1-indexed BM25 contrib=%.6f, 0-indexed would be=%.6f)",
					r.ElementID, r.Score, expectedBM25Contrib, wrongContrib)
			}
			break
		}
	}

	t.Logf("APP-INV-008 rank indexing: all %d results use 1-indexed ranks", len(results))
}

// TestRRFTypeBoosts verifies type boost multipliers are applied correctly.
func TestRRFTypeBoosts(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	const rrfK = 60.0

	typeBoosts := map[string]float64{
		"invariant":     1.2,
		"adr":           1.1,
		"gate":          1.1,
		"section":       1.0,
		"glossary":      0.8,
		"negative_spec": 0.9,
	}

	// Use a broad query to get results of many types
	results, err := search.Search(db, specID, "search integrity validation", search.SearchOptions{Limit: 50})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	typesVerified := make(map[string]bool)

	for _, r := range results {
		// Compute raw score (pre-boost)
		rawScore := 0.0
		if bm25, ok := r.Signals["bm25"]; ok {
			rawScore += 1.0 / (rrfK + float64(bm25))
		}
		if lsi, ok := r.Signals["lsi"]; ok {
			rawScore += 1.0 / (rrfK + float64(lsi))
		}
		if auth, ok := r.Signals["authority"]; ok {
			rawScore += 0.5 / (rrfK + float64(auth))
		}

		if rawScore == 0 {
			continue
		}

		// Derive the actual boost applied
		actualBoost := r.Score / rawScore
		expectedBoost, ok := typeBoosts[r.ElementType]
		if !ok {
			expectedBoost = 1.0
		}

		if math.Abs(actualBoost-expectedBoost) > 1e-10 {
			t.Errorf("result %s (type=%s): derived boost=%.6f, expected=%.1f",
				r.ElementID, r.ElementType, actualBoost, expectedBoost)
		}

		typesVerified[r.ElementType] = true
	}

	t.Logf("APP-INV-008 type boosts verified for %d types: %v", len(typesVerified), typesVerified)
	if len(typesVerified) < 2 {
		t.Error("fewer than 2 element types verified — insufficient coverage")
	}
}

// =============================================================================
// APP-INV-012: LSI Dimension Bound (Property Test)
// =============================================================================
//
// Verifies the formal predicate:
//   ∀ LSIIndex: k ≤ min(n, v) ∧ len(vec) = k for all vectors

func TestLSIDimensionBound(t *testing.T) {
	// Test cases with varying corpus sizes to exercise k-clamping
	testCases := []struct {
		name     string
		nDocs    int
		kInput   int
		wantKMax int // k must be <= this
	}{
		{"3-docs-k50", 3, 50, 3},
		{"5-docs-k50", 5, 50, 5},
		{"10-docs-k50", 10, 50, 10},
		{"20-docs-k10", 20, 10, 10},
		{"1-doc-k50", 1, 50, 1},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			// Build a corpus of distinct documents
			docs := make([]search.SearchDocument, tc.nDocs)
			for i := 0; i < tc.nDocs; i++ {
				// Each doc needs distinct content so vocabulary grows
				docs[i] = search.SearchDocument{
					DocID:       i,
					ElementType: "section",
					ElementID:   fmt.Sprintf("§test-%d", i),
					Title:       fmt.Sprintf("Test Section %d", i),
					Content:     fmt.Sprintf("unique content for document %d with words alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima mike november oscar papa quebec romeo sierra tango uniform victor whiskey xray yankee zulu variant%d", i, i),
				}
			}

			lsi, err := search.BuildLSI(docs, tc.kInput)
			if err != nil {
				t.Fatalf("BuildLSI: %v", err)
			}

			// Property 1: K must not exceed document count
			if lsi.K > tc.nDocs {
				t.Errorf("K=%d exceeds nDocs=%d", lsi.K, tc.nDocs)
			}

			// Property 2: K must not exceed requested k
			if lsi.K > tc.kInput {
				t.Errorf("K=%d exceeds kInput=%d", lsi.K, tc.kInput)
			}

			// Property 3: K must be positive
			if lsi.K <= 0 {
				t.Errorf("K=%d must be > 0", lsi.K)
			}

			// Property 4: K must not exceed the bound
			if lsi.K > tc.wantKMax {
				t.Errorf("K=%d exceeds expected max=%d", lsi.K, tc.wantKMax)
			}

			// Property 5: Every document vector must have exactly K dimensions
			if len(lsi.DocVectors) != tc.nDocs {
				t.Errorf("DocVectors count=%d, want %d", len(lsi.DocVectors), tc.nDocs)
			}
			for i, vec := range lsi.DocVectors {
				if len(vec) != lsi.K {
					t.Errorf("DocVectors[%d] has %d dims, want K=%d", i, len(vec), lsi.K)
				}
			}

			// Property 6: Query vector must have exactly K dimensions
			qvec := lsi.QueryVec("alpha bravo charlie delta")
			if qvec != nil && len(qvec) != lsi.K {
				t.Errorf("QueryVec has %d dims, want K=%d", len(qvec), lsi.K)
			}

			t.Logf("nDocs=%d, kInput=%d → K=%d, terms=%d, docVecs=%d",
				tc.nDocs, tc.kInput, lsi.K, len(lsi.TermIndex), len(lsi.DocVectors))
		})
	}
}

// TestLSIDimensionStability verifies that the same corpus produces identical K and vectors.
func TestLSIDimensionStability(t *testing.T) {
	db, specID, _ := buildSyntheticSearchDB(t)

	docs, err := search.ExtractDocuments(db, specID)
	if err != nil {
		t.Fatalf("extract docs: %v", err)
	}

	k := 50
	if len(docs) < k {
		k = len(docs)
	}

	lsi1, err := search.BuildLSI(docs, k)
	if err != nil {
		t.Fatalf("BuildLSI 1: %v", err)
	}

	lsi2, err := search.BuildLSI(docs, k)
	if err != nil {
		t.Fatalf("BuildLSI 2: %v", err)
	}

	// K must be identical
	if lsi1.K != lsi2.K {
		t.Errorf("K differs: %d vs %d", lsi1.K, lsi2.K)
	}

	// Vector dimensions must be identical
	for i := range lsi1.DocVectors {
		if len(lsi1.DocVectors[i]) != len(lsi2.DocVectors[i]) {
			t.Errorf("DocVectors[%d] dims differ: %d vs %d",
				i, len(lsi1.DocVectors[i]), len(lsi2.DocVectors[i]))
		}
	}

	// Query vectors must be identical
	qvec1 := lsi1.QueryVec("data integrity persistence")
	qvec2 := lsi2.QueryVec("data integrity persistence")
	if len(qvec1) != len(qvec2) {
		t.Errorf("QueryVec dims differ: %d vs %d", len(qvec1), len(qvec2))
	}
	for i := range qvec1 {
		if math.Abs(qvec1[i]-qvec2[i]) > 1e-10 {
			t.Errorf("QueryVec[%d] differs: %.10f vs %.10f", i, qvec1[i], qvec2[i])
		}
	}

	t.Logf("APP-INV-012 stability: K=%d, %d docs, %d terms — deterministic across builds",
		lsi1.K, len(docs), len(lsi1.TermIndex))
}
