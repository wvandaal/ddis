package autoprompt

import (
	"encoding/json"
	"testing"
)

// ---------------------------------------------------------------------------
// KStarEff
// ---------------------------------------------------------------------------

func TestKStarEff_DepthZero(t *testing.T) {
	got := KStarEff(0)
	if got != 12 {
		t.Errorf("KStarEff(0) = %d, want 12 (BaseBudget)", got)
	}
}

func TestKStarEff_DepthFive(t *testing.T) {
	// depth=5 => 12 - (5/5) = 12 - 1 = 11
	got := KStarEff(5)
	if got != 11 {
		t.Errorf("KStarEff(5) = %d, want 11", got)
	}
}

func TestKStarEff_DepthTen(t *testing.T) {
	// depth=10 => 12 - (10/5) = 12 - 2 = 10
	got := KStarEff(10)
	if got != 10 {
		t.Errorf("KStarEff(10) = %d, want 10", got)
	}
}

func TestKStarEff_DepthSixty(t *testing.T) {
	// depth=60 => 12 - (60/5) = 12 - 12 = 0, clamped to Floor(3)
	got := KStarEff(60)
	if got != Floor {
		t.Errorf("KStarEff(60) = %d, want %d (Floor)", got, Floor)
	}
}

func TestKStarEff_HighDepth(t *testing.T) {
	// depth=100 => 12 - (100/5) = 12 - 20 = -8, clamped to Floor(3)
	got := KStarEff(100)
	if got != Floor {
		t.Errorf("KStarEff(100) = %d, want %d (Floor)", got, Floor)
	}
}

func TestKStarEff_NegativeDepth(t *testing.T) {
	// Negative depth: -1/5 = 0 in integer division => 12 - 0 = 12
	got := KStarEff(-1)
	if got != 12 {
		t.Errorf("KStarEff(-1) = %d, want 12", got)
	}
}

func TestKStarEff_Monotonic(t *testing.T) {
	// KStarEff should be non-increasing as depth increases
	prev := KStarEff(0)
	for d := 1; d <= 100; d++ {
		cur := KStarEff(d)
		if cur > prev {
			t.Errorf("KStarEff not monotonic: KStarEff(%d)=%d > KStarEff(%d)=%d", d, cur, d-1, prev)
		}
		prev = cur
	}
}

// ---------------------------------------------------------------------------
// TokenTarget
// ---------------------------------------------------------------------------

func TestTokenTarget_DepthZero(t *testing.T) {
	// k*=12 => MinTokens + (12-3)*(2000-300)/(12-3) = 300 + 9*1700/9 = 300 + 1700 = 2000
	got := TokenTarget(0)
	if got != MaxTokens {
		t.Errorf("TokenTarget(0) = %d, want %d (MaxTokens)", got, MaxTokens)
	}
}

func TestTokenTarget_HighDepth(t *testing.T) {
	// k*=3 (floor) => MinTokens + (3-3)*... = 300
	got := TokenTarget(100)
	if got != MinTokens {
		t.Errorf("TokenTarget(100) = %d, want %d (MinTokens)", got, MinTokens)
	}
}

func TestTokenTarget_InRange(t *testing.T) {
	for d := 0; d <= 100; d++ {
		tok := TokenTarget(d)
		if tok < MinTokens {
			t.Errorf("TokenTarget(%d) = %d, below MinTokens %d", d, tok, MinTokens)
		}
		if tok > MaxTokens {
			t.Errorf("TokenTarget(%d) = %d, above MaxTokens %d", d, tok, MaxTokens)
		}
	}
}

func TestTokenTarget_Monotonic(t *testing.T) {
	prev := TokenTarget(0)
	for d := 1; d <= 100; d++ {
		cur := TokenTarget(d)
		if cur > prev {
			t.Errorf("TokenTarget not monotonic: TokenTarget(%d)=%d > TokenTarget(%d)=%d", d, cur, d-1, prev)
		}
		prev = cur
	}
}

func TestTokenTarget_MidDepth(t *testing.T) {
	// depth=25 => k*=12-(25/5)=12-5=7 => 300 + (7-3)*1700/9 = 300 + 4*1700/9 = 300 + 755 = 1055
	got := TokenTarget(25)
	expected := MinTokens + (7-Floor)*(MaxTokens-MinTokens)/(BaseBudget-Floor)
	if got != expected {
		t.Errorf("TokenTarget(25) = %d, want %d", got, expected)
	}
}

