package tests

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/validator"
)

// setupTraceDB creates a minimal spec index with the given invariants for traceability testing.
func setupTraceDB(t *testing.T, invariants []storage.Invariant) (*storage.DB, int64) {
	t.Helper()

	dbPath := filepath.Join(t.TempDir(), "trace_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := storage.InsertSpecIndex(db, &storage.SpecIndex{
		SpecPath:    "test_spec.md",
		TotalLines:  100,
		ContentHash: "abc123",
		ParsedAt:    "2026-01-01T00:00:00Z",
		SourceType:  "monolith",
	})
	if err != nil {
		t.Fatalf("insert spec_index: %v", err)
	}

	sfID, err := storage.InsertSourceFile(db, &storage.SourceFile{
		SpecID:      specID,
		FilePath:    "test_spec.md",
		FileRole:    "monolith",
		ContentHash: "abc123",
		LineCount:   100,
		RawText:     "test content",
	})
	if err != nil {
		t.Fatalf("insert source_file: %v", err)
	}

	secID, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "§1",
		Title:        "Test Section",
		HeadingLevel: 1,
		LineStart:    1,
		LineEnd:      100,
		RawText:      "test",
		ContentHash:  "sec123",
	})
	if err != nil {
		t.Fatalf("insert section: %v", err)
	}

	for i := range invariants {
		invariants[i].SpecID = specID
		invariants[i].SourceFileID = sfID
		invariants[i].SectionID = secID
		if _, err := storage.InsertInvariant(db, &invariants[i]); err != nil {
			t.Fatalf("insert invariant %s: %v", invariants[i].InvariantID, err)
		}
	}

	return &db, specID
}

func TestTraceabilitySkippedWithoutCodeRoot(t *testing.T) {
	dbPtr, specID := setupTraceDB(t, nil)
	db := *dbPtr

	// No CodeRoot = not applicable, so check 13 should be skipped entirely
	report, err := validator.Validate(db, specID, validator.ValidateOptions{
		CheckIDs: []int{13},
	})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	// Check 13 should not appear in results because Applicable returns false
	if len(report.Results) != 0 {
		t.Errorf("expected 0 results when code-root not set, got %d", len(report.Results))
	}
}

func TestTraceabilityValidAnnotation(t *testing.T) {
	// Create a temp Go file with a known function
	codeRoot := t.TempDir()
	goFile := filepath.Join(codeRoot, "pkg", "handler.go")
	if err := os.MkdirAll(filepath.Dir(goFile), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(goFile, []byte(`package pkg

func HandleRequest(input string) error {
	return nil
}
`), 0o644); err != nil {
		t.Fatal(err)
	}

	rawText := "### INV-001: Test Invariant\n\n**Implementation Trace:**\n- Source: `pkg/handler.go::HandleRequest`\n"

	dbPtr, specID := setupTraceDB(t, []storage.Invariant{
		{
			InvariantID: "INV-001",
			Title:       "Test Invariant",
			Statement:   "test statement",
			LineStart:   1,
			LineEnd:     10,
			RawText:     rawText,
			ContentHash: "inv001",
		},
	})
	db := *dbPtr

	report, err := validator.Validate(db, specID, validator.ValidateOptions{
		CheckIDs: []int{13},
		CodeRoot: codeRoot,
	})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(report.Results))
	}

	r := report.Results[0]
	if !r.Passed {
		t.Errorf("expected PASS, got FAIL: %s", r.Summary)
		for _, f := range r.Findings {
			t.Logf("  %s: %s", f.Severity, f.Message)
		}
	}

	// Should have at least one info finding for the valid annotation
	hasInfo := false
	for _, f := range r.Findings {
		if f.Severity == validator.SeverityInfo && f.InvariantID == "INV-001" {
			hasInfo = true
		}
	}
	if !hasInfo {
		t.Error("expected an info finding for the valid annotation")
	}
}

func TestTraceabilityBrokenFile(t *testing.T) {
	codeRoot := t.TempDir()

	rawText := "### INV-002: Broken File\n\n**Implementation Trace:**\n- Source: `nonexistent/file.go::SomeFunc`\n"

	dbPtr, specID := setupTraceDB(t, []storage.Invariant{
		{
			InvariantID: "INV-002",
			Title:       "Broken File",
			Statement:   "test statement",
			LineStart:   1,
			LineEnd:     10,
			RawText:     rawText,
			ContentHash: "inv002",
		},
	})
	db := *dbPtr

	report, err := validator.Validate(db, specID, validator.ValidateOptions{
		CheckIDs: []int{13},
		CodeRoot: codeRoot,
	})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(report.Results))
	}

	r := report.Results[0]
	if r.Passed {
		t.Error("expected FAIL for broken file reference, got PASS")
	}

	// Should have an error finding about missing file
	hasFileError := false
	for _, f := range r.Findings {
		if f.Severity == validator.SeverityError && f.InvariantID == "INV-002" {
			hasFileError = true
		}
	}
	if !hasFileError {
		t.Error("expected an error finding for the broken file reference")
	}
}

