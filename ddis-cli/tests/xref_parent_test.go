//go:build integration

package tests

import (
	"testing"

	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/validator"
)

// Uses getXRefDB from integration_helpers_test.go

// TestINV_XREF_SOUND verifies that every resolved cross-reference actually
// points to an existing element (no false positives in resolution).
func TestINV_XREF_SOUND(t *testing.T) {
	dbPtr, specID := getXRefDB(t)
	db := *dbPtr

	// Get all resolved refs for the CLI spec
	rows, err := db.Query(
		`SELECT id, ref_type, ref_target FROM cross_references WHERE spec_id = ? AND resolved = 1`, specID)
	if err != nil {
		t.Fatalf("query resolved refs: %v", err)
	}
	defer rows.Close()

	type ref struct {
		id     int64
		typ    string
		target string
	}
	var refs []ref
	for rows.Next() {
		var r ref
		if err := rows.Scan(&r.id, &r.typ, &r.target); err != nil {
			t.Fatalf("scan: %v", err)
		}
		refs = append(refs, r)
	}

	if len(refs) == 0 {
		t.Fatal("no resolved refs found — expected many")
	}

	// Get parent spec ID for fallback lookup
	parentID, err := storage.GetParentSpecID(db, specID)
	if err != nil {
		t.Fatalf("get parent spec ID: %v", err)
	}

	// Verify each resolved ref points to a real element
	bogus := 0
	for _, r := range refs {
		found := existsInSpec(db, specID, r.typ, r.target)
		if !found && parentID != nil {
			found = existsInSpec(db, *parentID, r.typ, r.target)
		}
		if !found {
			bogus++
			t.Errorf("resolved ref %s:%s (id=%d) not found in child or parent spec", r.typ, r.target, r.id)
		}
	}
	t.Logf("checked %d resolved refs, %d bogus", len(refs), bogus)
}

func existsInSpec(db storage.DB, specID int64, refType, target string) bool {
	var x int
	var err error
	switch refType {
	case "section":
		err = db.QueryRow(`SELECT 1 FROM sections WHERE spec_id = ? AND section_path = ?`, specID, target).Scan(&x)
	case "invariant", "app_invariant":
		err = db.QueryRow(`SELECT 1 FROM invariants WHERE spec_id = ? AND invariant_id = ?`, specID, target).Scan(&x)
	case "adr", "app_adr":
		err = db.QueryRow(`SELECT 1 FROM adrs WHERE spec_id = ? AND adr_id = ?`, specID, target).Scan(&x)
	case "gate":
		err = db.QueryRow(`SELECT 1 FROM quality_gates WHERE spec_id = ? AND gate_id = ?`, specID, target).Scan(&x)
	default:
		return false
	}
	return err == nil
}

// TestINV_XREF_COMPLETE verifies that all resolvable references are resolved.
// After parent fallback, only truly unresolvable refs (templates, etc.) should remain.
func TestINV_XREF_COMPLETE(t *testing.T) {
	dbPtr, specID := getXRefDB(t)
	db := *dbPtr

	unresolved, err := storage.GetUnresolvedRefs(db, specID)
	if err != nil {
		t.Fatalf("get unresolved refs: %v", err)
	}

	// All unresolved should be template/example refs
	realUnresolved := 0
	for _, xr := range unresolved {
		if !isTemplateRef(xr.RefTarget) {
			realUnresolved++
			t.Errorf("unresolved non-template ref: %s %s (line %d)", xr.RefType, xr.RefTarget, xr.SourceLine)
		}
	}
	t.Logf("%d total unresolved, %d non-template", len(unresolved), realUnresolved)
}

func isTemplateRef(target string) bool {
	for _, pat := range []string{"NNN", "XXX", "N.M", "§N.M", "INV-NNN", "ADR-NNN"} {
		if target == pat || contains(target, pat) {
			return true
		}
	}
	return false
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && searchString(s, substr)
}

func searchString(s, sub string) bool {
	for i := 0; i+len(sub) <= len(s); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}

// TestINV_XREF_ISOLATION verifies that parent and child specs have separate
// spec_ids and that cross-references don't accidentally cross-contaminate.
func TestINV_XREF_ISOLATION(t *testing.T) {
	dbPtr, specID := getXRefDB(t)
	db := *dbPtr

	parentID, err := storage.GetParentSpecID(db, specID)
	if err != nil {
		t.Fatalf("get parent spec ID: %v", err)
	}
	if parentID == nil {
		t.Fatal("expected parent spec ID to be set")
	}

	// Verify different spec IDs
	if specID == *parentID {
		t.Fatalf("child specID (%d) must differ from parent specID (%d)", specID, *parentID)
	}

	// Verify child refs belong to child spec
	var childRefCount int
	err = db.QueryRow(
		`SELECT COUNT(*) FROM cross_references WHERE spec_id = ?`, specID).Scan(&childRefCount)
	if err != nil {
		t.Fatalf("count child refs: %v", err)
	}

	// Verify parent refs belong to parent spec
	var parentRefCount int
	err = db.QueryRow(
		`SELECT COUNT(*) FROM cross_references WHERE spec_id = ?`, *parentID).Scan(&parentRefCount)
	if err != nil {
		t.Fatalf("count parent refs: %v", err)
	}

	// No cross-contamination: child refs should not have parent spec_id
	var contaminatedCount int
	err = db.QueryRow(
		`SELECT COUNT(*) FROM cross_references WHERE spec_id = ? AND source_file_id IN
		 (SELECT id FROM source_files WHERE spec_id = ?)`, specID, *parentID).Scan(&contaminatedCount)
	if err != nil {
		t.Fatalf("check contamination: %v", err)
	}
	if contaminatedCount > 0 {
		t.Errorf("found %d cross-contaminated refs", contaminatedCount)
	}

	t.Logf("child refs: %d, parent refs: %d, contaminated: %d", childRefCount, parentRefCount, contaminatedCount)
}

// TestParentResolution_98Refs is the critical integration test: parsing the CLI
// spec with its parent should resolve the ~98 previously unresolved references,
// causing Check 1 to pass.
func TestParentResolution_98Refs(t *testing.T) {
	dbPtr, specID := getXRefDB(t)
	db := *dbPtr

	// Run Check 1 (cross-reference integrity)
	report, err := validator.Validate(db, specID, validator.ValidateOptions{
		CheckIDs: []int{1},
	})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) == 0 {
		t.Fatal("expected Check 1 result")
	}

	check1 := report.Results[0]
	if !check1.Passed {
		// Count non-template errors
		errors := 0
		for _, f := range check1.Findings {
			if f.Severity == "error" {
				errors++
				t.Logf("unresolved error: %s", f.Message)
			}
		}
		t.Errorf("Check 1 failed with %d errors (summary: %s)", errors, check1.Summary)
	} else {
		t.Logf("Check 1 passed: %s", check1.Summary)
	}
}