// ---------------------------------------------------------------------------
// Attenuation
// ---------------------------------------------------------------------------

func TestAttenuation_DepthZero(t *testing.T) {
	// k*=12 => 1.0 - 12/12 = 0.0
	got := Attenuation(0)
	if got != 0.0 {
		t.Errorf("Attenuation(0) = %f, want 0.0", got)
	}
}

func TestAttenuation_HighDepth(t *testing.T) {
	// k*=3 (floor) => 1.0 - 3/12 = 0.75
	got := Attenuation(100)
	if got != 0.75 {
		t.Errorf("Attenuation(100) = %f, want 0.75", got)
	}
}

func TestAttenuation_InRange(t *testing.T) {
	for d := 0; d <= 100; d++ {
		att := Attenuation(d)
		if att < 0.0 {
			t.Errorf("Attenuation(%d) = %f, below 0.0", d, att)
		}
		if att > 0.75 {
			t.Errorf("Attenuation(%d) = %f, above 0.75", d, att)
		}
	}
}

func TestAttenuation_Monotonic(t *testing.T) {
	prev := Attenuation(0)
	for d := 1; d <= 100; d++ {
		cur := Attenuation(d)
		if cur < prev {
			t.Errorf("Attenuation not monotonic: Attenuation(%d)=%f < Attenuation(%d)=%f", d, cur, d-1, prev)
		}
		prev = cur
	}
}

func TestAttenuation_MidDepth(t *testing.T) {
	// depth=25 => k*=7 => 1.0 - 7/12 = 5/12 ~ 0.4167
	got := Attenuation(25)
	expected := 1.0 - 7.0/12.0
	if got < expected-0.001 || got > expected+0.001 {
		t.Errorf("Attenuation(25) = %f, want ~%f", got, expected)
	}
}

// ---------------------------------------------------------------------------
// CommandResult.RenderJSON
// ---------------------------------------------------------------------------

func TestRenderJSON_RoundTrip(t *testing.T) {
	cr := &CommandResult{
		Output: "test output",
		State: StateSnapshot{
			ActiveThread:     "test-thread",
			Confidence:       [5]int{8, 7, 6, 5, 4},
			LimitingFactor:   "depth",
			OpenQuestions:     3,
			ArtifactsWritten: 2,
			SpecDrift:        1.5,
			Iteration:        10,
		},
		Guidance: Guidance{
			ObservedMode:  "convergent",
			DoFHint:       "mid",
			SuggestedNext: []string{"action1", "action2"},
			Attenuation:   0.25,
		},
	}

	jsonStr, err := cr.RenderJSON()
	if err != nil {
		t.Fatalf("RenderJSON: %v", err)
	}

	// Parse it back
	var parsed CommandResult
	if err := json.Unmarshal([]byte(jsonStr), &parsed); err != nil {
		t.Fatalf("Unmarshal rendered JSON: %v", err)
	}

	if parsed.Output != cr.Output {
		t.Errorf("Output mismatch: %q != %q", parsed.Output, cr.Output)
	}
	if parsed.State.ActiveThread != cr.State.ActiveThread {
		t.Errorf("ActiveThread mismatch: %q != %q", parsed.State.ActiveThread, cr.State.ActiveThread)
	}
	if parsed.State.Confidence != cr.State.Confidence {
		t.Errorf("Confidence mismatch: %v != %v", parsed.State.Confidence, cr.State.Confidence)
	}
	if parsed.State.LimitingFactor != cr.State.LimitingFactor {
		t.Errorf("LimitingFactor mismatch: %q != %q", parsed.State.LimitingFactor, cr.State.LimitingFactor)
	}
	if parsed.State.SpecDrift != cr.State.SpecDrift {
		t.Errorf("SpecDrift mismatch: %f != %f", parsed.State.SpecDrift, cr.State.SpecDrift)
	}
	if parsed.Guidance.ObservedMode != cr.Guidance.ObservedMode {
		t.Errorf("ObservedMode mismatch: %q != %q", parsed.Guidance.ObservedMode, cr.Guidance.ObservedMode)
	}
	if parsed.Guidance.DoFHint != cr.Guidance.DoFHint {
		t.Errorf("DoFHint mismatch: %q != %q", parsed.Guidance.DoFHint, cr.Guidance.DoFHint)
	}
	if len(parsed.Guidance.SuggestedNext) != 2 {
		t.Errorf("SuggestedNext length: expected 2, got %d", len(parsed.Guidance.SuggestedNext))
	}
	if parsed.Guidance.Attenuation != cr.Guidance.Attenuation {
		t.Errorf("Attenuation mismatch: %f != %f", parsed.Guidance.Attenuation, cr.Guidance.Attenuation)
	}
}

