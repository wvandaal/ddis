package checklist

import (
	"database/sql"
	"encoding/json"
	"os"
	"strings"
	"testing"

	"github.com/wvandaal/ddis/internal/storage"
)

// setupTestDB creates a temporary database and populates it with a minimal spec,
// sections, and invariants for testing checklist generation.
func setupTestDB(t *testing.T) (*sql.DB, int64) {
	t.Helper()
	tmp, err := os.CreateTemp(t.TempDir(), "checklist-*.db")
	if err != nil {
		t.Fatalf("create temp file: %v", err)
	}
	tmp.Close()

	db, err := storage.Open(tmp.Name())
	if err != nil {
		t.Fatalf("open database: %v", err)
	}

	// Insert a spec
	specID, err := storage.InsertSpecIndex(db, &storage.SpecIndex{
		SpecPath:    "/test/spec.md",
		SpecName:    "Test Spec",
		DDISVersion: "3.0",
		TotalLines:  100,
		ContentHash: "abc123",
		SourceType:  "monolith",
	})
	if err != nil {
		t.Fatalf("insert spec: %v", err)
	}

	// Insert a source file (required FK for sections)
	sfID, err := storage.InsertSourceFile(db, &storage.SourceFile{
		SpecID:      specID,
		FilePath:    "/test/spec.md",
		FileRole:    "monolith",
		ContentHash: "abc123",
		LineCount:   100,
		RawText:     "test content",
	})
	if err != nil {
		t.Fatalf("insert source file: %v", err)
	}

	// Insert a section
	secID, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "§1.2",
		Title:        "Data Integrity",
		HeadingLevel: 2,
		RawText:      "This section covers data integrity.",
	})
	if err != nil {
		t.Fatalf("insert section: %v", err)
	}

	// Insert invariants with varying validation methods
	invs := []storage.Invariant{
		{
			SpecID:           specID,
			SourceFileID:     sfID,
			InvariantID:      "INV-001",
			Title:            "Round-Trip Fidelity",
			Statement:        "parse(render(parse(doc))) is byte-identical to parse(doc)",
			SemiFormal:       "parse ∘ render ∘ parse = parse",
			ViolationScenario: "A document parsed and rendered produces different bytes when re-parsed.",
			ValidationMethod: "- Parse a document\n- Render it back to markdown\n- Re-parse and compare SHA-256 hashes",
			WhyThisMatters:   "Ensures no information loss during round-trip.",
			SectionID:        secID,
			ContentHash:      "hash1",
		},
		{
			SpecID:           specID,
			SourceFileID:     sfID,
			InvariantID:      "INV-002",
			Title:            "Validation Determinism",
			Statement:        "No clock, RNG, or order dependency",
			SemiFormal:       "validate(spec) = validate(spec) always",
			ViolationScenario: "Running validate twice on the same spec produces different results.",
			ValidationMethod: "Check for time.Now(); verify no RNG seeds; run twice and diff output",
			WhyThisMatters:   "Reproducible builds and verification.",
			SectionID:        secID,
			ContentHash:      "hash2",
		},
		{
			SpecID:           specID,
			SourceFileID:     sfID,
			InvariantID:      "INV-003",
			Title:            "No Validation Method",
			Statement:        "Test invariant with empty validation",
			SemiFormal:       "true",
			ViolationScenario: "N/A",
			ValidationMethod: "",
			WhyThisMatters:   "Tests fallback behavior.",
			SectionID:        secID,
			ContentHash:      "hash3",
		},
	}

	for _, inv := range invs {
		if _, err := storage.InsertInvariant(db, &inv); err != nil {
			t.Fatalf("insert invariant %s: %v", inv.InvariantID, err)
		}
	}

	return db, specID
}

func TestParseValidationMethod_Bullets(t *testing.T) {
	input := "- Parse a document\n- Render it\n- Compare hashes"
	items := parseValidationMethod(input)
	if len(items) != 3 {
		t.Fatalf("expected 3 items, got %d: %v", len(items), items)
	}
	if items[0] != "Parse a document" {
		t.Errorf("items[0] = %q, want %q", items[0], "Parse a document")
	}
}

func TestParseValidationMethod_Numbered(t *testing.T) {
	input := "1. First step\n2. Second step\n3. Third step"
	items := parseValidationMethod(input)
	if len(items) != 3 {
		t.Fatalf("expected 3 items, got %d: %v", len(items), items)
	}
	if items[0] != "First step" {
		t.Errorf("items[0] = %q, want %q", items[0], "First step")
	}
}

func TestParseValidationMethod_Semicolons(t *testing.T) {
	// Semicolons only split if line-based splitting produces zero items.
	// A single-line input with no bullets yields 1 item (the whole line),
	// so semicolons do NOT split.
	input := "Check for time.Now(); verify no RNG seeds; run twice and diff output"
	items := parseValidationMethod(input)
	if len(items) != 1 {
		t.Fatalf("expected 1 item (single line, no markers), got %d: %v", len(items), items)
	}

	// Header-only input: line splitting skips headers, yielding 0 items.
	// Semicolon fallback splits the original text — but with no semicolons,
	// produces 1 item (the raw header text).
	input2 := "# Heading Only"
	items2 := parseValidationMethod(input2)
	if len(items2) != 1 {
		t.Fatalf("expected 1 item (semicolon fallback), got %d: %v", len(items2), items2)
	}
}

func TestParseValidationMethod_Empty(t *testing.T) {
	items := parseValidationMethod("")
	if items != nil {
		t.Fatalf("expected nil for empty input, got %v", items)
	}
}

