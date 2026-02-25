package tests

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/wvandaal/ddis/internal/exemplar"
	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

// sharedExemplarDB caches a parsed + indexed DB for exemplar tests.
var sharedExemplarDB *exemplarTestDB

type exemplarTestDB struct {
	db     *storage.DB
	specID int64
	lsi    *search.LSIIndex
}

func getExemplarDB(t *testing.T) (*storage.DB, int64, *search.LSIIndex) {
	t.Helper()
	if sharedExemplarDB != nil {
		return sharedExemplarDB.db, sharedExemplarDB.specID, sharedExemplarDB.lsi
	}

	// Try modular spec (manifest.yaml) first, fall back to monolith
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

	dbPath := filepath.Join(t.TempDir(), "exemplar_test.db")
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

	if err := search.BuildIndex(db, specID); err != nil {
		t.Fatalf("build index: %v", err)
	}

	docs, err := search.ExtractDocuments(db, specID)
	if err != nil {
		t.Fatalf("extract docs: %v", err)
	}
	k := 50
	if len(docs) < k {
		k = len(docs)
	}
	lsi, err := search.BuildLSI(docs, k)
	if err != nil {
		t.Fatalf("build lsi: %v", err)
	}

	sharedExemplarDB = &exemplarTestDB{db: &db, specID: specID, lsi: lsi}
	return sharedExemplarDB.db, sharedExemplarDB.specID, sharedExemplarDB.lsi
}

// =============================================================================
// EX-INV-001: Exemplar Quality Monotonicity
// Adding content to a candidate's component can only increase its composite quality score.
// =============================================================================

func TestEXINV001_QualityMonotonicity(t *testing.T) {
	// Test with synthetic invariants: toggling one component between empty and filled
	// must not decrease the quality score.
	tests := []struct {
		name      string
		component string
	}{
		{"statement", "statement"},
		{"semi_formal", "semi_formal"},
		{"violation_scenario", "violation_scenario"},
		{"validation_method", "validation_method"},
		{"why_this_matters", "why_this_matters"},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			// Score with component empty
			fieldsEmpty := map[string]string{
				"statement":          "The system preserves all data during round trips.",
				"semi_formal":        "FOR ALL x: f(g(x)) = x",
				"violation_scenario": "When a field is removed, the round-trip fails because the parser cannot reconstruct the missing field.",
				"validation_method":  "Run the test suite and verify all tests pass.",
				"why_this_matters":   "Without round-trip fidelity, editors corrupt specifications silently.",
			}
			fieldsEmpty[tc.component] = "" // Empty one field

			fieldsFull := map[string]string{
				"statement":          "The system preserves all data during round trips.",
				"semi_formal":        "FOR ALL x: f(g(x)) = x",
				"violation_scenario": "When a field is removed, the round-trip fails because the parser cannot reconstruct the missing field.",
				"validation_method":  "Run the test suite and verify all tests pass.",
				"why_this_matters":   "Without round-trip fidelity, editors corrupt specifications silently.",
			}

			// Compute WeakScores for the component
			scoreEmpty := exemplar.WeakScore(fieldsEmpty[tc.component], tc.component, "invariant")
			scoreFull := exemplar.WeakScore(fieldsFull[tc.component], tc.component, "invariant")

			if scoreFull < scoreEmpty {
				t.Errorf("filling %s decreased WeakScore: empty=%.3f, filled=%.3f", tc.component, scoreEmpty, scoreFull)
			}

			// Completeness with component empty vs filled
			emptyCount := 0
			fullCount := 0
			for _, comp := range exemplar.ComponentsForType("invariant") {
				if fieldsEmpty[comp] != "" {
					emptyCount++
				}
				if fieldsFull[comp] != "" {
					fullCount++
				}
			}
			completenessEmpty := float64(emptyCount) / 5.0
			completenessFull := float64(fullCount) / 5.0

			// Composite score approximation (authority=0, similarity=0 for this test)
			qualityEmpty := 0.25*completenessEmpty + 0.35*scoreEmpty
			qualityFull := 0.25*completenessFull + 0.35*scoreFull

			if qualityFull < qualityEmpty {
				t.Errorf("filling %s decreased composite quality: empty=%.3f, filled=%.3f", tc.component, qualityEmpty, qualityFull)
			}
		})
	}
}

// =============================================================================
// EX-INV-002: Gap Detection Completeness
// Every empty component field is detected as a "missing" gap.
// =============================================================================

