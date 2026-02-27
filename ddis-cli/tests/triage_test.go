//go:build integration

package tests

// ddis:tests APP-INV-063 (issue-discovery linkage)
// ddis:tests APP-INV-064 (spec-before-code gate)
// ddis:tests APP-INV-065 (resolution evidence chain)
// ddis:tests APP-INV-067 (self-bootstrap triage-workflow module)
// ddis:tests APP-INV-068 (fixpoint termination — μ ∈ ℕ³)
// ddis:tests APP-INV-069 (triage monotonic fitness — F(S) ∈ [0,1])
// ddis:tests APP-INV-070 (protocol completeness — self-contained JSON)

import (
	"encoding/json"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/coverage"
	"github.com/wvandaal/ddis/internal/drift"
	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/triage"
	"github.com/wvandaal/ddis/internal/validator"
)

// collectSignals gathers the 6 fitness signals from a real spec database.
// Replicates cli/triage.go collectSignals (private) inline for integration tests.
func collectSignals(t *testing.T, db storage.DB, specID int64) triage.FitnessSignals {
	t.Helper()
	signals := triage.FitnessSignals{}

	report, err := validator.Validate(db, specID, validator.ValidateOptions{})
	if err == nil && report.TotalChecks > 0 {
		signals.Validation = float64(report.Passed) / float64(report.TotalChecks)
	}

	covResult, err := coverage.Analyze(db, specID, coverage.Options{})
	if err == nil && covResult != nil {
		signals.Coverage = covResult.Summary.Score
	}

	driftReport, err := drift.Analyze(db, specID, drift.Options{Report: true})
	if err == nil && driftReport != nil {
		maxDrift := 100
		if driftReport.EffectiveDrift > maxDrift {
			signals.Drift = 1.0
		} else {
			signals.Drift = float64(driftReport.EffectiveDrift) / float64(maxDrift)
		}
	}

	challenges, err := storage.ListChallengeResults(db, specID)
	if err == nil && len(challenges) > 0 {
		confirmed := 0
		for _, c := range challenges {
			if c.Verdict == "confirmed" {
				confirmed++
			}
		}
		signals.ChallengeHP = float64(confirmed) / float64(len(challenges))
	} else {
		signals.ChallengeHP = 1.0
	}

	return signals
}

// readImplStream reads Stream 3 (implementation) events from the CLI spec workspace.
func readImplStream(t *testing.T) []events.Event {
	t.Helper()
	wsRoot := filepath.Join(projectRoot(), "ddis-cli-spec")
	streamPath := events.StreamPath(wsRoot, events.StreamImplementation)
	rawEvts, _ := events.ReadStream(streamPath, events.EventFilters{})
	evts := make([]events.Event, len(rawEvts))
	for i, e := range rawEvts {
		evts[i] = *e
	}
	return evts
}

// ─── TestTriageFitness_RealSpec ───────────────────────────────────────────────

// TestTriageFitness_RealSpec verifies F(S) ∈ [0,1] on the real CLI spec.
// (ddis:tests APP-INV-069)
func TestTriageFitness_RealSpec(t *testing.T) {
	db, specID := getModularDB(t)

	signals := collectSignals(t, db, specID)
	result := triage.ComputeFitness(signals)

	if result.Score < 0.0 || result.Score > 1.0 {
		t.Errorf("F(S) = %f is outside [0,1]", result.Score)
	}

	if result.Score == 0.0 {
		t.Errorf("F(S) = 0.0 for real spec — spec must have some content (score should be > 0)")
	}

	// Verify all weights sum to 1.0 (structural invariant, not data-dependent).
	const totalWeight = triage.WeightValidation +
		triage.WeightCoverage +
		triage.WeightDrift +
		triage.WeightChallengeHP +
		triage.WeightContradictions +
		triage.WeightIssueBacklog

	const epsilon = 1e-9
	if totalWeight < 1.0-epsilon || totalWeight > 1.0+epsilon {
		t.Errorf("signal weights sum to %f, want 1.0", totalWeight)
	}
}

// ─── TestTriageProtocol_RealSpec ─────────────────────────────────────────────

