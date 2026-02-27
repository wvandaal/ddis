package tests

import (
	"encoding/json"
	"testing"

	"github.com/wvandaal/ddis/internal/validator"
)

func TestValidateAllChecks(t *testing.T) {
	db, specID := buildSyntheticDB(t)

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

	// Should run at least 4 universal checks
	if report.TotalChecks < 4 {
		t.Errorf("total_checks = %d, want >= 4", report.TotalChecks)
	}
}

func TestValidateXRefIntegrity(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	report, err := validator.Validate(db, specID, validator.ValidateOptions{CheckIDs: []int{1}})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(report.Results))
	}

	r := report.Results[0]
	t.Logf("Check 1: %s — %s", r.CheckName, r.Summary)

	// Synthetic DB has 2 unresolved refs, so there should be findings
	if len(r.Findings) == 0 {
		t.Error("expected some findings for xref integrity (synthetic has 2 unresolved)")
	}
}

func TestValidateINV003(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	report, err := validator.Validate(db, specID, validator.ValidateOptions{CheckIDs: []int{2}})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(report.Results))
	}

	r := report.Results[0]
	t.Logf("Check 2 (INV-003): %s — passed=%v", r.Summary, r.Passed)

	// All synthetic invariants have all components, so this should pass
	if !r.Passed {
		t.Error("Check 2 should pass with fully-populated synthetic invariants")
	}
}

func TestValidateINV017(t *testing.T) {
	db, specID := buildSyntheticDB(t)

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
	db, specID := buildSyntheticDB(t)

	report, err := validator.Validate(db, specID, validator.ValidateOptions{CheckIDs: []int{10}})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if len(report.Results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(report.Results))
	}

	r := report.Results[0]
	t.Logf("Check 10 (Gate-1): %s — passed=%v", r.Summary, r.Passed)
	for _, f := range r.Findings {
		t.Logf("  %s: %s", f.Severity, f.Message)
	}
}

func TestValidateSelectiveChecks(t *testing.T) {
	db, specID := buildSyntheticDB(t)

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
	db, specID := buildSyntheticDB(t)

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
	t.Logf("JSON output: %d bytes, total_checks=%d", len(out), parsed.TotalChecks)
}