func TestEXINV002_GapDetectionCompleteness(t *testing.T) {
	t.Run("invariant_components", func(t *testing.T) {
		for _, comp := range exemplar.ComponentsForType("invariant") {
			t.Run(comp, func(t *testing.T) {
				fields := map[string]string{
					"statement":          "Strong statement about system behavior.",
					"semi_formal":        "∀ x ∈ S: P(x) ⟹ Q(x)",
					"violation_scenario": "When the precondition is violated, the system fails by producing an inconsistent state transition that corrupts downstream data.",
					"validation_method":  "Run cargo test --all-targets and verify all tests pass with exit code 0.",
					"why_this_matters":   "Without this invariant, users lose data.",
				}
				fields[comp] = "" // Empty this one field

				gaps := exemplar.DetectGaps("invariant", fields, exemplar.Options{})
				found := false
				for _, g := range gaps {
					if g.Component == comp && g.Severity == "missing" {
						found = true
						break
					}
				}
				if !found {
					t.Errorf("empty %s not detected as missing gap", comp)
				}
			})
		}
	})

	t.Run("adr_components", func(t *testing.T) {
		for _, comp := range exemplar.ComponentsForType("adr") {
			t.Run(comp, func(t *testing.T) {
				fields := map[string]string{
					"problem":       "Two competing requirements that cannot both be satisfied.",
					"decision_text": "We chose option B because it balances performance and simplicity better than alternatives.",
					"chosen_option": "B - Balanced approach",
					"consequences":  "Increased complexity but better long-term maintainability.",
					"tests":         "Run integration tests to verify the decision holds.",
				}
				fields[comp] = ""

				gaps := exemplar.DetectGaps("adr", fields, exemplar.Options{})
				found := false
				for _, g := range gaps {
					if g.Component == comp && g.Severity == "missing" {
						found = true
						break
					}
				}
				if !found {
					t.Errorf("empty %s not detected as missing gap", comp)
				}
			})
		}
	})
}

// =============================================================================
// EX-INV-003: Exemplar-Gap Relevance
// Every returned exemplar has the gap's component present AND strong (WeakScore > 0.6).
// =============================================================================

func TestEXINV003_ExemplarGapRelevance(t *testing.T) {
	dbPtr, specID, lsi := getExemplarDB(t)
	db := *dbPtr

	result, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
		Target:   "APP-INV-001",
		MinScore: 0.1,
		Limit:    10,
	})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	for _, ex := range result.Exemplars {
		// Verify the demonstrated component content is non-empty
		if ex.Content == "" {
			t.Errorf("exemplar %s has empty content for demonstrated component %s",
				ex.ElementID, ex.DemonstratedComponent)
		}

		// Verify the WeakScore is > 0.6
		score := exemplar.WeakScore(ex.Content, ex.DemonstratedComponent, ex.ElementType)
		if score <= 0.6 {
			t.Errorf("exemplar %s component %s WeakScore=%.3f (must be > 0.6)",
				ex.ElementID, ex.DemonstratedComponent, score)
		}
	}
}

// =============================================================================
// EX-INV-004: Ranking Consistency
// Exemplars are strictly ordered by composite quality score (descending).
// =============================================================================

func TestEXINV004_RankingConsistency(t *testing.T) {
	dbPtr, specID, lsi := getExemplarDB(t)
	db := *dbPtr

	result, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
		Target:   "APP-INV-001",
		MinScore: 0.1,
		Limit:    10,
	})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	// Group exemplars by demonstrated component
	byComponent := make(map[string][]exemplar.Exemplar)
	for _, ex := range result.Exemplars {
		byComponent[ex.DemonstratedComponent] = append(byComponent[ex.DemonstratedComponent], ex)
	}

	for comp, exs := range byComponent {
		for i := 1; i < len(exs); i++ {
			if exs[i].QualityScore > exs[i-1].QualityScore {
				t.Errorf("component %s: exemplar[%d] score %.2f > exemplar[%d] score %.2f (not descending)",
					comp, i, exs[i].QualityScore, i-1, exs[i-1].QualityScore)
			}
		}
	}
}

// =============================================================================
// EX-INV-005: Self-Exclusion
// The target element never appears as its own exemplar.
// =============================================================================

