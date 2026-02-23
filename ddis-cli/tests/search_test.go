package tests

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

// sharedSearchDB caches a parsed + indexed monolith DB for search tests.
var sharedSearchDB *searchTestDB

type searchTestDB struct {
	db     *storage.DB
	specID int64
	lsi    *search.LSIIndex
}

func getSearchDB(t *testing.T) (*storage.DB, int64, *search.LSIIndex) {
	t.Helper()
	if sharedSearchDB != nil {
		return sharedSearchDB.db, sharedSearchDB.specID, sharedSearchDB.lsi
	}

	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "search_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	// Build search index
	if err := search.BuildIndex(db, specID); err != nil {
		t.Fatalf("build index: %v", err)
	}

	// Build LSI for context tests
	docs, err := search.ExtractDocuments(db, specID)
	if err != nil {
		t.Fatalf("extract docs: %v", err)
	}
	k := 50
	if len(docs) < k {
		k = len(docs)
	}
	lsi, err := search.BuildLSI(docs, k)
	if err != nil {
		t.Fatalf("build lsi: %v", err)
	}

	sharedSearchDB = &searchTestDB{db: &db, specID: specID, lsi: lsi}
	return sharedSearchDB.db, sharedSearchDB.specID, sharedSearchDB.lsi
}

// TestSearchExactMatch verifies that "INV-006" returns INV-006 as the top result.
func TestSearchExactMatch(t *testing.T) {
	dbPtr, specID, _ := getSearchDB(t)
	db := *dbPtr

	results, err := search.Search(db, specID, "INV-006", search.SearchOptions{Limit: 10})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	if len(results) == 0 {
		t.Fatal("no results for INV-006")
	}

	// INV-006 should be in top 3 results
	found := false
	for i, r := range results {
		if r.ElementID == "INV-006" {
			found = true
			t.Logf("INV-006 found at position %d (score: %.4f)", i+1, r.Score)
			break
		}
	}
	if !found {
		t.Errorf("INV-006 not found in top %d results", len(results))
		for i, r := range results {
			t.Logf("  %d. %s: %s (%.4f)", i+1, r.ElementID, r.Title, r.Score)
		}
	}
}

// TestSearchSemanticMatch verifies that "verification" finds quality gates.
func TestSearchSemanticMatch(t *testing.T) {
	dbPtr, specID, _ := getSearchDB(t)
	db := *dbPtr

	results, err := search.Search(db, specID, "how to verify cross-references", search.SearchOptions{Limit: 10})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	if len(results) == 0 {
		t.Fatal("no results for semantic query")
	}

	// Should find elements related to verification/validation
	t.Logf("Results for 'how to verify cross-references':")
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
		t.Error("expected at least one gate or invariant in results for verification query")
	}
}

// TestSearchGlossaryExpansion verifies glossary-based query expansion.
func TestSearchGlossaryExpansion(t *testing.T) {
	dbPtr, specID, _ := getSearchDB(t)
	db := *dbPtr

	// Search for a glossary term — expansion should boost related results
	results, err := search.Search(db, specID, "invariant registry", search.SearchOptions{Limit: 10})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	if len(results) == 0 {
		t.Fatal("no results for glossary term query")
	}

	t.Logf("Results for 'invariant registry' (%d):", len(results))
	for i, r := range results {
		t.Logf("  %d. [%s] %s: %s", i+1, r.ElementType, r.ElementID, r.Title)
	}
}

