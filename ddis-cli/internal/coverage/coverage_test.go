package coverage

import (
	"testing"

	"github.com/wvandaal/ddis/internal/storage"
)

// setupTestDB creates an in-memory SQLite DB with a minimal spec, source file,
// section, invariants, and ADRs for testing coverage analysis.
func setupTestDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	db, err := storage.Open(":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := storage.InsertSpecIndex(db, &storage.SpecIndex{
		SpecPath:    "/test/spec.md",
		SpecName:    "test-spec",
		DDISVersion: "3.0",
		TotalLines:  100,
		ContentHash: "abc123",
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
		ContentHash: "sf123",
		LineCount:   100,
		RawText:     "test content",
	})
	if err != nil {
		t.Fatalf("insert source file: %v", err)
	}

	secID, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "§1",
		Title:        "Test Section",
		HeadingLevel: 1,
		LineStart:    1,
		LineEnd:      50,
		RawText:      "section content",
		ContentHash:  "sec123",
	})
	if err != nil {
		t.Fatalf("insert section: %v", err)
	}

	// Insert a complete invariant (all 5 components present)
	_, err = storage.InsertInvariant(db, &storage.Invariant{
		SpecID:            specID,
		SourceFileID:      sfID,
		SectionID:         secID,
		InvariantID:       "INV-001",
		Title:             "Completeness invariant",
		Statement:         "Every specification must be complete and self-consistent.",
		SemiFormal:        "forall s in Spec: complete(s) AND consistent(s)",
		ViolationScenario: "A specification that references undefined elements violates this invariant.",
		ValidationMethod:  "Check all cross-references resolve to defined elements in the spec index.",
		WhyThisMatters:    "Incomplete specs lead to ambiguous implementations and drift.",
		LineStart:         10,
		LineEnd:           20,
		RawText:           "**INV-001:** Completeness invariant",
		ContentHash:       "inv001",
	})
	if err != nil {
		t.Fatalf("insert invariant INV-001: %v", err)
	}

	// Insert an incomplete invariant (missing semi_formal, violation, why)
	_, err = storage.InsertInvariant(db, &storage.Invariant{
		SpecID:           specID,
		SourceFileID:     sfID,
		SectionID:        secID,
		InvariantID:      "INV-002",
		Title:            "Partial invariant",
		Statement:        "Each module shall be independently parseable.",
		ValidationMethod: "Parse each module file in isolation and verify zero errors.",
		LineStart:        21,
		LineEnd:          30,
		RawText:          "**INV-002:** Partial invariant",
		ContentHash:      "inv002",
	})
	if err != nil {
		t.Fatalf("insert invariant INV-002: %v", err)
	}

	// Insert a complete ADR
	_, err = storage.InsertADR(db, &storage.ADR{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionID:    secID,
		ADRID:        "ADR-001",
		Title:        "Use SQLite for storage",
		Problem:      "We need a portable single-file database for spec indexes.",
		DecisionText: "Use SQLite via modernc.org/sqlite for zero-CGO portability.",
		ChosenOption: "SQLite",
		Consequences: "Single binary distribution, ACID transactions, FTS5 support.",
		Tests:        "Open :memory: database, apply schema, insert and query data.",
		Status:       "active",
		LineStart:    31,
		LineEnd:      45,
		RawText:      "**ADR-001:** Use SQLite for storage",
		ContentHash:  "adr001",
	})
	if err != nil {
		t.Fatalf("insert ADR: %v", err)
	}

	return &db, specID
}

// TestAnalyzeBasic verifies that Analyze returns correct summary counts and score.
func TestAnalyzeBasic(t *testing.T) {
	dbPtr, specID := setupTestDB(t)
	db := *dbPtr

	result, err := Analyze(db, specID, Options{})
	if err != nil {
		t.Fatalf("Analyze: %v", err)
	}

	// Should find 2 invariants and 1 ADR
	if result.Summary.InvariantsTotal != 2 {
		t.Errorf("InvariantsTotal = %d, want 2", result.Summary.InvariantsTotal)
	}
	if result.Summary.ADRsTotal != 1 {
		t.Errorf("ADRsTotal = %d, want 1", result.Summary.ADRsTotal)
	}

	// Spec name should match
	if result.Spec != "test-spec" {
		t.Errorf("Spec = %q, want %q", result.Spec, "test-spec")
	}

	// Score should be between 0 and 1
	if result.Summary.Score < 0 || result.Summary.Score > 1 {
		t.Errorf("Score = %f, want 0 <= score <= 1", result.Summary.Score)
	}

	// INV-001 should have higher completeness than INV-002 (it has all fields)
	inv1, ok := result.Invariants["INV-001"]
	if !ok {
		t.Fatal("INV-001 not found in result")
	}
	inv2, ok := result.Invariants["INV-002"]
	if !ok {
		t.Fatal("INV-002 not found in result")
	}
	if inv1.Completeness <= inv2.Completeness {
		t.Errorf("INV-001 completeness (%f) should be > INV-002 completeness (%f)",
			inv1.Completeness, inv2.Completeness)
	}
}

// TestAnalyzeGaps verifies that gaps are detected for incomplete components.
func TestAnalyzeGaps(t *testing.T) {
	dbPtr, specID := setupTestDB(t)
	db := *dbPtr

	result, err := Analyze(db, specID, Options{})
	if err != nil {
		t.Fatalf("Analyze: %v", err)
	}

	// INV-002 is missing semi_formal, violation_scenario, and why_this_matters
	// so there should be at least some gaps reported
	if len(result.Gaps) == 0 {
		t.Error("expected non-empty gaps for incomplete invariant")
	}

	// Check that at least one gap mentions INV-002
	found := false
	for _, g := range result.Gaps {
		if len(g) > 7 && g[:7] == "INV-002" {
			found = true
			break
		}
	}
	if !found {
		t.Error("expected at least one gap for INV-002")
	}
}

// TestAnalyzeEmptySpec verifies Analyze handles an empty spec gracefully.
func TestAnalyzeEmptySpec(t *testing.T) {
	db, err := storage.Open(":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := storage.InsertSpecIndex(db, &storage.SpecIndex{
		SpecPath:    "/test/empty.md",
		SpecName:    "empty-spec",
		TotalLines:  0,
		ContentHash: "empty",
		ParsedAt:    "2026-01-01T00:00:00Z",
		SourceType:  "monolith",
	})
	if err != nil {
		t.Fatalf("insert spec: %v", err)
	}

	result, err := Analyze(db, specID, Options{})
	if err != nil {
		t.Fatalf("Analyze: %v", err)
	}

	if result.Summary.InvariantsTotal != 0 {
		t.Errorf("InvariantsTotal = %d, want 0", result.Summary.InvariantsTotal)
	}
	if result.Summary.ADRsTotal != 0 {
		t.Errorf("ADRsTotal = %d, want 0", result.Summary.ADRsTotal)
	}
	if result.Summary.Score != 0 {
		t.Errorf("Score = %f, want 0", result.Summary.Score)
	}
	if len(result.Gaps) != 0 {
		t.Errorf("Gaps = %d, want 0", len(result.Gaps))
	}
}