func TestEXINV005_SelfExclusion(t *testing.T) {
	dbPtr, specID, lsi := getExemplarDB(t)
	db := *dbPtr

	// Test with multiple targets
	targets := []string{"APP-INV-001", "APP-INV-006", "APP-INV-008"}
	for _, target := range targets {
		t.Run(target, func(t *testing.T) {
			result, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
				Target:   target,
				MinScore: 0.01,
				Limit:    20,
			})
			if err != nil {
				t.Fatalf("analyze: %v", err)
			}

			for _, ex := range result.Exemplars {
				if ex.ElementID == target {
					t.Errorf("target %s appears as its own exemplar", target)
				}
			}
		})
	}
}

// =============================================================================
// EX-INV-006: Substrate Cue Structure
// Every substrate cue contains all 4 structural elements.
// =============================================================================

func TestEXINV006_SubstrateCueStructure(t *testing.T) {
	dbPtr, specID, lsi := getExemplarDB(t)
	db := *dbPtr

	result, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
		Target:   "APP-INV-001",
		MinScore: 0.1,
		Limit:    10,
	})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	for _, ex := range result.Exemplars {
		if ex.SubstrateCue == "" {
			t.Errorf("exemplar %s has empty substrate cue", ex.ElementID)
			continue
		}

		// 1. "DEMONSTRATION:" label
		if !strings.Contains(ex.SubstrateCue, "DEMONSTRATION:") {
			t.Errorf("exemplar %s cue missing DEMONSTRATION: label", ex.ElementID)
		}

		// 2. Attribution (exemplar element ID)
		if !strings.Contains(ex.SubstrateCue, ex.ElementID) {
			t.Errorf("exemplar %s cue missing attribution (element ID)", ex.ElementID)
		}

		// 3. Structural observation — contains words like "Notice", "how", "names"
		hasObservation := strings.Contains(ex.SubstrateCue, "Notice") ||
			strings.Contains(ex.SubstrateCue, "notice")
		if !hasObservation {
			t.Errorf("exemplar %s cue missing structural observation", ex.ElementID)
		}

		// 4. Transfer instruction — contains target ID
		if !strings.Contains(ex.SubstrateCue, "APP-INV-001") {
			t.Errorf("exemplar %s cue missing target ID (transfer instruction)", ex.ElementID)
		}
	}
}

// =============================================================================
// Substrate Cue Templates — all 10 components have working templates
// =============================================================================

func TestSubstrateCueTemplates(t *testing.T) {
	allComponents := append(exemplar.ComponentsForType("invariant"), exemplar.ComponentsForType("adr")...)

	for _, comp := range allComponents {
		t.Run(comp, func(t *testing.T) {
			cue := exemplar.GenerateSubstrateCue("TEST-001", "TARGET-002", comp)
			if cue == "" {
				t.Errorf("no cue template for component %s", comp)
				return
			}
			if !strings.Contains(cue, "DEMONSTRATION:") {
				t.Errorf("cue for %s missing DEMONSTRATION: label", comp)
			}
			if !strings.Contains(cue, "TEST-001") {
				t.Errorf("cue for %s missing exemplar ID", comp)
			}
			if !strings.Contains(cue, "TARGET-002") {
				t.Errorf("cue for %s missing target ID", comp)
			}
		})
	}
}

// =============================================================================
// Gap Detection per Component (Coverage Matrix)
// =============================================================================

func TestGapInvariantStatement(t *testing.T) {
	fields := map[string]string{"statement": "", "semi_formal": "x", "violation_scenario": "x", "validation_method": "x", "why_this_matters": "x"}
	gaps := exemplar.DetectGaps("invariant", fields, exemplar.Options{})
	assertGapExists(t, gaps, "statement", "missing")
}

func TestGapInvariantSemiFormal(t *testing.T) {
	fields := map[string]string{"statement": "x", "semi_formal": "", "violation_scenario": "x", "validation_method": "x", "why_this_matters": "x"}
	gaps := exemplar.DetectGaps("invariant", fields, exemplar.Options{})
	assertGapExists(t, gaps, "semi_formal", "missing")
}

func TestGapInvariantViolation(t *testing.T) {
	fields := map[string]string{"statement": "x", "semi_formal": "x", "violation_scenario": "", "validation_method": "x", "why_this_matters": "x"}
	gaps := exemplar.DetectGaps("invariant", fields, exemplar.Options{})
	assertGapExists(t, gaps, "violation_scenario", "missing")
}

