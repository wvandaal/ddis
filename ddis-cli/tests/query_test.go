package tests

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/query"
	"github.com/wvandaal/ddis/internal/storage"
)

// sharedQueryDB caches a parsed monolith DB for query tests.
var sharedQueryDB *queryTestDB

type queryTestDB struct {
	db     *storage.DB
	specID int64
	dbPath string
}

func getQueryDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedQueryDB != nil {
		return sharedQueryDB.db, sharedQueryDB.specID
	}

	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "query_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	sharedQueryDB = &queryTestDB{db: &db, specID: specID, dbPath: dbPath}
	return sharedQueryDB.db, sharedQueryDB.specID
}

func TestQueryInvariant(t *testing.T) {
	dbPtr, specID := getQueryDB(t)
	db := *dbPtr

	frag, err := query.QueryTarget(db, specID, "INV-006", query.QueryOptions{})
	if err != nil {
		t.Fatalf("query INV-006: %v", err)
	}

	if frag.Type != query.FragmentInvariant {
		t.Errorf("type = %s, want invariant", frag.Type)
	}
	if frag.ID != "INV-006" {
		t.Errorf("id = %s, want INV-006", frag.ID)
	}
	if frag.Title == "" {
		t.Error("title is empty")
	}
	if frag.RawText == "" {
		t.Error("raw_text is empty")
	}
	if frag.LineStart <= 0 {
		t.Errorf("line_start = %d, want > 0", frag.LineStart)
	}
	t.Logf("INV-006: %s (lines %d–%d, section %s)", frag.Title, frag.LineStart, frag.LineEnd, frag.SectionPath)
}

func TestQuerySection(t *testing.T) {
	dbPtr, specID := getQueryDB(t)
	db := *dbPtr

	frag, err := query.QueryTarget(db, specID, "§0.5", query.QueryOptions{})
	if err != nil {
		t.Fatalf("query §0.5: %v", err)
	}

	if frag.Type != query.FragmentSection {
		t.Errorf("type = %s, want section", frag.Type)
	}
	if frag.ID != "§0.5" {
		t.Errorf("id = %s, want §0.5", frag.ID)
	}
	if frag.Title == "" {
		t.Error("title is empty")
	}
	if frag.RawText == "" {
		t.Error("raw_text is empty")
	}
	t.Logf("§0.5: %s (lines %d–%d)", frag.Title, frag.LineStart, frag.LineEnd)
}

func TestQueryADR(t *testing.T) {
	dbPtr, specID := getQueryDB(t)
	db := *dbPtr

	frag, err := query.QueryTarget(db, specID, "ADR-003", query.QueryOptions{})
	if err != nil {
		t.Fatalf("query ADR-003: %v", err)
	}

	if frag.Type != query.FragmentADR {
		t.Errorf("type = %s, want adr", frag.Type)
	}
	if frag.ID != "ADR-003" {
		t.Errorf("id = %s, want ADR-003", frag.ID)
	}
	if frag.Title == "" {
		t.Error("title is empty")
	}
	t.Logf("ADR-003: %s", frag.Title)
}

func TestQueryGate(t *testing.T) {
	dbPtr, specID := getQueryDB(t)
	db := *dbPtr

	frag, err := query.QueryTarget(db, specID, "Gate-1", query.QueryOptions{})
	if err != nil {
		t.Fatalf("query Gate-1: %v", err)
	}

	if frag.Type != query.FragmentGate {
		t.Errorf("type = %s, want gate", frag.Type)
	}
	if frag.ID != "Gate-1" {
		t.Errorf("id = %s, want Gate-1", frag.ID)
	}
	if frag.Title == "" {
		t.Error("title is empty")
	}
	t.Logf("Gate-1: %s", frag.Title)
}

func TestQueryResolveRefs(t *testing.T) {
	dbPtr, specID := getQueryDB(t)
	db := *dbPtr

	frag, err := query.QueryTarget(db, specID, "§0.5", query.QueryOptions{ResolveRefs: true})
	if err != nil {
		t.Fatalf("query §0.5 with resolve-refs: %v", err)
	}

	if len(frag.ResolvedRefs) == 0 {
		t.Error("expected resolved refs, got none")
	}
	t.Logf("§0.5 has %d outgoing refs", len(frag.ResolvedRefs))

	// At least some should have RawText
	hasText := 0
	for _, ref := range frag.ResolvedRefs {
		if ref.RawText != "" {
			hasText++
		}
	}
	if hasText == 0 {
		t.Error("no resolved refs have raw_text")
	}
}

