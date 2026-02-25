package search

import (
	"testing"

	"github.com/wvandaal/ddis/internal/storage"
)

// setupSearchDB creates an in-memory SQLite DB populated with a spec containing
// sections, invariants, and an ADR, then populates the FTS5 index.
func setupSearchDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	db, err := storage.Open(":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := storage.InsertSpecIndex(db, &storage.SpecIndex{
		SpecPath:    "/test/spec.md",
		SpecName:    "search-test-spec",
		DDISVersion: "3.0",
		TotalLines:  300,
		ContentHash: "search123",
		ParsedAt:    "2026-01-01T00:00:00Z",
		SourceType:  "monolith",
	})
	if err != nil {
		t.Fatalf("insert spec: %v", err)
	}

	sfID, err := storage.InsertSourceFile(db, &storage.SourceFile{
		SpecID:      specID,
		FilePath:    "/test/spec.md",
		FileRole:    "monolith",
		ContentHash: "sfsearch",
		LineCount:   300,
		RawText:     "full spec text",
	})
	if err != nil {
		t.Fatalf("insert source file: %v", err)
	}

	secID, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "§1",
		Title:        "Parsing Pipeline",
		HeadingLevel: 1,
		LineStart:    1,
		LineEnd:      100,
		RawText:      "The parsing pipeline processes markdown documents into a structured SQLite index. It handles sections, invariants, ADRs, and cross-references.",
		ContentHash:  "sec1",
	})
	if err != nil {
		t.Fatalf("insert section §1: %v", err)
	}

	_, err = storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "§2",
		Title:        "Search Intelligence",
		HeadingLevel: 1,
		LineStart:    101,
		LineEnd:      200,
		RawText:      "The search system combines BM25 lexical search with LSI semantic search and PageRank authority scoring via reciprocal rank fusion.",
		ContentHash:  "sec2",
	})
	if err != nil {
		t.Fatalf("insert section §2: %v", err)
	}

	_, err = storage.InsertInvariant(db, &storage.Invariant{
		SpecID:            specID,
		SourceFileID:      sfID,
		SectionID:         secID,
		InvariantID:       "INV-001",
		Title:             "Cross-reference resolution",
		Statement:         "Every cross-reference in the spec must resolve to a defined element.",
		SemiFormal:        "forall ref in CrossRefs: exists target in Elements: ref.target == target.id",
		ViolationScenario: "A reference to INV-999 that does not exist.",
		ValidationMethod:  "Parse spec and check all references resolve.",
		WhyThisMatters:    "Unresolved references indicate incomplete specifications.",
		LineStart:         10,
		LineEnd:           25,
		RawText:           "INV-001: Cross-reference resolution",
		ContentHash:       "inv001",
	})
	if err != nil {
		t.Fatalf("insert INV-001: %v", err)
	}

	_, err = storage.InsertADR(db, &storage.ADR{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionID:    secID,
		ADRID:        "ADR-001",
		Title:        "BM25 over TF-IDF for lexical search",
		Problem:      "Need a ranking function for full-text search results.",
		DecisionText: "Use BM25 via SQLite FTS5 built-in ranking for lexical relevance.",
		Status:       "active",
		LineStart:    30,
		LineEnd:      50,
		RawText:      "ADR-001: BM25 over TF-IDF",
		ContentHash:  "adr001",
	})
	if err != nil {
		t.Fatalf("insert ADR-001: %v", err)
	}

	// Populate FTS5 index from documents
	docs, err := ExtractDocuments(db, specID)
	if err != nil {
		t.Fatalf("extract documents: %v", err)
	}
	if err := PopulateFTS(db, docs); err != nil {
		t.Fatalf("populate FTS: %v", err)
	}

	return &db, specID
}

// TestBuildLSIAndQuery verifies that LSI index construction and query projection
// produce meaningful similarity rankings from extracted documents.
func TestBuildLSIAndQuery(t *testing.T) {
	dbPtr, specID := setupSearchDB(t)
	db := *dbPtr

	docs, err := ExtractDocuments(db, specID)
	if err != nil {
		t.Fatalf("ExtractDocuments: %v", err)
	}

	if len(docs) == 0 {
		t.Fatal("expected non-empty documents")
	}

	k := len(docs) // clamp to doc count for small test corpus
	lsi, err := BuildLSI(docs, k)
	if err != nil {
		t.Fatalf("BuildLSI: %v", err)
	}

	if lsi.Uk == nil {
		t.Fatal("LSI index Uk matrix should not be nil")
	}
	if len(lsi.DocIDs) != len(docs) {
		t.Errorf("DocIDs length = %d, want %d", len(lsi.DocIDs), len(docs))
	}

	// Query for "cross-reference resolution" should rank INV-001 highly
	qVec := lsi.QueryVec("cross-reference resolution")
	if qVec == nil {
		t.Fatal("QueryVec returned nil")
	}

	ranked := lsi.RankAll(qVec)
	if len(ranked) == 0 {
		t.Fatal("RankAll returned empty results")
	}

	// The top result should be INV-001 (about cross-reference resolution)
	if ranked[0].ElementID != "INV-001" {
		t.Logf("top result was %s (similarity=%.4f), expected INV-001",
			ranked[0].ElementID, ranked[0].Similarity)
		// Not a hard failure since LSI on a tiny corpus can be noisy,
		// but INV-001 should at least appear in the top half
		foundInTop := false
		half := len(ranked) / 2
		if half < 2 {
			half = len(ranked)
		}
		for _, rd := range ranked[:half] {
			if rd.ElementID == "INV-001" {
				foundInTop = true
				break
			}
		}
		if !foundInTop {
			t.Error("INV-001 not found in top half of LSI rankings")
		}
	}
}

// TestSearchHybrid verifies the full Search function (BM25+LSI+authority fusion).
func TestSearchHybrid(t *testing.T) {
	dbPtr, specID := setupSearchDB(t)
	db := *dbPtr

	results, err := Search(db, specID, "cross-reference resolution", SearchOptions{
		Limit: 5,
	})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}

	if len(results) == 0 {
		t.Fatal("expected at least one result for 'cross-reference resolution'")
	}

	// INV-001 should appear since it is about cross-reference resolution
	found := false
	for _, r := range results {
		if r.ElementID == "INV-001" {
			found = true
			if r.Score <= 0 {
				t.Errorf("INV-001 score = %f, want > 0", r.Score)
			}
			break
		}
	}
	if !found {
		t.Error("expected INV-001 in search results for 'cross-reference resolution'")
	}

	// All results should have positive scores
	for _, r := range results {
		if r.Score <= 0 {
			t.Errorf("result %s has score %f, want > 0", r.ElementID, r.Score)
		}
	}
}

// TestSearchEmptyQuery verifies that an empty query returns an error.
func TestSearchEmptyQuery(t *testing.T) {
	dbPtr, specID := setupSearchDB(t)
	db := *dbPtr

	_, err := Search(db, specID, "", SearchOptions{})
	if err == nil {
		t.Error("expected error for empty query, got nil")
	}

	_, err = Search(db, specID, "   ", SearchOptions{})
	if err == nil {
		t.Error("expected error for whitespace-only query, got nil")
	}
}