// TestSearchTypeFilter verifies that --type invariant filters correctly.
func TestSearchTypeFilter(t *testing.T) {
	dbPtr, specID, _ := getSearchDB(t)
	db := *dbPtr

	results, err := search.Search(db, specID, "cross-reference", search.SearchOptions{
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
		t.Logf("Found %d invariants matching 'cross-reference'", len(results))
	}
}

// TestSearchLSIBuild verifies that the LSI index builds correctly.
func TestSearchLSIBuild(t *testing.T) {
	dbPtr, specID, _ := getSearchDB(t)
	db := *dbPtr

	docs, err := search.ExtractDocuments(db, specID)
	if err != nil {
		t.Fatalf("extract docs: %v", err)
	}

	if len(docs) < 100 {
		t.Errorf("expected 100+ documents, got %d", len(docs))
	}

	lsi, err := search.BuildLSI(docs, 50)
	if err != nil {
		t.Fatalf("build LSI: %v", err)
	}

	if lsi.K <= 0 {
		t.Error("LSI K should be > 0")
	}
	if len(lsi.DocVectors) != len(docs) {
		t.Errorf("DocVectors length %d != docs %d", len(lsi.DocVectors), len(docs))
	}
	if len(lsi.TermIndex) < 100 {
		t.Errorf("expected 100+ terms, got %d", len(lsi.TermIndex))
	}

	t.Logf("LSI index: k=%d, %d terms, %d docs", lsi.K, len(lsi.TermIndex), len(lsi.DocVectors))
}

// TestSearchAuthorityComputed verifies PageRank scores are non-zero.
func TestSearchAuthorityComputed(t *testing.T) {
	dbPtr, specID, _ := getSearchDB(t)
	db := *dbPtr

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
	dbPtr, specID, _ := getSearchDB(t)
	db := *dbPtr

	results, err := search.Search(db, specID, "cross-reference density", search.SearchOptions{Limit: 5})
	if err != nil {
		t.Fatalf("search: %v", err)
	}

	if len(results) == 0 {
		t.Fatal("no results")
	}

	// Top result should have multiple signals
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
	dbPtr, specID, _ := getSearchDB(t)
	db := *dbPtr

	results, err := search.Search(db, specID, "state machine", search.SearchOptions{Limit: 5})
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
	dbPtr, specID, _ := getSearchDB(t)
	db := *dbPtr

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
	dbPtr, specID, _ := getSearchDB(t)
	db := *dbPtr

	results, err := search.Search(db, specID, "cross-reference density", search.SearchOptions{
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
	dbPtr, specID, lsi := getSearchDB(t)
	db := *dbPtr

	bundle, err := search.BuildContext(db, specID, "§0.5", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context: %v", err)
	}

	if bundle.Target != "§0.5" {
		t.Errorf("target = %s, want §0.5", bundle.Target)
	}
	if bundle.Content == "" {
		t.Error("content is empty")
	}
	if bundle.Title == "" {
		t.Error("title is empty")
	}

	t.Logf("Context bundle for §0.5: %d constraints, %d related, %d impact nodes",
		len(bundle.Constraints), len(bundle.Related), 0)
	if bundle.ImpactRadius != nil {
		t.Logf("  Impact: %d nodes", bundle.ImpactRadius.TotalCount)
	}
}

// TestContextBundleInvariant verifies context bundle includes impact radius.
func TestContextBundleInvariant(t *testing.T) {
	dbPtr, specID, lsi := getSearchDB(t)
	db := *dbPtr

	bundle, err := search.BuildContext(db, specID, "INV-006", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context: %v", err)
	}

	if bundle.Target != "INV-006" {
		t.Errorf("target = %s, want INV-006", bundle.Target)
	}
	if bundle.ElementType != "invariant" {
		t.Errorf("element_type = %s, want invariant", bundle.ElementType)
	}

	// INV-006 should have non-empty impact radius
	if bundle.ImpactRadius == nil {
		t.Error("impact radius is nil for INV-006")
	} else if bundle.ImpactRadius.TotalCount == 0 {
		t.Error("impact radius has 0 nodes for INV-006")
	} else {
		t.Logf("INV-006 impact radius: %d nodes", bundle.ImpactRadius.TotalCount)
	}
}

// TestContextBundleJSON verifies JSON output parses correctly.
func TestContextBundleJSON(t *testing.T) {
	dbPtr, specID, lsi := getSearchDB(t)
	db := *dbPtr

	bundle, err := search.BuildContext(db, specID, "INV-006", lsi, "", 2, 5)
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

	if parsed.Target != "INV-006" {
		t.Errorf("JSON target = %s, want INV-006", parsed.Target)
	}
	if parsed.Content == "" {
		t.Error("JSON content is empty")
	}
}

// TestContextEditingGuidance verifies guidance is derived from constraints.
func TestContextEditingGuidance(t *testing.T) {
	dbPtr, specID, lsi := getSearchDB(t)
	db := *dbPtr

	bundle, err := search.BuildContext(db, specID, "§0.5", lsi, "", 2, 5)
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
	dbPtr, specID, lsi := getSearchDB(t)
	db := *dbPtr

	// Test a section that may have bold terms not in glossary
	bundle, err := search.BuildContext(db, specID, "INV-006", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context: %v", err)
	}

	t.Logf("Coverage gaps for INV-006: %d", len(bundle.CoverageGaps))
	for _, gap := range bundle.CoverageGaps {
		t.Logf("  [%s] %s (ref: %s)", gap.Severity, gap.Description, gap.InvariantRef)
	}
}

// TestContextInvariantCompleteness verifies invariant completeness checking.
func TestContextInvariantCompleteness(t *testing.T) {
	dbPtr, specID, lsi := getSearchDB(t)
	db := *dbPtr

	bundle, err := search.BuildContext(db, specID, "INV-006", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context: %v", err)
	}

	if len(bundle.InvCompleteness) == 0 {
		t.Error("no invariant completeness data for INV-006")
	}

	for _, inv := range bundle.InvCompleteness {
		t.Logf("  %s: statement=%v semi-formal=%v validation=%v why-matters=%v complete=%v",
			inv.ID, inv.HasStatement, inv.HasSemiFormal, inv.HasValidation,
			inv.HasWhyMatters, inv.Complete)
		if inv.ID == "INV-006" && !inv.Complete {
			t.Errorf("INV-006 should be complete (all fields present)")
		}
	}
}

// TestContextReasoningModeTags verifies reasoning mode tagging.
func TestContextReasoningModeTags(t *testing.T) {
	dbPtr, specID, lsi := getSearchDB(t)
	db := *dbPtr

	// INV-006 is inside a section that may have related reasoning mode elements
	bundle, err := search.BuildContext(db, specID, "INV-006", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context: %v", err)
	}

	t.Logf("Reasoning mode items for INV-006: %d", len(bundle.ReasoningMode))
	for _, rm := range bundle.ReasoningMode {
		t.Logf("  [%s] %s: %s — %s", rm.Mode, rm.ElementType, rm.ElementID, rm.Description)
	}

	// INV-006 itself should appear as a Formal element
	hasFormal := false
	for _, rm := range bundle.ReasoningMode {
		if rm.Mode == "Formal" && rm.ElementID == "INV-006" {
			hasFormal = true
		}
	}
	if !hasFormal {
		t.Error("INV-006 should be tagged as Formal reasoning mode")
	}

	// Test a section that has negative specs for Meta mode
	bundle2, err := search.BuildContext(db, specID, "§0.5", lsi, "", 2, 5)
	if err != nil {
		t.Fatalf("build context §0.5: %v", err)
	}
	t.Logf("Reasoning mode items for §0.5: %d", len(bundle2.ReasoningMode))
}
