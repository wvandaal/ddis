package tests

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/wvandaal/ddis/internal/bundle"
	"github.com/wvandaal/ddis/internal/storage"
)

// =============================================================================
// INV-BUNDLE-SELF-CONTAINED: Bundle includes constitution + modules + stubs
// =============================================================================

func TestBundleSelfContained(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	// Get available domains
	domains, err := storage.GetModuleDomains(db, specID)
	if err != nil {
		t.Fatalf("get domains: %v", err)
	}
	if len(domains) == 0 {
		t.Skip("no domains in spec")
	}

	for _, domain := range domains {
		t.Run(domain, func(t *testing.T) {
			result, err := bundle.Assemble(db, specID, domain, bundle.Options{})
			if err != nil {
				t.Fatalf("assemble: %v", err)
			}

			// Constitution must be present
			if result.ConstitutionLines == 0 {
				t.Error("bundle must include constitution (0 lines)")
			}

			// Must have at least one module for the domain
			if len(result.Modules) == 0 {
				t.Errorf("bundle for domain %q has no modules", domain)
			}

			// Total lines must be the sum of parts
			expectedTotal := result.ConstitutionLines + result.ModuleLines + result.InterfaceLines
			if result.TotalLines != expectedTotal {
				t.Errorf("total lines mismatch: got %d, want %d (const=%d + mod=%d + iface=%d)",
					result.TotalLines, expectedTotal,
					result.ConstitutionLines, result.ModuleLines, result.InterfaceLines)
			}

			// Content must be non-empty
			if result.Content == "" {
				t.Error("content is empty")
			}

			// Slices must be non-nil
			if result.Modules == nil {
				t.Error("modules slice is nil")
			}
			if result.InterfaceElements == nil {
				t.Error("interface_elements slice is nil")
			}
		})
	}
}

// =============================================================================
// INV-BUNDLE-BUDGET: Budget is computed correctly
// =============================================================================

func TestBundleBudget(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	domains, err := storage.GetModuleDomains(db, specID)
	if err != nil {
		t.Fatalf("get domains: %v", err)
	}
	if len(domains) == 0 {
		t.Skip("no domains in spec")
	}

	domain := domains[0]
	result, err := bundle.Assemble(db, specID, domain, bundle.Options{})
	if err != nil {
		t.Fatalf("assemble: %v", err)
	}

	// Budget ceiling and target must be positive
	if result.Budget.Ceiling <= 0 {
		t.Errorf("budget ceiling must be positive: %d", result.Budget.Ceiling)
	}
	if result.Budget.Target <= 0 {
		t.Errorf("budget target must be positive: %d", result.Budget.Target)
	}

	// Usage must match total/ceiling
	expectedUsage := float64(result.TotalLines) / float64(result.Budget.Ceiling)
	if diff := result.Budget.Usage - expectedUsage; diff > 0.001 || diff < -0.001 {
		t.Errorf("budget usage mismatch: got %.4f, want %.4f", result.Budget.Usage, expectedUsage)
	}

	// Target should be less than ceiling
	if result.Budget.Target > result.Budget.Ceiling {
		t.Errorf("target (%d) > ceiling (%d)", result.Budget.Target, result.Budget.Ceiling)
	}
}

// =============================================================================
// JSON output is valid and round-trips
// =============================================================================

func TestBundleJSONValid(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	domains, err := storage.GetModuleDomains(db, specID)
	if err != nil {
		t.Fatalf("get domains: %v", err)
	}
	if len(domains) == 0 {
		t.Skip("no domains in spec")
	}

	result, err := bundle.Assemble(db, specID, domains[0], bundle.Options{AsJSON: true})
	if err != nil {
		t.Fatalf("assemble: %v", err)
	}

	out, err := bundle.Render(result, true, false)
	if err != nil {
		t.Fatalf("render JSON: %v", err)
	}

	var parsed bundle.BundleResult
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("invalid JSON output: %v\nOutput:\n%s", err, out[:min(len(out), 500)])
	}

	if parsed.Domain != result.Domain {
		t.Errorf("domain mismatch: got %s, want %s", parsed.Domain, result.Domain)
	}
	if parsed.TotalLines != result.TotalLines {
		t.Errorf("total lines mismatch: got %d, want %d", parsed.TotalLines, result.TotalLines)
	}
}

// =============================================================================
// Human-readable output has expected sections
// =============================================================================

func TestBundleHumanReadable(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	domains, err := storage.GetModuleDomains(db, specID)
	if err != nil {
		t.Fatalf("get domains: %v", err)
	}
	if len(domains) == 0 {
		t.Skip("no domains in spec")
	}

	result, err := bundle.Assemble(db, specID, domains[0], bundle.Options{})
	if err != nil {
		t.Fatalf("assemble: %v", err)
	}

	out, err := bundle.Render(result, false, false)
	if err != nil {
		t.Fatalf("render: %v", err)
	}

	if !strings.Contains(out, "Domain Bundle:") {
		t.Error("missing 'Domain Bundle:' header")
	}
	if !strings.Contains(out, "Constitution:") {
		t.Error("missing 'Constitution:' in output")
	}
	if !strings.Contains(out, "Modules") {
		t.Error("missing 'Modules' in output")
	}
	if !strings.Contains(out, "ceiling") {
		t.Error("missing 'ceiling' in output")
	}
}

// =============================================================================
// Content-only mode returns just the content
// =============================================================================

func TestBundleContentOnly(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	domains, err := storage.GetModuleDomains(db, specID)
	if err != nil {
		t.Fatalf("get domains: %v", err)
	}
	if len(domains) == 0 {
		t.Skip("no domains in spec")
	}

	result, err := bundle.Assemble(db, specID, domains[0], bundle.Options{ContentOnly: true})
	if err != nil {
		t.Fatalf("assemble: %v", err)
	}

	out, err := bundle.Render(result, false, true)
	if err != nil {
		t.Fatalf("render: %v", err)
	}

	// Content-only should NOT have the summary header
	if strings.Contains(out, "Domain Bundle:") {
		t.Error("content-only mode should not include summary header")
	}

	// But should have actual content
	if len(out) == 0 {
		t.Error("content-only output is empty")
	}
}

// =============================================================================
// Determinism: same input produces same output
// =============================================================================

func TestBundleDeterminism(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	domains, err := storage.GetModuleDomains(db, specID)
	if err != nil {
		t.Fatalf("get domains: %v", err)
	}
	if len(domains) == 0 {
		t.Skip("no domains in spec")
	}

	result1, err := bundle.Assemble(db, specID, domains[0], bundle.Options{})
	if err != nil {
		t.Fatalf("first assemble: %v", err)
	}
	out1, _ := bundle.Render(result1, true, false)

	result2, err := bundle.Assemble(db, specID, domains[0], bundle.Options{})
	if err != nil {
		t.Fatalf("second assemble: %v", err)
	}
	out2, _ := bundle.Render(result2, true, false)

	if out1 != out2 {
		t.Error("non-deterministic output between two runs")
	}
}

// =============================================================================
// Empty domain returns empty modules but no error
// =============================================================================

func TestBundleEmptyDomain(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result, err := bundle.Assemble(db, specID, "nonexistent-domain-xyz", bundle.Options{})
	if err != nil {
		t.Fatalf("assemble for nonexistent domain should not error: %v", err)
	}

	if len(result.Modules) != 0 {
		t.Errorf("expected 0 modules for nonexistent domain, got %d", len(result.Modules))
	}

	// Constitution should still be present
	if result.ConstitutionLines == 0 {
		t.Error("constitution should still be present even for nonexistent domain")
	}
}
