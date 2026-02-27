package tests

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/wvandaal/ddis/internal/drift"
	"github.com/wvandaal/ddis/internal/search"
)

// =============================================================================
// TestDriftAnalyze: basic analysis on synthetic spec
// =============================================================================

func TestDriftAnalyze(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	report, err := drift.Analyze(db, specID, drift.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	// DriftReport should have non-nil Details
	if report.ImplDrift.Details == nil {
		t.Error("impl_drift.details should be non-nil")
	}
	if report.IntentDrift.Details == nil {
		t.Error("intent_drift.details should be non-nil")
	}

	// Effective drift should be >= 0
	if report.EffectiveDrift < 0 {
		t.Errorf("effective_drift should be >= 0, got %d", report.EffectiveDrift)
	}

	// Total should equal formula
	expectedTotal := report.ImplDrift.Unspecified + report.ImplDrift.Unimplemented + 2*report.ImplDrift.Contradictions
	if report.ImplDrift.Total != expectedTotal {
		t.Errorf("impl_drift.total=%d != unspecified(%d)+unimplemented(%d)+2*contradictions(%d)=%d",
			report.ImplDrift.Total, report.ImplDrift.Unspecified,
			report.ImplDrift.Unimplemented, report.ImplDrift.Contradictions, expectedTotal)
	}
}

// =============================================================================
// TestDriftQualityBreakdown: verify quality decomposition
// =============================================================================

func TestDriftQualityBreakdown(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	report, err := drift.Analyze(db, specID, drift.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	q := report.QualityBreakdown

	// Correctness = unimplemented + contradictions
	if q.Correctness != report.ImplDrift.Unimplemented+report.ImplDrift.Contradictions {
		t.Errorf("correctness=%d != unimplemented(%d)+contradictions(%d)",
			q.Correctness, report.ImplDrift.Unimplemented, report.ImplDrift.Contradictions)
	}

	// Depth = unspecified
	if q.Depth != report.ImplDrift.Unspecified {
		t.Errorf("depth=%d != unspecified(%d)", q.Depth, report.ImplDrift.Unspecified)
	}

	// Coherence should be >= 0
	if q.Coherence < 0 {
		t.Errorf("coherence should be >= 0, got %d", q.Coherence)
	}
}

// =============================================================================
// TestDriftClassifyDirection: verify direction classification logic
// =============================================================================

func TestDriftClassifyDirection(t *testing.T) {
	tests := []struct {
		name           string
		unspecified    int
		unimplemented  int
		contradictions int
		wantDirection  string
	}{
		{"aligned", 0, 0, 0, "aligned"},
		{"impl-ahead", 5, 0, 0, "impl-ahead"},
		{"spec-ahead", 0, 3, 0, "spec-ahead"},
		{"contradictory", 1, 1, 1, "contradictory"},
		{"mutual", 2, 2, 0, "mutual"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			report := &drift.DriftReport{
				ImplDrift: drift.ImplDrift{
					Unspecified:    tt.unspecified,
					Unimplemented:  tt.unimplemented,
					Contradictions: tt.contradictions,
				},
			}
			c := drift.Classify(report)
			if c.Direction != tt.wantDirection {
				t.Errorf("direction=%q, want %q", c.Direction, tt.wantDirection)
			}
		})
	}
}

// =============================================================================
// TestDriftClassifySeverity: verify severity classification
// =============================================================================

func TestDriftClassifySeverity(t *testing.T) {
	tests := []struct {
		name           string
		contradictions int
		wantSeverity   string
	}{
		{"additive", 0, "additive"},
		{"contradictory", 1, "contradictory"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			report := &drift.DriftReport{
				ImplDrift: drift.ImplDrift{
					Contradictions: tt.contradictions,
				},
			}
			c := drift.Classify(report)
			if c.Severity != tt.wantSeverity {
				t.Errorf("severity=%q, want %q", c.Severity, tt.wantSeverity)
			}
		})
	}
}

// =============================================================================
// TestDriftClassifyIntentionality: verify intentionality classification
// =============================================================================

func TestDriftClassifyIntentionality(t *testing.T) {
	tests := []struct {
		name       string
		planned    int
		wantIntent string
	}{
		{"organic", 0, "organic"},
		{"planned", 2, "planned"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			report := &drift.DriftReport{
				PlannedDivergences: tt.planned,
			}
			c := drift.Classify(report)
			if c.Intentionality != tt.wantIntent {
				t.Errorf("intentionality=%q, want %q", c.Intentionality, tt.wantIntent)
			}
		})
	}
}

// =============================================================================
// TestDriftRenderJSON: JSON output is valid and round-trips
// =============================================================================

func TestDriftRenderJSON(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	report, err := drift.Analyze(db, specID, drift.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	out, err := drift.Render(report, true)
	if err != nil {
		t.Fatalf("render JSON: %v", err)
	}

	var parsed drift.DriftReport
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("invalid JSON: %v\nOutput:\n%s", err, out[:min(len(out), 500)])
	}

	if parsed.ImplDrift.Total != report.ImplDrift.Total {
		t.Errorf("total mismatch: parsed=%d, original=%d",
			parsed.ImplDrift.Total, report.ImplDrift.Total)
	}
	if parsed.EffectiveDrift != report.EffectiveDrift {
		t.Errorf("effective_drift mismatch: parsed=%d, original=%d",
			parsed.EffectiveDrift, report.EffectiveDrift)
	}
}