func TestTraceabilityBrokenFunction(t *testing.T) {
	codeRoot := t.TempDir()
	goFile := filepath.Join(codeRoot, "pkg", "handler.go")
	if err := os.MkdirAll(filepath.Dir(goFile), 0o755); err != nil {
		t.Fatal(err)
	}
	// File exists but contains a different function
	if err := os.WriteFile(goFile, []byte(`package pkg

func DifferentFunc() {}
`), 0o644); err != nil {
		t.Fatal(err)
	}

	rawText := "### INV-003: Broken Function\n\n**Implementation Trace:**\n- Source: `pkg/handler.go::MissingFunc`\n"

	dbPtr, specID := setupTraceDB(t, []storage.Invariant{
		{
			InvariantID: "INV-003",
			Title:       "Broken Function",
			Statement:   "test statement",
			LineStart:   1,
			LineEnd:     10,
			RawText:     rawText,
			ContentHash: "inv003",
		},
	})
	db := *dbPtr

	report, err := validator.Validate(db, specID, validator.ValidateOptions{
		CheckIDs: []int{13},
		CodeRoot: codeRoot,
	})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(report.Results))
	}

	r := report.Results[0]
	if r.Passed {
		t.Error("expected FAIL for broken function reference, got PASS")
	}

	// Should have an error about function not found
	hasFuncError := false
	for _, f := range r.Findings {
		if f.Severity == validator.SeverityError && f.InvariantID == "INV-003" {
			hasFuncError = true
		}
	}
	if !hasFuncError {
		t.Error("expected an error finding for the broken function reference")
	}
}

func TestTraceabilityMethodReceiver(t *testing.T) {
	codeRoot := t.TempDir()
	goFile := filepath.Join(codeRoot, "pkg", "service.go")
	if err := os.MkdirAll(filepath.Dir(goFile), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(goFile, []byte(`package pkg

type Service struct{}

func (s *Service) HandleRequest(input string) error {
	return nil
}
`), 0o644); err != nil {
		t.Fatal(err)
	}

	rawText := "### INV-004: Method Test\n\n**Implementation Trace:**\n- Source: `pkg/service.go::HandleRequest`\n"

	dbPtr, specID := setupTraceDB(t, []storage.Invariant{
		{
			InvariantID: "INV-004",
			Title:       "Method Test",
			Statement:   "test statement",
			LineStart:   1,
			LineEnd:     10,
			RawText:     rawText,
			ContentHash: "inv004",
		},
	})
	db := *dbPtr

	report, err := validator.Validate(db, specID, validator.ValidateOptions{
		CheckIDs: []int{13},
		CodeRoot: codeRoot,
	})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(report.Results))
	}

	r := report.Results[0]
	if !r.Passed {
		t.Errorf("expected PASS for method receiver match, got FAIL: %s", r.Summary)
		for _, f := range r.Findings {
			t.Logf("  %s: %s", f.Severity, f.Message)
		}
	}
}

func TestTraceabilityNoAnnotations(t *testing.T) {
	rawText := "### INV-005: Plain Invariant\n\nNo implementation trace here.\n"

	dbPtr, specID := setupTraceDB(t, []storage.Invariant{
		{
			InvariantID: "INV-005",
			Title:       "Plain Invariant",
			Statement:   "test statement",
			LineStart:   1,
			LineEnd:     10,
			RawText:     rawText,
			ContentHash: "inv005",
		},
	})
	db := *dbPtr

	codeRoot := t.TempDir()

	report, err := validator.Validate(db, specID, validator.ValidateOptions{
		CheckIDs: []int{13},
		CodeRoot: codeRoot,
	})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(report.Results))
	}

	r := report.Results[0]
	// Should PASS with info message about no annotations
	if !r.Passed {
		t.Errorf("expected PASS for no annotations, got FAIL: %s", r.Summary)
	}

	hasInfoMsg := false
	for _, f := range r.Findings {
		if f.Severity == validator.SeverityInfo && f.Message == "no Implementation Trace annotations found in any invariant" {
			hasInfoMsg = true
		}
	}
	if !hasInfoMsg {
		t.Error("expected info message about no annotations")
	}
}
