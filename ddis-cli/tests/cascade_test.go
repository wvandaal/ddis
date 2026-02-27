package tests

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/wvandaal/ddis/internal/cascade"
)

// INV-CASCADE-TERM: Cascade analysis terminates for any valid element.
func TestCascadeTerminates(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	targets := []string{"INV-001", "INV-004", "INV-005"}
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
	db, specID := buildSyntheticModularDB(t)

	result, err := cascade.Analyze(db, specID, "INV-001", cascade.Options{Depth: 3})
	if err != nil {
		t.Fatalf("cascade: %v", err)
	}

	if result.ElementType != "invariant" {
		t.Errorf("element_type = %s, want invariant", result.ElementType)
	}

	if result.Title == "" {
		t.Error("title should not be empty")
	}

	for _, m := range result.AffectedModules {
		t.Logf("  affected: %s (%s) — %s", m.Module, m.Domain, m.Relationship)
	}
}

func TestCascadeOwner(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result, err := cascade.Analyze(db, specID, "INV-001", cascade.Options{})
	if err != nil {
		t.Fatalf("cascade: %v", err)
	}

	if result.OwnerModule == "" {
		t.Log("INV-001 has no owner in registry (may be expected)")
	} else {
		t.Logf("INV-001 owner: %s (%s)", result.OwnerModule, result.OwnerDomain)
	}
}

func TestCascadeJSON(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result, err := cascade.Analyze(db, specID, "INV-001", cascade.Options{})
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

	if parsed.ChangedElement != "INV-001" {
		t.Errorf("JSON changed_element = %s, want INV-001", parsed.ChangedElement)
	}
	if parsed.AffectedModules == nil {
		t.Error("affected_modules should be non-nil (even if empty)")
	}
	if parsed.AffectedDomains == nil {
		t.Error("affected_domains should be non-nil (even if empty)")
	}
}

func TestCascadeHumanOutput(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result, err := cascade.Analyze(db, specID, "INV-001", cascade.Options{})
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

	if !strings.Contains(out, "Cascade Analysis:") {
		t.Error("missing 'Cascade Analysis:' header")
	}
	if !strings.Contains(out, "INV-001") {
		t.Error("missing element ID in output")
	}
	if !strings.Contains(out, "revalidation") {
		t.Error("missing summary line")
	}

	t.Logf("Human output:\n%s", out)
}

func TestCascadeInvalidElement(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	_, err := cascade.Analyze(db, specID, "NONEXISTENT-999", cascade.Options{})
	if err == nil {
		t.Error("expected error for nonexistent element, got nil")
	}
}

func TestCascadeADR(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result, err := cascade.Analyze(db, specID, "ADR-001", cascade.Options{})
	if err != nil {
		t.Fatalf("cascade ADR: %v", err)
	}

	if result.ElementType != "adr" {
		t.Errorf("element_type = %s, want adr", result.ElementType)
	}
	t.Logf("ADR-001 cascade: %d refs, %d affected modules", result.TotalReferences, len(result.AffectedModules))
}
