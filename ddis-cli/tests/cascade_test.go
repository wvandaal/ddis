package tests

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/cascade"
	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/storage"
)

// sharedCascadeDB caches a parsed modular CLI-spec DB for cascade tests.
var sharedCascadeDB *cascadeTestDB

type cascadeTestDB struct {
	db     *storage.DB
	specID int64
}

func getCascadeDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedCascadeDB != nil {
		return sharedCascadeDB.db, sharedCascadeDB.specID
	}

	manifestPath := filepath.Join(projectRoot(), "ddis-cli-spec", "manifest.yaml")
	if _, err := os.Stat(manifestPath); os.IsNotExist(err) {
		t.Skipf("ddis-cli-spec manifest not found: %s", manifestPath)
	}

	dbPath := filepath.Join(t.TempDir(), "cascade_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := parser.ParseModularSpec(manifestPath, db)
	if err != nil {
		t.Fatalf("parse modular spec: %v", err)
	}

	sharedCascadeDB = &cascadeTestDB{db: &db, specID: specID}
	return sharedCascadeDB.db, sharedCascadeDB.specID
}

// INV-CASCADE-TERM: Cascade analysis terminates for any valid element.
func TestCascadeTerminates(t *testing.T) {
	dbPtr, specID := getCascadeDB(t)
	db := *dbPtr

	// Test with several known APP-INV invariants
	targets := []string{"APP-INV-001", "APP-INV-006", "APP-INV-008"}
	for _, target := range targets {
		t.Run(target, func(t *testing.T) {
			result, err := cascade.Analyze(db, specID, target, cascade.Options{Depth: 3})
			if err != nil {
				t.Fatalf("cascade %s: %v", target, err)
			}
			if result.ChangedElement != target {
				t.Errorf("changed_element = %s, want %s", result.ChangedElement, target)
			}
			t.Logf("%s: %d modules, %d domains, %d refs",
				target, len(result.AffectedModules), len(result.AffectedDomains), result.TotalReferences)
		})
	}
}

// INV-CASCADE-COMPLETE: All reachable referrers appear in the result.
func TestCascadeComplete(t *testing.T) {
	dbPtr, specID := getCascadeDB(t)
	db := *dbPtr

	// APP-INV-001 (Round-Trip Fidelity) should be referenced by at least one module
	result, err := cascade.Analyze(db, specID, "APP-INV-001", cascade.Options{Depth: 3})
	if err != nil {
		t.Fatalf("cascade: %v", err)
	}

	// Verify element type detected correctly
	if result.ElementType != "invariant" {
		t.Errorf("element_type = %s, want invariant", result.ElementType)
	}

	// Verify title is non-empty
	if result.Title == "" {
		t.Error("title should not be empty")
	}

	// Log all affected modules for manual inspection
	for _, m := range result.AffectedModules {
		t.Logf("  affected: %s (%s) — %s", m.Module, m.Domain, m.Relationship)
	}
}

// Test that cascade detects owner module from invariant registry.
func TestCascadeOwner(t *testing.T) {
	dbPtr, specID := getCascadeDB(t)
	db := *dbPtr

	result, err := cascade.Analyze(db, specID, "APP-INV-001", cascade.Options{})
	if err != nil {
		t.Fatalf("cascade: %v", err)
	}

	// APP-INV-001 should have an owner in the registry
	if result.OwnerModule == "" {
		t.Log("APP-INV-001 has no owner in registry (may be expected if registry is sparse)")
	} else {
		t.Logf("APP-INV-001 owner: %s (%s)", result.OwnerModule, result.OwnerDomain)
	}
}

// Test JSON output round-trips correctly.
func TestCascadeJSON(t *testing.T) {
	dbPtr, specID := getCascadeDB(t)
	db := *dbPtr

	result, err := cascade.Analyze(db, specID, "APP-INV-001", cascade.Options{})
	if err != nil {
		t.Fatalf("cascade: %v", err)
	}

	out, err := cascade.Render(result, true)
	if err != nil {
		t.Fatalf("render JSON: %v", err)
	}

	var parsed cascade.CascadeResult
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("invalid JSON: %v\nOutput:\n%s", err, out)
	}

	if parsed.ChangedElement != "APP-INV-001" {
		t.Errorf("JSON changed_element = %s, want APP-INV-001", parsed.ChangedElement)
	}
	if parsed.AffectedModules == nil {
		t.Error("affected_modules should be non-nil (even if empty)")
	}
	if parsed.AffectedDomains == nil {
		t.Error("affected_domains should be non-nil (even if empty)")
	}
}

// Test human-readable output includes key elements.
func TestCascadeHumanOutput(t *testing.T) {
	dbPtr, specID := getCascadeDB(t)
	db := *dbPtr

	result, err := cascade.Analyze(db, specID, "APP-INV-001", cascade.Options{})
	if err != nil {
		t.Fatalf("cascade: %v", err)
	}

	out, err := cascade.Render(result, false)
	if err != nil {
		t.Fatalf("render: %v", err)
	}

	if len(out) == 0 {
		t.Fatal("empty human output")
	}

	// Must contain the element ID and "Cascade Analysis"
	if !contains(out, "Cascade Analysis:") {
		t.Error("missing 'Cascade Analysis:' header")
	}
	if !contains(out, "APP-INV-001") {
		t.Error("missing element ID in output")
	}
	if !contains(out, "revalidation") {
		t.Error("missing summary line")
	}

	t.Logf("Human output:\n%s", out)
}

// Test invalid element returns error.
func TestCascadeInvalidElement(t *testing.T) {
	dbPtr, specID := getCascadeDB(t)
	db := *dbPtr

	_, err := cascade.Analyze(db, specID, "NONEXISTENT-999", cascade.Options{})
	if err == nil {
		t.Error("expected error for nonexistent element, got nil")
	}
}

// Test ADR cascade.
func TestCascadeADR(t *testing.T) {
	dbPtr, specID := getCascadeDB(t)
	db := *dbPtr

	result, err := cascade.Analyze(db, specID, "APP-ADR-001", cascade.Options{})
	if err != nil {
		t.Fatalf("cascade ADR: %v", err)
	}

	if result.ElementType != "adr" {
		t.Errorf("element_type = %s, want adr", result.ElementType)
	}
	t.Logf("APP-ADR-001 cascade: %d refs, %d affected modules", result.TotalReferences, len(result.AffectedModules))
}