// TestTriageProtocol_RealSpec generates a protocol and verifies its structure.
// (ddis:tests APP-INV-070)
func TestTriageProtocol_RealSpec(t *testing.T) {
	db, specID := getModularDB(t)

	signals := collectSignals(t, db, specID)
	fitness := triage.ComputeFitness(signals)

	evts := readImplStream(t)

	driftReport, _ := drift.Analyze(db, specID, drift.Options{Report: true})
	driftScore := 0
	if driftReport != nil {
		driftScore = driftReport.EffectiveDrift
	}

	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		t.Fatalf("list invariants: %v", err)
	}
	unspecifiedCount := 0
	for _, inv := range invs {
		if inv.Statement == "" {
			unspecifiedCount++
		}
	}

	measure := triage.ComputeMeasure(evts, unspecifiedCount, driftScore)
	protocol := triage.GenerateProtocol(specID, fitness, measure, evts, "manifest.ddis.db")

	if protocol.Version != "1.0" {
		t.Errorf("protocol.Version = %q, want %q", protocol.Version, "1.0")
	}
	if protocol.SpecID != specID {
		t.Errorf("protocol.SpecID = %d, want %d", protocol.SpecID, specID)
	}
	if protocol.SpecID <= 0 {
		t.Errorf("protocol.SpecID = %d, want > 0", protocol.SpecID)
	}

	// Fitness section must be populated.
	if protocol.Fitness.Target != 1.0 {
		t.Errorf("protocol.Fitness.Target = %f, want 1.0", protocol.Fitness.Target)
	}
	if protocol.Fitness.Lyapunov < 0.0 {
		t.Errorf("protocol.Fitness.Lyapunov = %f, want >= 0", protocol.Fitness.Lyapunov)
	}

	// Measure section must be populated with non-negative values.
	if protocol.Measure.OpenIssues < 0 {
		t.Errorf("protocol.Measure.OpenIssues = %d, want >= 0", protocol.Measure.OpenIssues)
	}
	if protocol.Measure.Unspecified < 0 {
		t.Errorf("protocol.Measure.Unspecified = %d, want >= 0", protocol.Measure.Unspecified)
	}
	if protocol.Measure.DriftScore < 0 {
		t.Errorf("protocol.Measure.DriftScore = %d, want >= 0", protocol.Measure.DriftScore)
	}

	// Convergence section must be populated.
	if protocol.Convergence.EstimatedStepsToFP < 0 {
		t.Errorf("protocol.Convergence.EstimatedStepsToFP = %d, want >= 0",
			protocol.Convergence.EstimatedStepsToFP)
	}

	// JSON serialization roundtrip.
	data, err := json.Marshal(protocol)
	if err != nil {
		t.Fatalf("marshal protocol: %v", err)
	}
	var roundtripped triage.Protocol
	if err := json.Unmarshal(data, &roundtripped); err != nil {
		t.Fatalf("unmarshal protocol: %v", err)
	}
	if roundtripped.Version != protocol.Version {
		t.Errorf("roundtrip: Version = %q, want %q", roundtripped.Version, protocol.Version)
	}
	if roundtripped.SpecID != protocol.SpecID {
		t.Errorf("roundtrip: SpecID = %d, want %d", roundtripped.SpecID, protocol.SpecID)
	}
}

// ─── TestEvidenceChain_NoAffected ────────────────────────────────────────────

// TestEvidenceChain_NoAffected verifies that VerifyEvidenceChain returns
// violations when no affected invariants are declared for the issue.
// (ddis:tests APP-INV-065)
func TestEvidenceChain_NoAffected(t *testing.T) {
	db, specID := getModularDB(t)

	// Use an empty event stream — issue 9999 has no triage events, so no
	// affected invariants are declared.
	chain, violations := triage.VerifyEvidenceChain(db, specID, 9999, nil)

	if chain != nil {
		t.Errorf("expected nil chain for issue with no affected invariants, got %+v", chain)
	}
	if len(violations) == 0 {
		t.Error("expected violations for issue with no affected invariants, got none")
	}

	// The single violation must identify the missing affected invariants.
	found := false
	for _, v := range violations {
		if v.Type == "no_affected_invariants" {
			found = true
			if v.Remedy == "" {
				t.Error("violation remedy should not be empty")
			}
		}
	}
	if !found {
		t.Errorf("expected violation type %q, got %v", "no_affected_invariants", violations)
	}
}

// ─── TestSpecConverged_RealSpec ───────────────────────────────────────────────

// TestSpecConverged_RealSpec runs the spec-convergence gate against the real spec.
// (ddis:tests APP-INV-064)
func TestSpecConverged_RealSpec(t *testing.T) {
	db, specID := getModularDB(t)

	result, err := triage.SpecConverged(db, specID)
	if err != nil {
		t.Fatalf("SpecConverged: %v", err)
	}
	if result == nil {
		t.Fatal("SpecConverged returned nil result")
	}

	// Result must have the required fields (regardless of convergence value).
	// NonConverged and Details may be nil when converged.
	if result.Converged {
		if len(result.NonConverged) != 0 {
			t.Errorf("converged=true but NonConverged = %v", result.NonConverged)
		}
	} else {
		// Not converged — must describe what failed.
		if len(result.NonConverged) == 0 {
			t.Error("converged=false but NonConverged is empty — missing failure description")
		}
	}
}

// ─── TestTriageMeasure_RealSpec ───────────────────────────────────────────────