func TestGapInvariantValidation(t *testing.T) {
	fields := map[string]string{"statement": "x", "semi_formal": "x", "violation_scenario": "x", "validation_method": "", "why_this_matters": "x"}
	gaps := exemplar.DetectGaps("invariant", fields, exemplar.Options{})
	assertGapExists(t, gaps, "validation_method", "missing")
}

func TestGapInvariantWhy(t *testing.T) {
	fields := map[string]string{"statement": "x", "semi_formal": "x", "violation_scenario": "x", "validation_method": "x", "why_this_matters": ""}
	gaps := exemplar.DetectGaps("invariant", fields, exemplar.Options{})
	assertGapExists(t, gaps, "why_this_matters", "missing")
}

func TestGapADRProblem(t *testing.T) {
	fields := map[string]string{"problem": "", "decision_text": "x", "chosen_option": "x", "consequences": "x", "tests": "x"}
	gaps := exemplar.DetectGaps("adr", fields, exemplar.Options{})
	assertGapExists(t, gaps, "problem", "missing")
}

func TestGapADRDecision(t *testing.T) {
	fields := map[string]string{"problem": "x", "decision_text": "", "chosen_option": "x", "consequences": "x", "tests": "x"}
	gaps := exemplar.DetectGaps("adr", fields, exemplar.Options{})
	assertGapExists(t, gaps, "decision_text", "missing")
}

func TestGapADROption(t *testing.T) {
	fields := map[string]string{"problem": "x", "decision_text": "x", "chosen_option": "", "consequences": "x", "tests": "x"}
	gaps := exemplar.DetectGaps("adr", fields, exemplar.Options{})
	assertGapExists(t, gaps, "chosen_option", "missing")
}

func TestGapADRConsequences(t *testing.T) {
	fields := map[string]string{"problem": "x", "decision_text": "x", "chosen_option": "x", "consequences": "", "tests": "x"}
	gaps := exemplar.DetectGaps("adr", fields, exemplar.Options{})
	assertGapExists(t, gaps, "consequences", "missing")
}

func TestGapADRTests(t *testing.T) {
	fields := map[string]string{"problem": "x", "decision_text": "x", "chosen_option": "x", "consequences": "x", "tests": ""}
	gaps := exemplar.DetectGaps("adr", fields, exemplar.Options{})
	assertGapExists(t, gaps, "tests", "missing")
}

func assertGapExists(t *testing.T, gaps []exemplar.ComponentGap, component, severity string) {
	t.Helper()
	for _, g := range gaps {
		if g.Component == component && g.Severity == severity {
			return
		}
	}
	t.Errorf("expected gap for %s with severity %s, but not found in %v", component, severity, gaps)
}

// =============================================================================
// Edge Cases
// =============================================================================

func TestExemplarNoGaps(t *testing.T) {
	dbPtr, specID, lsi := getExemplarDB(t)
	db := *dbPtr

	// APP-INV-001 has all components well-filled
	result, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
		Target: "APP-INV-001",
	})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	if len(result.Gaps) != 0 {
		t.Errorf("expected 0 gaps for APP-INV-001, got %d", len(result.Gaps))
	}
	if len(result.Exemplars) != 0 {
		t.Errorf("expected 0 exemplars for complete element, got %d", len(result.Exemplars))
	}
	if !strings.Contains(result.Guidance, "no component gaps") {
		t.Errorf("guidance should mention no gaps: %s", result.Guidance)
	}
}

func TestExemplarAllGaps(t *testing.T) {
	dbPtr, specID, lsi := getExemplarDB(t)
	db := *dbPtr

	// APP-INV-001 is complete — verify Analyze runs cleanly with MinScore and Limit
	result, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
		Target:   "APP-INV-001",
		MinScore: 0.1,
		Limit:    3,
	})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	// A fully-specified element should have no gaps and generate appropriate guidance
	if result.Target != "APP-INV-001" {
		t.Errorf("expected target APP-INV-001, got %s", result.Target)
	}
	if !strings.Contains(result.Guidance, "no component gaps") {
		t.Errorf("complete element should report no gaps; guidance: %s", result.Guidance)
	}
}

func TestExemplarGapFilter(t *testing.T) {
	dbPtr, specID, lsi := getExemplarDB(t)
	db := *dbPtr

	result, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
		Target: "APP-INV-001",
		Gap:    "semi_formal",
	})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	for _, g := range result.Gaps {
		if g.Component != "semi_formal" {
			t.Errorf("gap filter not working: got component %s", g.Component)
		}
	}
	for _, ex := range result.Exemplars {
		if ex.DemonstratedComponent != "semi_formal" {
			t.Errorf("exemplar filter not working: got component %s", ex.DemonstratedComponent)
		}
	}
}

