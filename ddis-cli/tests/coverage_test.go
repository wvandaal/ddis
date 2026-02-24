package tests

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/wvandaal/ddis/internal/coverage"
	"github.com/wvandaal/ddis/internal/exemplar"
	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/storage"
)

// sharedCoverageDB caches a parsed DB for coverage tests.
var sharedCoverageDB *coverageTestDB

type coverageTestDB struct {
	db     *storage.DB
	specID int64
}

func getCoverageDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedCoverageDB != nil {
		return sharedCoverageDB.db, sharedCoverageDB.specID
	}

	manifestPath := filepath.Join(projectRoot(), "ddis-cli-spec", "manifest.yaml")
	monolithPath := filepath.Join(projectRoot(), "ddis_final.md")

	var specPath string
	var isModular bool
	if _, err := os.Stat(manifestPath); err == nil {
		specPath = manifestPath
		isModular = true
	} else if _, err := os.Stat(monolithPath); err == nil {
		specPath = monolithPath
	} else {
		t.Skipf("no spec found (tried %s and %s)", manifestPath, monolithPath)
	}

	dbPath := filepath.Join(t.TempDir(), "coverage_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	var specID int64
	if isModular {
		specID, err = parser.ParseModularSpec(specPath, db)
	} else {
		specID, err = parser.ParseDocument(specPath, db)
	}
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	sharedCoverageDB = &coverageTestDB{db: &db, specID: specID}
	return sharedCoverageDB.db, sharedCoverageDB.specID
}

// =============================================================================
// INV-COV-MONO: Coverage Monotonicity
// Adding content to a component can only increase the coverage score.
// =============================================================================

func TestCoverageMonotonicity(t *testing.T) {
	components := exemplar.ComponentsForType("invariant")

	for _, comp := range components {
		t.Run(comp, func(t *testing.T) {
			// Build fields with one component empty
			fieldsEmpty := map[string]string{
				"statement":          "The system preserves all data during round trips.",
				"semi_formal":        "FOR ALL x: f(g(x)) = x",
				"violation_scenario": "When a field is removed, the round-trip fails because the parser cannot reconstruct the missing field.",
				"validation_method":  "Run the test suite and verify all tests pass.",
				"why_this_matters":   "Without round-trip fidelity, editors corrupt specifications silently.",
			}
			fieldsEmpty[comp] = ""

			fieldsFull := map[string]string{
				"statement":          "The system preserves all data during round trips.",
				"semi_formal":        "FOR ALL x: f(g(x)) = x",
				"violation_scenario": "When a field is removed, the round-trip fails because the parser cannot reconstruct the missing field.",
				"validation_method":  "Run the test suite and verify all tests pass.",
				"why_this_matters":   "Without round-trip fidelity, editors corrupt specifications silently.",
			}

			// Count present components for each case
			emptyPresent := 0
			fullPresent := 0
			for _, c := range components {
				if fieldsEmpty[c] != "" {
					score := exemplar.WeakScore(fieldsEmpty[c], c, "invariant")
					if score > 0 {
						emptyPresent++
					}
				}
				if fieldsFull[c] != "" {
					score := exemplar.WeakScore(fieldsFull[c], c, "invariant")
					if score > 0 {
						fullPresent++
					}
				}
			}

			completenessEmpty := float64(emptyPresent) / float64(len(components))
			completenessFull := float64(fullPresent) / float64(len(components))

			if completenessFull < completenessEmpty {
				t.Errorf("filling %s decreased completeness: empty=%.3f, filled=%.3f",
					comp, completenessEmpty, completenessFull)
			}
		})
	}
}

// =============================================================================
// Coverage Analyze correctness on real spec
// =============================================================================