// TestTriageMeasure_RealSpec computes μ(S) from the real event stream.
// Verifies μ ∈ ℕ³ (all non-negative) and IsFixpoint returns correct value.
// (ddis:tests APP-INV-068)
func TestTriageMeasure_RealSpec(t *testing.T) {
	db, specID := getModularDB(t)

	evts := readImplStream(t)

	driftReport, err := drift.Analyze(db, specID, drift.Options{Report: true})
	if err != nil {
		t.Fatalf("drift.Analyze: %v", err)
	}
	driftScore := 0
	if driftReport != nil {
		driftScore = driftReport.EffectiveDrift
	}

	measure := triage.ComputeMeasure(evts, 0, driftScore)

	// μ ∈ ℕ³: all components must be non-negative.
	if measure.OpenIssues < 0 {
		t.Errorf("measure.OpenIssues = %d, want >= 0", measure.OpenIssues)
	}
	if measure.Unspecified < 0 {
		t.Errorf("measure.Unspecified = %d, want >= 0", measure.Unspecified)
	}
	if measure.DriftScore < 0 {
		t.Errorf("measure.DriftScore = %d, want >= 0", measure.DriftScore)
	}

	// IsFixpoint is true iff all three components are zero.
	expectedFixpoint := measure.OpenIssues == 0 && measure.Unspecified == 0 && measure.DriftScore == 0
	if measure.IsFixpoint() != expectedFixpoint {
		t.Errorf("IsFixpoint() = %v, want %v (measure = %+v)",
			measure.IsFixpoint(), expectedFixpoint, measure)
	}

	// Verify LexLess is consistent: a measure is never lex-less than itself.
	if triage.LexLess(measure, measure) {
		t.Errorf("LexLess(m, m) should be false for measure %+v", measure)
	}
}

// ─── TestTriageAutoRanking_RealSpec ──────────────────────────────────────────

// TestTriageAutoRanking_RealSpec verifies deficiency ranking on real signals.
// If deficiencies exist, they must be sorted by ΔF descending and each must
// have a non-empty Action. (ddis:tests APP-INV-069)
func TestTriageAutoRanking_RealSpec(t *testing.T) {
	db, specID := getModularDB(t)

	signals := collectSignals(t, db, specID)
	defs := triage.RankDeficiencies(signals, "manifest.ddis.db")

	// All deficiencies must have non-empty Action.
	for i, d := range defs {
		if d.Action == "" {
			t.Errorf("deficiency[%d] (category=%q) has empty Action", i, d.Category)
		}
		if d.Category == "" {
			t.Errorf("deficiency[%d] has empty Category", i)
		}
		if d.DeltaF < 0.0 {
			t.Errorf("deficiency[%d] (category=%q) DeltaF = %f, want >= 0",
				i, d.Category, d.DeltaF)
		}
	}

	// Verify descending ΔF order.
	for i := 1; i < len(defs); i++ {
		if defs[i].DeltaF > defs[i-1].DeltaF {
			t.Errorf("deficiencies not sorted by ΔF descending: defs[%d].DeltaF=%f > defs[%d].DeltaF=%f",
				i, defs[i].DeltaF, i-1, defs[i-1].DeltaF)
		}
	}
}

// ─── TestSelfBootstrap_TriageWorkflow ─────────────────────────────────────────

// TestSelfBootstrap_TriageWorkflow verifies the triage-workflow module is present
// in the spec and that the expected invariants and ADRs are defined.
// (ddis:tests APP-INV-067)
func TestSelfBootstrap_TriageWorkflow(t *testing.T) {
	db, specID := getModularDB(t)

	// The triage-workflow module must be registered in the spec.
	var moduleCount int
	err := db.QueryRow(
		`SELECT COUNT(*) FROM modules WHERE spec_id = ? AND module_name LIKE '%triage%'`,
		specID,
	).Scan(&moduleCount)
	if err != nil {
		t.Fatalf("query triage module: %v", err)
	}
	if moduleCount == 0 {
		t.Error("triage-workflow module not found in spec — self-bootstrap requires it")
	}

	// APP-INV-063 through APP-INV-070 must all be present.
	requiredInvariants := []string{
		"APP-INV-063",
		"APP-INV-064",
		"APP-INV-065",
		"APP-INV-066",
		"APP-INV-067",
		"APP-INV-068",
		"APP-INV-069",
		"APP-INV-070",
	}
	for _, invID := range requiredInvariants {
		inv, err := storage.GetInvariant(db, specID, invID)
		if err != nil || inv == nil {
			t.Errorf("invariant %s not found in spec (err=%v)", invID, err)
			continue
		}
		if inv.Statement == "" {
			t.Errorf("invariant %s has empty Statement", invID)
		}
	}

	// APP-ADR-053 through APP-ADR-057 must all be present.
	requiredADRs := []string{
		"APP-ADR-053",
		"APP-ADR-054",
		"APP-ADR-055",
		"APP-ADR-056",
		"APP-ADR-057",
	}
	for _, adrID := range requiredADRs {
		adr, err := storage.GetADR(db, specID, adrID)
		if err != nil || adr == nil {
			t.Errorf("ADR %s not found in spec (err=%v)", adrID, err)
			continue
		}
		if adr.Title == "" {
			t.Errorf("ADR %s has empty Title", adrID)
		}
	}
}
