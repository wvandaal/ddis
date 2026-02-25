package refine

import (
	"testing"

	"github.com/wvandaal/ddis/internal/autoprompt"
)

// ---------------------------------------------------------------------------
// clampScore
// ---------------------------------------------------------------------------

func TestClampScore(t *testing.T) {
	tests := []struct {
		input    int
		expected int
	}{
		{-5, 0},
		{0, 0},
		{5, 5},
		{10, 10},
		{15, 10},
	}
	for _, tc := range tests {
		got := clampScore(tc.input)
		if got != tc.expected {
			t.Errorf("clampScore(%d) = %d, want %d", tc.input, got, tc.expected)
		}
	}
}

// ---------------------------------------------------------------------------
// selectFocusDimension
// ---------------------------------------------------------------------------

func TestSelectFocusDimension_ClearMinimum(t *testing.T) {
	// Coverage=5, Depth=3, Coherence=7, Completeness=8, Formality=4
	conf := [5]int{5, 3, 7, 8, 4}
	name, score := selectFocusDimension(conf)
	if name != "depth" {
		t.Errorf("expected 'depth' (lowest at 3), got %q", name)
	}
	if score != 3 {
		t.Errorf("expected score 3, got %d", score)
	}
}

func TestSelectFocusDimension_TieBreak(t *testing.T) {
	// All tied at 5. Priority: completeness > coherence > depth > coverage > formality
	conf := [5]int{5, 5, 5, 5, 5}
	name, _ := selectFocusDimension(conf)
	// DimensionPriority = [ConfCompleteness, ConfCoherence, ConfDepth, ConfCoverage, ConfFormality]
	// First in priority with lowest score is completeness (all tied).
	if name != "completeness" {
		t.Errorf("expected 'completeness' for tie-break, got %q", name)
	}
}

func TestSelectFocusDimension_AllZero(t *testing.T) {
	conf := [5]int{0, 0, 0, 0, 0}
	name, score := selectFocusDimension(conf)
	if score != 0 {
		t.Errorf("expected score 0, got %d", score)
	}
	// Completeness wins tie-break (first in DimensionPriority).
	if name != "completeness" {
		t.Errorf("expected 'completeness', got %q", name)
	}
}

func TestSelectFocusDimension_AllMax(t *testing.T) {
	conf := [5]int{10, 10, 10, 10, 10}
	name, score := selectFocusDimension(conf)
	if score != 10 {
		t.Errorf("expected score 10, got %d", score)
	}
	// All tied at 10, completeness wins.
	if name != "completeness" {
		t.Errorf("expected 'completeness', got %q", name)
	}
}

// ---------------------------------------------------------------------------
// selectLimitingFactor
// ---------------------------------------------------------------------------

func TestSelectLimitingFactor(t *testing.T) {
	conf := [5]int{8, 2, 6, 4, 9}
	got := selectLimitingFactor(conf)
	if got != "depth" {
		t.Errorf("expected 'depth' (score 2), got %q", got)
	}
}

// ---------------------------------------------------------------------------
// deriveConfidence
// ---------------------------------------------------------------------------

func TestDeriveConfidence_AllZeros(t *testing.T) {
	sid := &specInternalDrift{}
	conf := deriveConfidence(sid)
	// With zero elements, all dimensions default to 10 (vacuously satisfied).
	for i, name := range autoprompt.DimensionNames {
		if conf[i] != 10 {
			t.Errorf("deriveConfidence empty: %s expected 10, got %d", name, conf[i])
		}
	}
}

func TestDeriveConfidence_WithElements(t *testing.T) {
	sid := &specInternalDrift{
		TotalInvariants:      10,
		CompleteInvariants:   5,
		TotalADRs:            10,
		CompleteADRs:         5,
		TotalElements:        20,
		CompleteElements:     10,
		TotalRefs:            100,
		UnresolvedRefs:       20,
		MissingViolation:     2,
		MissingSemiFormal:    3,
		MissingADRTests:      1,
		WeakChosenOption:     1,
		InvariantsWithFormal: 7,
	}
	conf := deriveConfidence(sid)

	// Coverage = 10 * 10 / 20 = 5
	if conf[autoprompt.ConfCoverage] != 5 {
		t.Errorf("coverage: expected 5, got %d", conf[autoprompt.ConfCoverage])
	}

	// Depth = 10 * 5 / 10 = 5
	if conf[autoprompt.ConfDepth] != 5 {
		t.Errorf("depth: expected 5, got %d", conf[autoprompt.ConfDepth])
	}

	// Coherence = 10 * 80 / 100 = 8
	if conf[autoprompt.ConfCoherence] != 8 {
		t.Errorf("coherence: expected 8, got %d", conf[autoprompt.ConfCoherence])
	}

	// Formality = 10 * 7 / 10 = 7
	if conf[autoprompt.ConfFormality] != 7 {
		t.Errorf("formality: expected 7, got %d", conf[autoprompt.ConfFormality])
	}
}

