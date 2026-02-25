package drift

import (
	"testing"

	"github.com/wvandaal/ddis/internal/storage"
)

// setupDriftDB creates an in-memory SQLite DB with a spec that has known
// drift characteristics for testing classification and score calculation.
func setupDriftDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	db, err := storage.Open(":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := storage.InsertSpecIndex(db, &storage.SpecIndex{
		SpecPath:    "/test/spec.md",
		SpecName:    "drift-test-spec",
		DDISVersion: "3.0",
		TotalLines:  200,
		ContentHash: "drift123",
		ParsedAt:    "2026-01-01T00:00:00Z",
		SourceType:  "modular",
	})
	if err != nil {
		t.Fatalf("insert spec: %v", err)
	}

	sfID, err := storage.InsertSourceFile(db, &storage.SourceFile{
		SpecID:      specID,
		FilePath:    "/test/module.md",
		FileRole:    "module",
		ModuleName:  "core",
		ContentHash: "sf123",
		LineCount:   200,
		RawText:     "module content",
	})
	if err != nil {
		t.Fatalf("insert source file: %v", err)
	}

	secID, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "§1",
		Title:        "Core Module",
		HeadingLevel: 1,
		LineStart:    1,
		LineEnd:      100,
		RawText:      "core module content",
		ContentHash:  "sec123",
	})
	if err != nil {
		t.Fatalf("insert section: %v", err)
	}

	// Insert a module
	modID, err := storage.InsertModule(db, &storage.Module{
		SpecID:       specID,
		SourceFileID: sfID,
		ModuleName:   "core",
		Domain:       "foundation",
		LineCount:    200,
	})
	if err != nil {
		t.Fatalf("insert module: %v", err)
	}

	// Insert invariants (these are fully defined)
	_, err = storage.InsertInvariant(db, &storage.Invariant{
		SpecID:            specID,
		SourceFileID:      sfID,
		SectionID:         secID,
		InvariantID:       "INV-001",
		Title:             "Defined invariant",
		Statement:         "Must be complete.",
		SemiFormal:        "forall x: complete(x)",
		ViolationScenario: "Incomplete spec found.",
		ValidationMethod:  "Check completeness.",
		WhyThisMatters:    "Prevents drift.",
		LineStart:         10,
		LineEnd:           20,
		RawText:           "INV-001 content",
		ContentHash:       "inv001",
	})
	if err != nil {
		t.Fatalf("insert INV-001: %v", err)
	}

	// Registry has INV-001 (matched) and INV-003 (unimplemented — no definition)
	_, err = storage.InsertInvariantRegistryEntry(db, &storage.InvariantRegistryEntry{
		SpecID:      specID,
		InvariantID: "INV-001",
		Owner:       "core",
		Domain:      "foundation",
		Description: "Completeness",
	})
	if err != nil {
		t.Fatalf("insert registry INV-001: %v", err)
	}

	_, err = storage.InsertInvariantRegistryEntry(db, &storage.InvariantRegistryEntry{
		SpecID:      specID,
		InvariantID: "INV-003",
		Owner:       "core",
		Domain:      "foundation",
		Description: "An unimplemented invariant",
	})
	if err != nil {
		t.Fatalf("insert registry INV-003: %v", err)
	}

	// Module "maintains" INV-001 (in registry) and INV-099 (NOT in registry = unspecified)
	_, err = storage.InsertModuleRelationship(db, &storage.ModuleRelationship{
		ModuleID: modID,
		RelType:  "maintains",
		Target:   "INV-001",
	})
	if err != nil {
		t.Fatalf("insert rel INV-001: %v", err)
	}

	_, err = storage.InsertModuleRelationship(db, &storage.ModuleRelationship{
		ModuleID: modID,
		RelType:  "maintains",
		Target:   "INV-099",
	})
	if err != nil {
		t.Fatalf("insert rel INV-099: %v", err)
	}

	return &db, specID
}

// TestClassifyDirection verifies the direction classification logic with
// synthetic DriftReport values (unit test, no DB needed).
func TestClassifyDirection(t *testing.T) {
	tests := []struct {
		name           string
		unspecified    int
		unimplemented  int
		contradictions int
		want           string
	}{
		{"aligned", 0, 0, 0, "aligned"},
		{"impl-ahead", 3, 0, 0, "impl-ahead"},
		{"spec-ahead", 0, 2, 0, "spec-ahead"},
		{"contradictory", 1, 1, 1, "contradictory"},
		{"mutual", 2, 3, 0, "mutual"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			report := &DriftReport{
				ImplDrift: ImplDrift{
					Unspecified:    tt.unspecified,
					Unimplemented:  tt.unimplemented,
					Contradictions: tt.contradictions,
				},
			}
			c := Classify(report)
			if c.Direction != tt.want {
				t.Errorf("direction = %q, want %q", c.Direction, tt.want)
			}
		})
	}
}

