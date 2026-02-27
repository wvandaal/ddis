package tests

import (
	"testing"

	"github.com/wvandaal/ddis/internal/query"
)

func TestQueryInvariant(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	frag, err := query.QueryTarget(db, specID, "INV-001", query.QueryOptions{})
	if err != nil {
		t.Fatalf("query INV-001: %v", err)
	}

	if frag.Type != query.FragmentInvariant {
		t.Errorf("type = %s, want invariant", frag.Type)
	}
	if frag.ID != "INV-001" {
		t.Errorf("id = %s, want INV-001", frag.ID)
	}
	if frag.Title == "" {
		t.Error("title is empty")
	}
	if frag.RawText == "" {
		t.Error("raw_text is empty")
	}
	t.Logf("INV-001: %s (lines %d–%d, section %s)", frag.Title, frag.LineStart, frag.LineEnd, frag.SectionPath)
}

func TestQuerySection(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	frag, err := query.QueryTarget(db, specID, "§1", query.QueryOptions{})
	if err != nil {
		t.Fatalf("query §1: %v", err)
	}

	if frag.Type != query.FragmentSection {
		t.Errorf("type = %s, want section", frag.Type)
	}
	if frag.ID != "§1" {
		t.Errorf("id = %s, want §1", frag.ID)
	}
	if frag.Title == "" {
		t.Error("title is empty")
	}
	if frag.RawText == "" {
		t.Error("raw_text is empty")
	}
	t.Logf("§1: %s (lines %d–%d)", frag.Title, frag.LineStart, frag.LineEnd)
}

func TestQueryADR(t *testing.T) {
	db, specID := buildSyntheticDB(t)

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
	db, specID := buildSyntheticDB(t)

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
	db, specID := buildSyntheticDB(t)

	// §1.1 has 4 xrefs (INV-001, INV-002, ADR-001, §3.1-unresolved)
	frag, err := query.QueryTarget(db, specID, "§1.1", query.QueryOptions{ResolveRefs: true})
	if err != nil {
		t.Fatalf("query §1.1 with resolve-refs: %v", err)
	}

	if len(frag.ResolvedRefs) == 0 {
		t.Error("expected resolved refs, got none")
	}
	t.Logf("§1.1 has %d outgoing refs", len(frag.ResolvedRefs))
}

func TestQueryBacklinks(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	// INV-001 has a cross-reference pointing to it
	frag, err := query.QueryTarget(db, specID, "INV-001", query.QueryOptions{Backlinks: true})
	if err != nil {
		t.Fatalf("query INV-001 with backlinks: %v", err)
	}

	if len(frag.Backlinks) == 0 {
		t.Error("expected backlinks for INV-001, got none")
	}
	t.Logf("INV-001 has %d backlinks", len(frag.Backlinks))
}

func TestQueryGlossary(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	// §1.1 contains section about data integrity
	frag, err := query.QueryTarget(db, specID, "§1.1", query.QueryOptions{IncludeGlossary: true})
	if err != nil {
		t.Fatalf("query §1.1 with glossary: %v", err)
	}

	// Glossary matching depends on term overlap with section raw_text
	t.Logf("§1.1 matches %d glossary terms", len(frag.GlossaryDefs))
}

func TestQueryList(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	tests := []struct {
		listType query.ListType
		minCount int
	}{
		{query.ListInvariants, 5},
		{query.ListADRs, 3},
		{query.ListGates, 2},
		{query.ListSections, 5},
		{query.ListGlossary, 3},
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
	db, specID := buildSyntheticDB(t)

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
	db, specID := buildSyntheticDB(t)

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