// ---------------------------------------------------------------------------
// countNonEmpty
// ---------------------------------------------------------------------------

func TestCountNonEmpty(t *testing.T) {
	fields := map[string]string{
		"a": "hello",
		"b": "",
		"c": "world",
		"d": "",
	}
	got := countNonEmpty(fields)
	if got != 2 {
		t.Errorf("expected 2 non-empty, got %d", got)
	}
}

func TestCountNonEmpty_AllEmpty(t *testing.T) {
	fields := map[string]string{"a": "", "b": ""}
	got := countNonEmpty(fields)
	if got != 0 {
		t.Errorf("expected 0, got %d", got)
	}
}

func TestCountNonEmpty_NilMap(t *testing.T) {
	got := countNonEmpty(nil)
	if got != 0 {
		t.Errorf("expected 0 for nil map, got %d", got)
	}
}

// ---------------------------------------------------------------------------
// dimensionToComponent
// ---------------------------------------------------------------------------

func TestDimensionToComponent(t *testing.T) {
	tests := []struct {
		dimension string
		expected  string
	}{
		{"completeness", "statement"},
		{"coherence", "validation_method"},
		{"depth", "violation_scenario"},
		{"coverage", "statement"},
		{"formality", "semi_formal"},
		{"unknown", "statement"},
	}
	for _, tc := range tests {
		got := dimensionToComponent(tc.dimension)
		if got != tc.expected {
			t.Errorf("dimensionToComponent(%q) = %q, want %q", tc.dimension, got, tc.expected)
		}
	}
}

// ---------------------------------------------------------------------------
// dimensionToADRComponent
// ---------------------------------------------------------------------------

func TestDimensionToADRComponent(t *testing.T) {
	tests := []struct {
		dimension string
		expected  string
	}{
		{"completeness", "chosen_option"},
		{"coherence", "consequences"},
		{"depth", "tests"},
		{"coverage", "problem"},
		{"formality", ""},
		{"unknown", ""},
	}
	for _, tc := range tests {
		got := dimensionToADRComponent(tc.dimension)
		if got != tc.expected {
			t.Errorf("dimensionToADRComponent(%q) = %q, want %q", tc.dimension, got, tc.expected)
		}
	}
}

// ---------------------------------------------------------------------------
// finding struct
// ---------------------------------------------------------------------------

func TestFinding_Fields(t *testing.T) {
	f := finding{
		ElementID: "INV-001",
		Component: "violation_scenario",
		Detail:    "missing",
	}
	if f.ElementID != "INV-001" {
		t.Errorf("expected ElementID 'INV-001', got %q", f.ElementID)
	}
}

// ---------------------------------------------------------------------------
// specInternalDrift
// ---------------------------------------------------------------------------

func TestSpecInternalDrift_Defaults(t *testing.T) {
	sid := specInternalDrift{}
	if sid.TotalInvariants != 0 {
		t.Errorf("expected 0, got %d", sid.TotalInvariants)
	}
	if sid.TotalADRs != 0 {
		t.Errorf("expected 0, got %d", sid.TotalADRs)
	}
}

// ---------------------------------------------------------------------------
// weakestElement struct
// ---------------------------------------------------------------------------

func TestWeakestElement_Fields(t *testing.T) {
	w := weakestElement{
		ElementType: "invariant",
		ElementID:   "INV-001",
		Title:       "Test",
		Fields:      map[string]string{"statement": "hello"},
		WeakScore:   0.5,
	}
	if w.ElementType != "invariant" {
		t.Errorf("expected 'invariant', got %q", w.ElementType)
	}
	if w.WeakScore != 0.5 {
		t.Errorf("expected 0.5, got %f", w.WeakScore)
	}
}
