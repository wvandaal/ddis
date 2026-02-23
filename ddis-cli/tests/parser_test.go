package tests

import (
	"database/sql"
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/storage"
)

// projectRoot is defined in roundtrip_test.go (same package).

// TestParsePopulatesIndex verifies element counts after parsing the real spec.
func TestParsePopulatesIndex(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	tests := []struct {
		label    string
		query    string
		minCount int
	}{
		{"Invariants", "SELECT COUNT(*) FROM invariants WHERE spec_id = ?", 20},
		{"ADRs", "SELECT COUNT(*) FROM adrs WHERE spec_id = ?", 11},
		{"Quality Gates", "SELECT COUNT(*) FROM quality_gates WHERE spec_id = ?", 12},
		{"Sections", "SELECT COUNT(*) FROM sections WHERE spec_id = ?", 80},
		{"Negative Specs", "SELECT COUNT(*) FROM negative_specs WHERE spec_id = ?", 50},
		{"Verification Prompts", "SELECT COUNT(*) FROM verification_prompts WHERE spec_id = ?", 6},
		{"Glossary Entries", "SELECT COUNT(*) FROM glossary_entries WHERE spec_id = ?", 40},
		{"Cross-References", "SELECT COUNT(*) FROM cross_references WHERE spec_id = ?", 100},
		{"Meta-Instructions", "SELECT COUNT(*) FROM meta_instructions WHERE spec_id = ?", 2},
	}

	for _, tt := range tests {
		t.Run(tt.label, func(t *testing.T) {
			var count int
			if err := db.QueryRow(tt.query, specID).Scan(&count); err != nil {
				t.Fatalf("query %s: %v", tt.label, err)
			}
			if count < tt.minCount {
				t.Errorf("%s: got %d, want >= %d", tt.label, count, tt.minCount)
			} else {
				t.Logf("%s: %d (>= %d)", tt.label, count, tt.minCount)
			}
		})
	}
}

// TestInvariantFields verifies that parsed invariants have all expected fields.
func TestInvariantFields(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	rows, err := db.Query(
		`SELECT invariant_id, title, statement, semi_formal, violation_scenario,
		        validation_method, why_this_matters
		 FROM invariants WHERE spec_id = ? ORDER BY invariant_id`, specID)
	if err != nil {
		t.Fatalf("query invariants: %v", err)
	}
	defer rows.Close()

	count := 0
	for rows.Next() {
		var id, title, stmt string
		var semiFormal, violation, validation, whyMatters sql.NullString
		if err := rows.Scan(&id, &title, &stmt, &semiFormal, &violation, &validation, &whyMatters); err != nil {
			t.Fatalf("scan: %v", err)
		}
		count++

		if title == "" {
			t.Errorf("%s: missing title", id)
		}
		if stmt == "" {
			t.Errorf("%s: missing statement", id)
		}

		// Most invariants should have all components
		if !semiFormal.Valid {
			t.Logf("%s: no semi-formal expression", id)
		}
		if !violation.Valid {
			t.Logf("%s: no violation scenario", id)
		}
		if !whyMatters.Valid {
			t.Logf("%s: no WHY THIS MATTERS", id)
		}
	}

	if count < 20 {
		t.Errorf("expected >= 20 invariants, got %d", count)
	}
}

// TestADRFields verifies that parsed ADRs have required fields.
func TestADRFields(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	rows, err := db.Query(
		`SELECT adr_id, title, problem, decision_text FROM adrs WHERE spec_id = ?`, specID)
	if err != nil {
		t.Fatalf("query: %v", err)
	}
	defer rows.Close()

	count := 0
	for rows.Next() {
		var id, title, problem, decision string
		if err := rows.Scan(&id, &title, &problem, &decision); err != nil {
			t.Fatalf("scan: %v", err)
		}
		count++

		if title == "" {
			t.Errorf("%s: missing title", id)
		}
		if problem == "" {
			t.Errorf("%s: missing problem", id)
		}
		if decision == "" {
			t.Errorf("%s: missing decision", id)
		}
	}

	if count != 11 {
		t.Errorf("expected 11 ADRs, got %d", count)
	}
}

// TestCrossReferenceResolution verifies that most cross-references resolve.
func TestCrossReferenceResolution(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	var total, resolved int
	db.QueryRow("SELECT COUNT(*) FROM cross_references WHERE spec_id = ?", specID).Scan(&total)
	db.QueryRow("SELECT COUNT(*) FROM cross_references WHERE spec_id = ? AND resolved = 1", specID).Scan(&resolved)

	if total == 0 {
		t.Fatal("no cross-references found")
	}

	ratio := float64(resolved) / float64(total) * 100
	t.Logf("Cross-references: %d total, %d resolved (%.1f%%)", total, resolved, ratio)

	if ratio < 90 {
		t.Errorf("cross-reference resolution too low: %.1f%% (want >= 90%%)", ratio)
	}
}

// TestSectionTree verifies section hierarchy.
func TestSectionTree(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	// Check for known section paths
	expectedPaths := []string{
		"PART-0",
		"§0.1",
		"§0.5",
		"§0.6",
		"§0.7",
	}

	for _, p := range expectedPaths {
		var count int
		db.QueryRow(
			"SELECT COUNT(*) FROM sections WHERE spec_id = ? AND section_path = ?",
			specID, p).Scan(&count)
		if count == 0 {
			t.Errorf("expected section %s not found", p)
		}
	}
}