func TestQueryBacklinks(t *testing.T) {
	dbPtr, specID := getQueryDB(t)
	db := *dbPtr

	frag, err := query.QueryTarget(db, specID, "INV-006", query.QueryOptions{Backlinks: true})
	if err != nil {
		t.Fatalf("query INV-006 with backlinks: %v", err)
	}

	if len(frag.Backlinks) == 0 {
		t.Error("expected backlinks for INV-006, got none")
	}
	t.Logf("INV-006 has %d backlinks", len(frag.Backlinks))
}

func TestQueryGlossary(t *testing.T) {
	dbPtr, specID := getQueryDB(t)
	db := *dbPtr

	// §0.5 should contain glossary terms since it's the invariant registry
	frag, err := query.QueryTarget(db, specID, "§0.5", query.QueryOptions{IncludeGlossary: true})
	if err != nil {
		t.Fatalf("query §0.5 with glossary: %v", err)
	}

	if len(frag.GlossaryDefs) == 0 {
		t.Error("expected glossary matches, got none")
	}
	t.Logf("§0.5 matches %d glossary terms", len(frag.GlossaryDefs))
}

func TestQueryList(t *testing.T) {
	dbPtr, specID := getQueryDB(t)
	db := *dbPtr

	tests := []struct {
		listType query.ListType
		minCount int
	}{
		{query.ListInvariants, 20},
		{query.ListADRs, 11},
		{query.ListGates, 12},
		{query.ListSections, 80},
		{query.ListGlossary, 40},
	}

	for _, tt := range tests {
		t.Run(string(tt.listType), func(t *testing.T) {
			items, err := query.ListElements(db, specID, tt.listType)
			if err != nil {
				t.Fatalf("list %s: %v", tt.listType, err)
			}
			if len(items) < tt.minCount {
				t.Errorf("got %d items, want >= %d", len(items), tt.minCount)
			}
			t.Logf("%s: %d items", tt.listType, len(items))
		})
	}
}

func TestQueryStats(t *testing.T) {
	dbPtr, specID := getQueryDB(t)
	db := *dbPtr

	stats, err := query.ComputeStats(db, specID)
	if err != nil {
		t.Fatalf("compute stats: %v", err)
	}

	if stats.SpecPath == "" {
		t.Error("spec_path is empty")
	}
	if stats.TotalLines <= 0 {
		t.Errorf("total_lines = %d, want > 0", stats.TotalLines)
	}
	if stats.XRefTotal <= 0 {
		t.Errorf("xref_total = %d, want > 0", stats.XRefTotal)
	}
	if stats.XRefResolutionPc < 90 {
		t.Errorf("xref_resolution = %.1f%%, want >= 90%%", stats.XRefResolutionPc)
	}

	// All important counts > 0
	required := []string{"sections", "invariants", "adrs", "quality_gates", "negative_specs", "glossary_entries"}
	for _, key := range required {
		if stats.ElementCounts[key] <= 0 {
			t.Errorf("%s count = %d, want > 0", key, stats.ElementCounts[key])
		}
	}

	t.Logf("Stats: %d lines, %d xrefs (%.1f%% resolved)", stats.TotalLines, stats.XRefTotal, stats.XRefResolutionPc)
}

func TestQueryBadTarget(t *testing.T) {
	dbPtr, specID := getQueryDB(t)
	db := *dbPtr

	// Non-existent invariant
	_, err := query.QueryTarget(db, specID, "INV-999", query.QueryOptions{})
	if err == nil {
		t.Error("expected error for INV-999, got nil")
	}

	// Invalid format
	_, err = query.QueryTarget(db, specID, "INVALID", query.QueryOptions{})
	if err == nil {
		t.Error("expected error for INVALID target, got nil")
	}
}