// TestDriftScoreFormula verifies the drift total formula:
// total = unspecified + unimplemented + 2*contradictions
func TestDriftScoreFormula(t *testing.T) {
	dbPtr, specID := setupDriftDB(t)
	db := *dbPtr

	report, err := Analyze(db, specID, Options{})
	if err != nil {
		t.Fatalf("Analyze: %v", err)
	}

	// Verify the total formula
	expectedTotal := report.ImplDrift.Unspecified + report.ImplDrift.Unimplemented + 2*report.ImplDrift.Contradictions
	if report.ImplDrift.Total != expectedTotal {
		t.Errorf("ImplDrift.Total = %d, want %d (unspecified=%d + unimplemented=%d + 2*contradictions=%d)",
			report.ImplDrift.Total, expectedTotal,
			report.ImplDrift.Unspecified, report.ImplDrift.Unimplemented, report.ImplDrift.Contradictions)
	}

	// Our test DB has INV-003 in registry but not defined (unimplemented=1)
	// and INV-099 maintained but not in registry (unspecified=1)
	if report.ImplDrift.Unimplemented < 1 {
		t.Errorf("expected at least 1 unimplemented (INV-003), got %d", report.ImplDrift.Unimplemented)
	}
	if report.ImplDrift.Unspecified < 1 {
		t.Errorf("expected at least 1 unspecified (INV-099), got %d", report.ImplDrift.Unspecified)
	}

	// Quality breakdown should be consistent
	q := report.QualityBreakdown
	if q.Correctness != report.ImplDrift.Unimplemented+report.ImplDrift.Contradictions {
		t.Errorf("Correctness = %d, want unimplemented(%d)+contradictions(%d)",
			q.Correctness, report.ImplDrift.Unimplemented, report.ImplDrift.Contradictions)
	}
	if q.Depth != report.ImplDrift.Unspecified {
		t.Errorf("Depth = %d, want unspecified(%d)", q.Depth, report.ImplDrift.Unspecified)
	}

	// Effective drift >= 0
	if report.EffectiveDrift < 0 {
		t.Errorf("EffectiveDrift = %d, want >= 0", report.EffectiveDrift)
	}
}

// TestAnalyzeWithMutualDrift verifies the full Analyze pipeline detects
// both unimplemented and unspecified drift using a populated database,
// and checks classification is "mutual".
func TestAnalyzeWithMutualDrift(t *testing.T) {
	dbPtr, specID := setupDriftDB(t)
	db := *dbPtr

	report, err := Analyze(db, specID, Options{})
	if err != nil {
		t.Fatalf("Analyze: %v", err)
	}

	// With both unimplemented (INV-003) and unspecified (INV-099), direction should be "mutual"
	if report.Classification.Direction != "mutual" {
		t.Errorf("Classification.Direction = %q, want %q", report.Classification.Direction, "mutual")
	}

	// No contradictions, so severity should be "additive"
	if report.Classification.Severity != "additive" {
		t.Errorf("Classification.Severity = %q, want %q", report.Classification.Severity, "additive")
	}

	// No planned divergences stored, so intentionality should be "organic"
	if report.Classification.Intentionality != "organic" {
		t.Errorf("Classification.Intentionality = %q, want %q", report.Classification.Intentionality, "organic")
	}

	// Details should be non-nil and contain both types
	if report.ImplDrift.Details == nil {
		t.Fatal("ImplDrift.Details should be non-nil")
	}

	foundUnimplemented := false
	foundUnspecified := false
	for _, d := range report.ImplDrift.Details {
		if d.Type == "unimplemented" && d.Element == "INV-003" {
			foundUnimplemented = true
		}
		if d.Type == "unspecified" && d.Element == "INV-099" {
			foundUnspecified = true
		}
	}
	if !foundUnimplemented {
		t.Error("expected detail for unimplemented INV-003")
	}
	if !foundUnspecified {
		t.Error("expected detail for unspecified INV-099")
	}
}
