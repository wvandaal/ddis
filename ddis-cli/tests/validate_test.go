package tests

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/validator"
)

// sharedValidateDB caches a parsed monolith DB for validation tests.
var sharedValidateDB *validateTestDB

type validateTestDB struct {
	db     *storage.DB
	specID int64
}

func getValidateDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedValidateDB != nil {
		return sharedValidateDB.db, sharedValidateDB.specID
	}

	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "validate_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	sharedValidateDB = &validateTestDB{db: &db, specID: specID}
	return sharedValidateDB.db, sharedValidateDB.specID
}

func TestValidateAllChecks(t *testing.T) {
	dbPtr, specID := getValidateDB(t)
	db := *dbPtr

	report, err := validator.Validate(db, specID, validator.ValidateOptions{})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	t.Logf("Total checks: %d, Passed: %d, Failed: %d, Errors: %d, Warnings: %d",
		report.TotalChecks, report.Passed, report.Failed, report.Errors, report.Warnings)

	for _, r := range report.Results {
		status := "PASS"
		if !r.Passed {
			status = "FAIL"
		}
		t.Logf("  [%s] Check %d: %s — %s", status, r.CheckID, r.CheckName, r.Summary)
	}

	// Should run at least 8 checks (monolith only = 8 universal checks)
	if report.TotalChecks < 8 {
		t.Errorf("total_checks = %d, want >= 8", report.TotalChecks)
	}
}

func TestValidateXRefIntegrity(t *testing.T) {
	dbPtr, specID := getValidateDB(t)
	db := *dbPtr

	report, err := validator.Validate(db, specID, validator.ValidateOptions{CheckIDs: []int{1}})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(report.Results))
	}

	r := report.Results[0]
	t.Logf("Check 1: %s — %s", r.CheckName, r.Summary)

	// Count actual errors (not template refs)
	errorCount := 0
	for _, f := range r.Findings {
		if f.Severity == validator.SeverityError {
			errorCount++
		}
	}
	t.Logf("  %d error findings, %d total findings", errorCount, len(r.Findings))

	// We know there are ~7 unresolved refs in the real spec
	if len(r.Findings) == 0 {
		t.Error("expected some findings for xref integrity")
	}
}

func TestValidateINV003(t *testing.T) {
	dbPtr, specID := getValidateDB(t)
	db := *dbPtr

	report, err := validator.Validate(db, specID, validator.ValidateOptions{CheckIDs: []int{2}})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(report.Results))
	}

	r := report.Results[0]
	t.Logf("Check 2 (INV-003): %s — passed=%v", r.Summary, r.Passed)

	// All invariants should have at least a statement
	if r.Passed {
		t.Log("  All invariants have core components")
	}
}

func TestValidateINV017(t *testing.T) {
	dbPtr, specID := getValidateDB(t)
	db := *dbPtr

	report, err := validator.Validate(db, specID, validator.ValidateOptions{CheckIDs: []int{9}})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(report.Results))
	}

	r := report.Results[0]
	t.Logf("Check 9 (INV-017): %s — passed=%v", r.Summary, r.Passed)

	for _, f := range r.Findings {
		t.Logf("  %s: %s", f.Severity, f.Message)
	}
}

func TestValidateGate1(t *testing.T) {
	dbPtr, specID := getValidateDB(t)
	db := *dbPtr

	report, err := validator.Validate(db, specID, validator.ValidateOptions{CheckIDs: []int{10}})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(report.Results))
	}

	r := report.Results[0]
	if !r.Passed {
		t.Errorf("Gate-1 structural conformance failed: %s", r.Summary)
		for _, f := range r.Findings {
			t.Errorf("  %s: %s", f.Severity, f.Message)
		}
	}
	t.Logf("Check 10 (Gate-1): %s", r.Summary)
}

func TestValidateModularChecks(t *testing.T) {
	manifestPath := filepath.Join(projectRoot(), "ddis-modular", "manifest.yaml")
	if _, err := os.Stat(manifestPath); os.IsNotExist(err) {
		t.Skipf("manifest.yaml not found at %s", manifestPath)
	}

	dbPath := filepath.Join(t.TempDir(), "modular_validate.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseModularSpec(manifestPath, db)
	if err != nil {
		t.Fatalf("parse modular: %v", err)
	}

	// Run modular-only checks (5-8)
	report, err := validator.Validate(db, specID, validator.ValidateOptions{CheckIDs: []int{5, 6, 7, 8}})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 4 {
		t.Fatalf("expected 4 modular results, got %d", len(report.Results))
	}

	for _, r := range report.Results {
		status := "PASS"
		if !r.Passed {
			status = "FAIL"
		}
		t.Logf("[%s] Check %d: %s — %s", status, r.CheckID, r.CheckName, r.Summary)
	}
}

func TestValidateSelectiveChecks(t *testing.T) {
	dbPtr, specID := getValidateDB(t)
	db := *dbPtr

	report, err := validator.Validate(db, specID, validator.ValidateOptions{CheckIDs: []int{1, 2, 3}})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 3 {
		t.Errorf("expected 3 results for checks 1,2,3; got %d", len(report.Results))
	}

	// Verify the right checks ran
	expectedIDs := map[int]bool{1: true, 2: true, 3: true}
	for _, r := range report.Results {
		if !expectedIDs[r.CheckID] {
			t.Errorf("unexpected check ID %d", r.CheckID)
		}
	}
}

func TestValidateJSON(t *testing.T) {
	dbPtr, specID := getValidateDB(t)
	db := *dbPtr

	report, err := validator.Validate(db, specID, validator.ValidateOptions{CheckIDs: []int{10}})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	out, err := validator.RenderReport(report, true)
	if err != nil {
		t.Fatalf("render JSON: %v", err)
	}

	// Verify it's valid JSON
	var parsed validator.Report
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("parse JSON output: %v", err)
	}

	if parsed.TotalChecks != report.TotalChecks {
		t.Errorf("JSON total_checks = %d, want %d", parsed.TotalChecks, report.TotalChecks)
	}
	if parsed.SpecPath == "" {
		t.Error("JSON spec_path is empty")
	}
	t.Logf("JSON output: %d bytes, total_checks=%d", len(out), parsed.TotalChecks)
}