func TestParseValidationMethod_MixedMarkers(t *testing.T) {
	input := "- Bullet one\n* Star two\n+ Plus three"
	items := parseValidationMethod(input)
	if len(items) != 3 {
		t.Fatalf("expected 3 items, got %d: %v", len(items), items)
	}
}

func TestParseValidationMethod_SkipsHeaders(t *testing.T) {
	input := "# Heading\n- Step one\n## Sub\n- Step two"
	items := parseValidationMethod(input)
	if len(items) != 2 {
		t.Fatalf("expected 2 items (headers skipped), got %d: %v", len(items), items)
	}
}

func TestStripListMarker(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"- item", "item"},
		{"* item", "item"},
		{"+ item", "item"},
		{"1. item", "item"},
		{"12. item", "item"},
		{"1) item", "item"},
		{"no marker", "no marker"},
		{"- ", ""},
	}
	for _, tc := range tests {
		got := stripListMarker(tc.input)
		if got != tc.want {
			t.Errorf("stripListMarker(%q) = %q, want %q", tc.input, got, tc.want)
		}
	}
}

func TestAnalyze_Basic(t *testing.T) {
	db, specID := setupTestDB(t)
	defer db.Close()

	result, err := Analyze(db, specID, Options{})
	if err != nil {
		t.Fatalf("Analyze: %v", err)
	}

	if result.Spec != "Test Spec" {
		t.Errorf("spec name = %q, want %q", result.Spec, "Test Spec")
	}
	if result.TotalInvariants != 3 {
		t.Errorf("total invariants = %d, want 3", result.TotalInvariants)
	}
	if result.TotalItems < 3 {
		t.Errorf("total items = %d, want >= 3", result.TotalItems)
	}
	if len(result.Sections) != 1 {
		t.Fatalf("sections count = %d, want 1", len(result.Sections))
	}
	if result.Sections[0].Section != "§1.2" {
		t.Errorf("section path = %q, want %q", result.Sections[0].Section, "§1.2")
	}
}

func TestAnalyze_FilterByInvariant(t *testing.T) {
	db, specID := setupTestDB(t)
	defer db.Close()

	result, err := Analyze(db, specID, Options{Invariant: "INV-001"})
	if err != nil {
		t.Fatalf("Analyze: %v", err)
	}
	if result.TotalInvariants != 1 {
		t.Errorf("total invariants = %d, want 1", result.TotalInvariants)
	}
	if len(result.Sections) > 0 && result.Sections[0].Items[0].InvariantID != "INV-001" {
		t.Errorf("wrong invariant: %s", result.Sections[0].Items[0].InvariantID)
	}
}

func TestAnalyze_FilterBySection(t *testing.T) {
	db, specID := setupTestDB(t)
	defer db.Close()

	// Filter with matching prefix
	result, err := Analyze(db, specID, Options{Section: "§1"})
	if err != nil {
		t.Fatalf("Analyze: %v", err)
	}
	if result.TotalInvariants != 3 {
		t.Errorf("with matching prefix: total = %d, want 3", result.TotalInvariants)
	}

	// Filter with non-matching prefix
	result, err = Analyze(db, specID, Options{Section: "§99"})
	if err != nil {
		t.Fatalf("Analyze: %v", err)
	}
	if result.TotalInvariants != 0 {
		t.Errorf("with non-matching prefix: total = %d, want 0", result.TotalInvariants)
	}
}

func TestAnalyze_EmptyValidationFallback(t *testing.T) {
	db, specID := setupTestDB(t)
	defer db.Close()

	result, err := Analyze(db, specID, Options{Invariant: "INV-003"})
	if err != nil {
		t.Fatalf("Analyze: %v", err)
	}
	if result.TotalInvariants != 1 {
		t.Fatalf("total invariants = %d, want 1", result.TotalInvariants)
	}
	item := result.Sections[0].Items[0]
	if len(item.Checklist) != 1 {
		t.Fatalf("checklist items = %d, want 1 (fallback)", len(item.Checklist))
	}
	if item.Checklist[0] != "(no validation method specified)" {
		t.Errorf("fallback text = %q", item.Checklist[0])
	}
}

func TestRender_JSON(t *testing.T) {
	db, specID := setupTestDB(t)
	defer db.Close()

	result, err := Analyze(db, specID, Options{Invariant: "INV-001"})
	if err != nil {
		t.Fatalf("Analyze: %v", err)
	}

	out, err := Render(result, true)
	if err != nil {
		t.Fatalf("Render JSON: %v", err)
	}

	// Verify it's valid JSON
	var parsed ChecklistResult
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("invalid JSON: %v", err)
	}
	if parsed.TotalInvariants != 1 {
		t.Errorf("JSON total_invariants = %d, want 1", parsed.TotalInvariants)
	}
}

func TestRender_Human(t *testing.T) {
	db, specID := setupTestDB(t)
	defer db.Close()

	result, err := Analyze(db, specID, Options{})
	if err != nil {
		t.Fatalf("Analyze: %v", err)
	}

	out, err := Render(result, false)
	if err != nil {
		t.Fatalf("Render Human: %v", err)
	}

	if !strings.Contains(out, "Verification Checklist: Test Spec") {
		t.Errorf("missing title in output")
	}
	if !strings.Contains(out, "INV-001") {
		t.Errorf("missing INV-001 in output")
	}
	if !strings.Contains(out, "Total:") {
		t.Errorf("missing Total: in output")
	}
}