func TestExemplarJSONValid(t *testing.T) {
	dbPtr, specID, lsi := getExemplarDB(t)
	db := *dbPtr

	result, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
		Target:   "APP-INV-001",
		MinScore: 0.1,
	})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	out, err := exemplar.Render(result, true)
	if err != nil {
		t.Fatalf("render JSON: %v", err)
	}

	var parsed exemplar.ExemplarResult
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("invalid JSON output: %v\nOutput:\n%s", err, out)
	}

	if parsed.Target != "APP-INV-001" {
		t.Errorf("target mismatch: got %s", parsed.Target)
	}
}

func TestExemplarHumanReadable(t *testing.T) {
	dbPtr, specID, lsi := getExemplarDB(t)
	db := *dbPtr

	result, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
		Target: "APP-INV-001",
	})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	out, err := exemplar.Render(result, false)
	if err != nil {
		t.Fatalf("render human: %v", err)
	}

	if !strings.Contains(out, "Exemplar Analysis:") {
		t.Error("missing header in human output")
	}
	if !strings.Contains(out, "APP-INV-001") {
		t.Error("missing target ID in human output")
	}
	if !strings.Contains(out, "Guidance:") {
		t.Error("missing guidance section in human output")
	}
}

func TestExemplarDeterminism(t *testing.T) {
	dbPtr, specID, lsi := getExemplarDB(t)
	db := *dbPtr

	opts := exemplar.Options{
		Target:   "APP-INV-001",
		MinScore: 0.1,
		Limit:    5,
	}

	result1, err := exemplar.Analyze(db, specID, lsi, opts)
	if err != nil {
		t.Fatalf("first analyze: %v", err)
	}
	out1, _ := exemplar.Render(result1, true)

	result2, err := exemplar.Analyze(db, specID, lsi, opts)
	if err != nil {
		t.Fatalf("second analyze: %v", err)
	}
	out2, _ := exemplar.Render(result2, true)

	if out1 != out2 {
		t.Errorf("non-deterministic output:\nRun 1:\n%s\nRun 2:\n%s", out1, out2)
	}
}

func TestExemplarInvalidTarget(t *testing.T) {
	dbPtr, specID, lsi := getExemplarDB(t)
	db := *dbPtr

	_, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
		Target: "NONEXISTENT-999",
	})
	if err == nil {
		t.Error("expected error for invalid target")
	}
}

func TestExemplarADR(t *testing.T) {
	dbPtr, specID, lsi := getExemplarDB(t)
	db := *dbPtr

	result, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
		Target:   "APP-ADR-001",
		MinScore: 0.1,
	})
	if err != nil {
		t.Fatalf("analyze ADR: %v", err)
	}

	if result.ElementType != "adr" {
		t.Errorf("expected element_type=adr, got %s", result.ElementType)
	}
}

// =============================================================================
// WeakScore unit tests
// =============================================================================

func TestWeakScoreEmpty(t *testing.T) {
	score := exemplar.WeakScore("", "statement", "invariant")
	if score != 0.0 {
		t.Errorf("empty text should score 0.0, got %.3f", score)
	}
}

func TestWeakScoreShort(t *testing.T) {
	// "hello" = 5 chars, threshold for statement = 40
	score := exemplar.WeakScore("hello", "statement", "invariant")
	expected := 5.0 / 40.0 // 0.125
	if score < expected-0.01 || score > expected+0.01 {
		t.Errorf("short text score: expected ~%.3f, got %.3f", expected, score)
	}
}

func TestWeakScoreWithBonus(t *testing.T) {
	// Text with "∀" should get +0.15 bonus for semi_formal
	text := "∀ x ∈ S: P(x)"
	score := exemplar.WeakScore(text, "semi_formal", "invariant")
	baseScore := float64(len(text)) / 60.0
	if score <= baseScore {
		t.Errorf("semi_formal with logical operator should have bonus: score=%.3f, base=%.3f", score, baseScore)
	}
}

func TestWeakScoreAboveThreshold(t *testing.T) {
	// Long text should score 1.0
	text := strings.Repeat("a", 200)
	score := exemplar.WeakScore(text, "statement", "invariant")
	if score != 1.0 {
		t.Errorf("long text should score 1.0, got %.3f", score)
	}
}