// =============================================================================
// TestDriftRenderHuman: human output contains expected sections
// =============================================================================

func TestDriftRenderHuman(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	report, err := drift.Analyze(db, specID, drift.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	out, err := drift.Render(report, false)
	if err != nil {
		t.Fatalf("render: %v", err)
	}

	required := []string{
		"Drift Report",
		"Implementation drift:",
		"Intent drift:",
		"Quality breakdown:",
		"Correctness:",
		"Depth:",
		"Coherence:",
		"Direction:",
		"Recommendation:",
	}

	for _, keyword := range required {
		if !strings.Contains(out, keyword) {
			t.Errorf("missing %q in human output", keyword)
		}
	}
}

// =============================================================================
// TestDriftDeterminism: same input produces same output
// =============================================================================

func TestDriftDeterminism(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	report1, err := drift.Analyze(db, specID, drift.Options{})
	if err != nil {
		t.Fatalf("first analyze: %v", err)
	}
	out1, _ := drift.Render(report1, true)

	report2, err := drift.Analyze(db, specID, drift.Options{})
	if err != nil {
		t.Fatalf("second analyze: %v", err)
	}
	out2, _ := drift.Render(report2, true)

	if out1 != out2 {
		t.Error("non-deterministic output between two runs")
	}
}

// =============================================================================
// TestDriftRemediateZero: returns nil when no drift
// =============================================================================

func TestDriftRemediateZero(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	// Remediate on a fresh synthetic DB (may or may not have drift)
	pkg, err := drift.Remediate(db, specID, nil)
	if err != nil {
		t.Fatalf("remediate: %v", err)
	}

	// We verify the function doesn't error and returns a valid structure
	if pkg != nil {
		if pkg.Target == "" {
			t.Error("remediation package has empty target")
		}
		if pkg.ExpectedDrift < 0 {
			t.Error("expected_drift should be >= 0")
		}
	}
}

// =============================================================================
// TestDriftRemediation: returns valid package when drift exists
// =============================================================================

func TestDriftRemediation(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	report, err := drift.Analyze(db, specID, drift.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	if report.EffectiveDrift == 0 {
		t.Skip("no drift to remediate")
	}

	// Build LSI for proxy search
	docs, err := search.ExtractDocuments(db, specID)
	if err != nil {
		t.Fatalf("extract docs: %v", err)
	}
	k := 50
	if len(docs) < k {
		k = len(docs)
	}
	var lsi *search.LSIIndex
	if k > 0 {
		lsi, _ = search.BuildLSI(docs, k)
	}

	pkg, err := drift.Remediate(db, specID, lsi)
	if err != nil {
		t.Fatalf("remediate: %v", err)
	}

	if pkg == nil {
		t.Fatal("expected non-nil remediation package for spec with drift")
	}

	if pkg.Target == "" {
		t.Error("remediation target is empty")
	}
	if pkg.DriftType == "" {
		t.Error("drift_type is empty")
	}
	if len(pkg.Guidance) == 0 {
		t.Error("guidance is empty")
	}
	if pkg.ExpectedDrift != pkg.TotalDrift-1 {
		t.Errorf("expected_drift=%d should equal total_drift(%d)-1",
			pkg.ExpectedDrift, pkg.TotalDrift)
	}
}

// =============================================================================
// TestDriftRenderRemediationNil: nil package produces aligned message
// =============================================================================

func TestDriftRenderRemediationNil(t *testing.T) {
	out, err := drift.RenderRemediation(nil, false)
	if err != nil {
		t.Fatalf("render nil: %v", err)
	}
	if !strings.Contains(out, "aligned") {
		t.Error("nil remediation should mention alignment")
	}

	jsonOut, err := drift.RenderRemediation(nil, true)
	if err != nil {
		t.Fatalf("render nil JSON: %v", err)
	}
	var parsed map[string]interface{}
	if err := json.Unmarshal([]byte(jsonOut), &parsed); err != nil {
		t.Fatalf("invalid JSON for nil remediation: %v", err)
	}
}

// =============================================================================
// TestDriftWithIntent: intent flag adds intent drift data
// =============================================================================

func TestDriftWithIntent(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	// Without intent
	reportNoIntent, err := drift.Analyze(db, specID, drift.Options{Intent: false})
	if err != nil {
		t.Fatalf("analyze without intent: %v", err)
	}

	// With intent
	reportWithIntent, err := drift.Analyze(db, specID, drift.Options{Intent: true})
	if err != nil {
		t.Fatalf("analyze with intent: %v", err)
	}

	// Without intent, intent drift should be zero
	if reportNoIntent.IntentDrift.Total != 0 {
		t.Errorf("intent drift without --intent should be 0, got %d", reportNoIntent.IntentDrift.Total)
	}

	// With intent, we can't guarantee specific values, but check structure
	if reportWithIntent.IntentDrift.Details == nil {
		t.Error("intent drift details should be non-nil")
	}
}