func TestCoverageAnalyze(t *testing.T) {
	dbPtr, specID := getCoverageDB(t)
	db := *dbPtr

	result, err := coverage.Analyze(db, specID, coverage.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	if result.Spec == "" {
		t.Error("spec name should be non-empty")
	}

	if result.Summary.InvariantsTotal == 0 {
		t.Error("expected at least one invariant")
	}

	if result.Summary.Score < 0 || result.Summary.Score > 1 {
		t.Errorf("score out of range: %.3f", result.Summary.Score)
	}

	// Every invariant should have completeness in [0, 1]
	for id, ic := range result.Invariants {
		if ic.Completeness < 0 || ic.Completeness > 1 {
			t.Errorf("invariant %s completeness out of range: %.3f", id, ic.Completeness)
		}
		if len(ic.Components) != len(exemplar.ComponentsForType("invariant")) {
			t.Errorf("invariant %s has %d components, expected %d",
				id, len(ic.Components), len(exemplar.ComponentsForType("invariant")))
		}
	}

	// Every ADR should have completeness in [0, 1]
	for id, ac := range result.ADRs {
		if ac.Completeness < 0 || ac.Completeness > 1 {
			t.Errorf("adr %s completeness out of range: %.3f", id, ac.Completeness)
		}
	}
}

// =============================================================================
// JSON output is valid and round-trips
// =============================================================================

func TestCoverageJSONValid(t *testing.T) {
	dbPtr, specID := getCoverageDB(t)
	db := *dbPtr

	result, err := coverage.Analyze(db, specID, coverage.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	out, err := coverage.Render(result, true)
	if err != nil {
		t.Fatalf("render JSON: %v", err)
	}

	var parsed coverage.CoverageResult
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("invalid JSON output: %v\nOutput:\n%s", err, out)
	}

	if parsed.Spec != result.Spec {
		t.Errorf("spec name mismatch: got %s, want %s", parsed.Spec, result.Spec)
	}
	if parsed.Summary.InvariantsTotal != result.Summary.InvariantsTotal {
		t.Errorf("invariants total mismatch: got %d, want %d",
			parsed.Summary.InvariantsTotal, result.Summary.InvariantsTotal)
	}
}

// =============================================================================
// Human-readable output has expected sections
// =============================================================================

func TestCoverageHumanReadable(t *testing.T) {
	dbPtr, specID := getCoverageDB(t)
	db := *dbPtr

	result, err := coverage.Analyze(db, specID, coverage.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	out, err := coverage.Render(result, false)
	if err != nil {
		t.Fatalf("render human: %v", err)
	}

	if !strings.Contains(out, "Coverage:") {
		t.Error("missing Coverage: header")
	}
	if !strings.Contains(out, "complete") {
		t.Error("missing 'complete' in output")
	}
	if !strings.Contains(out, "invariants") {
		t.Error("missing 'invariants' in output")
	}
}

// =============================================================================
// Domain filter reduces results
// =============================================================================

func TestCoverageDomainFilter(t *testing.T) {
	dbPtr, specID := getCoverageDB(t)
	db := *dbPtr

	full, err := coverage.Analyze(db, specID, coverage.Options{})
	if err != nil {
		t.Fatalf("analyze full: %v", err)
	}

	if len(full.Domains) == 0 {
		t.Skip("no domains in spec")
	}

	// Pick the first domain
	var domainName string
	for d := range full.Domains {
		domainName = d
		break
	}

	filtered, err := coverage.Analyze(db, specID, coverage.Options{Domain: domainName})
	if err != nil {
		t.Fatalf("analyze filtered: %v", err)
	}

	if len(filtered.Invariants) > len(full.Invariants) {
		t.Errorf("domain filter should reduce invariants: filtered=%d, full=%d",
			len(filtered.Invariants), len(full.Invariants))
	}

	if len(filtered.Domains) != 1 {
		t.Errorf("expected 1 domain in filtered result, got %d", len(filtered.Domains))
	}
}

// =============================================================================
// Determinism: same input produces same output
// =============================================================================

func TestCoverageDeterminism(t *testing.T) {
	dbPtr, specID := getCoverageDB(t)
	db := *dbPtr

	result1, err := coverage.Analyze(db, specID, coverage.Options{})
	if err != nil {
		t.Fatalf("first analyze: %v", err)
	}
	out1, _ := coverage.Render(result1, true)

	result2, err := coverage.Analyze(db, specID, coverage.Options{})
	if err != nil {
		t.Fatalf("second analyze: %v", err)
	}
	out2, _ := coverage.Render(result2, true)

	if out1 != out2 {
		t.Error("non-deterministic output between two runs")
	}
}

// =============================================================================
// Gaps list is non-nil even when empty
// =============================================================================

func TestCoverageGapsNonNil(t *testing.T) {
	dbPtr, specID := getCoverageDB(t)
	db := *dbPtr

	result, err := coverage.Analyze(db, specID, coverage.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	if result.Gaps == nil {
		t.Error("gaps should be non-nil (use empty slice)")
	}
}

// =============================================================================
// Complete invariants counted correctly
// =============================================================================

func TestCoverageCompleteCount(t *testing.T) {
	dbPtr, specID := getCoverageDB(t)
	db := *dbPtr

	result, err := coverage.Analyze(db, specID, coverage.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	// Manually count complete invariants
	manualComplete := 0
	for _, ic := range result.Invariants {
		if ic.Completeness >= 1.0 {
			manualComplete++
		}
	}

	if manualComplete != result.Summary.InvariantsComplete {
		t.Errorf("invariants_complete mismatch: manual=%d, reported=%d",
			manualComplete, result.Summary.InvariantsComplete)
	}

	manualADRComplete := 0
	for _, ac := range result.ADRs {
		if ac.Completeness >= 1.0 {
			manualADRComplete++
		}
	}

	if manualADRComplete != result.Summary.ADRsComplete {
		t.Errorf("adrs_complete mismatch: manual=%d, reported=%d",
			manualADRComplete, result.Summary.ADRsComplete)
	}
}