func TestRenderJSON_EmptyResult(t *testing.T) {
	cr := &CommandResult{}
	jsonStr, err := cr.RenderJSON()
	if err != nil {
		t.Fatalf("RenderJSON on empty: %v", err)
	}
	if jsonStr == "" {
		t.Error("expected non-empty JSON for empty CommandResult")
	}

	var parsed CommandResult
	if err := json.Unmarshal([]byte(jsonStr), &parsed); err != nil {
		t.Fatalf("Unmarshal empty rendered JSON: %v", err)
	}
}

// ---------------------------------------------------------------------------
// DimensionNames and DimensionPriority
// ---------------------------------------------------------------------------

func TestDimensionNames_HasFiveEntries(t *testing.T) {
	if len(DimensionNames) != 5 {
		t.Errorf("DimensionNames has %d entries, want 5", len(DimensionNames))
	}
}

func TestDimensionNames_ExpectedValues(t *testing.T) {
	expected := [5]string{"coverage", "depth", "coherence", "completeness", "formality"}
	if DimensionNames != expected {
		t.Errorf("DimensionNames = %v, want %v", DimensionNames, expected)
	}
}

func TestDimensionPriority_HasFiveEntries(t *testing.T) {
	if len(DimensionPriority) != 5 {
		t.Errorf("DimensionPriority has %d entries, want 5", len(DimensionPriority))
	}
}

func TestDimensionPriority_ValidIndices(t *testing.T) {
	seen := make(map[int]bool)
	for i, idx := range DimensionPriority {
		if idx < 0 || idx > 4 {
			t.Errorf("DimensionPriority[%d] = %d, out of range [0,4]", i, idx)
		}
		seen[idx] = true
	}
	if len(seen) != 5 {
		t.Errorf("DimensionPriority should contain all 5 unique indices, got %d unique", len(seen))
	}
}

func TestDimensionPriority_ExpectedOrder(t *testing.T) {
	// Completeness first, then coherence, depth, coverage, formality
	expected := [5]int{ConfCompleteness, ConfCoherence, ConfDepth, ConfCoverage, ConfFormality}
	if DimensionPriority != expected {
		t.Errorf("DimensionPriority = %v, want %v", DimensionPriority, expected)
	}
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

func TestConstants(t *testing.T) {
	if BaseBudget != 12 {
		t.Errorf("BaseBudget = %d, want 12", BaseBudget)
	}
	if Step != 5 {
		t.Errorf("Step = %d, want 5", Step)
	}
	if Floor != 3 {
		t.Errorf("Floor = %d, want 3", Floor)
	}
	if MaxTokens != 2000 {
		t.Errorf("MaxTokens = %d, want 2000", MaxTokens)
	}
	if MinTokens != 300 {
		t.Errorf("MinTokens = %d, want 300", MinTokens)
	}
}

func TestConfidenceIndices(t *testing.T) {
	if ConfCoverage != 0 {
		t.Errorf("ConfCoverage = %d, want 0", ConfCoverage)
	}
	if ConfDepth != 1 {
		t.Errorf("ConfDepth = %d, want 1", ConfDepth)
	}
	if ConfCoherence != 2 {
		t.Errorf("ConfCoherence = %d, want 2", ConfCoherence)
	}
	if ConfCompleteness != 3 {
		t.Errorf("ConfCompleteness = %d, want 3", ConfCompleteness)
	}
	if ConfFormality != 4 {
		t.Errorf("ConfFormality = %d, want 4", ConfFormality)
	}
}
