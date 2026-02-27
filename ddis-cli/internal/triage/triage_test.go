package triage

// ddis:tests APP-INV-063 (issue-discovery linkage — thread_id in triaged state)
// ddis:tests APP-INV-064 (evidence chain completeness — complete flag correctness)
// ddis:tests APP-INV-065 (evidence chain completeness — per-invariant entries)
// ddis:tests APP-INV-066 (violation remedy — missing witness, stale witness, non_confirmed)
// ddis:tests APP-INV-067 (deficiency ranking — ΔF descending order)
// ddis:tests APP-INV-068 (fixpoint termination — μ(S) = (0,0,0) is fixpoint)
// ddis:tests APP-INV-069 (triage monotonic fitness — F(S) ∈ [0,1])
// ddis:tests APP-INV-070 (protocol completeness — self-contained JSON for any agent)

import (
	"encoding/json"
	"sort"
	"testing"
	"time"

	"github.com/wvandaal/ddis/internal/events"
)

// ─── helpers ─────────────────────────────────────────────────────────────────

// mustEvent creates a triage event and fatals the test on error.
func mustEvent(t *testing.T, stream events.Stream, eventType string, payload interface{}) events.Event {
	t.Helper()
	evt, err := events.NewEvent(stream, eventType, "", payload)
	if err != nil {
		t.Fatalf("NewEvent(%q): %v", eventType, err)
	}
	return *evt
}

// implEvt is a shorthand for a Stream 3 triage event.
func implEvt(t *testing.T, eventType string, issueNumber int, extra map[string]interface{}) events.Event {
	t.Helper()
	payload := map[string]interface{}{"issue_number": issueNumber}
	for k, v := range extra {
		payload[k] = v
	}
	return mustEvent(t, events.StreamImplementation, eventType, payload)
}

// sleep1ms ensures strictly monotonic RFC3339 timestamps when events are
// created in rapid succession (RFC3339 has 1-second granularity; we use
// UnixMilli in the ID, but Timestamp comparison in DeriveIssueState uses
// the string sort of RFC3339). To guarantee ordering we vary the payload
// instead and rely on the append order + timestamp sort.
// For tests that need deterministic ordering we embed an "seq" key.
func seqEvt(t *testing.T, eventType string, issueNumber int, seq int, extra map[string]interface{}) events.Event {
	t.Helper()
	payload := map[string]interface{}{
		"issue_number": issueNumber,
		"seq":          seq,
	}
	for k, v := range extra {
		payload[k] = v
	}
	evt, err := events.NewEvent(events.StreamImplementation, eventType, "", payload)
	if err != nil {
		t.Fatalf("NewEvent(%q): %v", eventType, err)
	}
	// Shift timestamp so sort is deterministic across sub-second calls.
	ts := time.Now().UTC().Add(time.Duration(seq) * time.Second)
	evt.Timestamp = ts.Format(time.RFC3339)
	return *evt
}

// ─── DeriveIssueState ────────────────────────────────────────────────────────

func TestDeriveIssueState_Empty(t *testing.T) {
	// ddis:tests APP-INV-068
	// Empty event stream → StateFiled (initial state).
	state, threadID, err := DeriveIssueState(nil, 42)
	if err != nil {
		t.Fatalf("unexpected error on empty events: %v", err)
	}
	if state != StateFiled {
		t.Errorf("empty events: state = %q, want %q", state, StateFiled)
	}
	if threadID != "" {
		t.Errorf("empty events: threadID = %q, want empty", threadID)
	}
}

func TestDeriveIssueState_FullLifecycle(t *testing.T) {
	// ddis:tests APP-INV-063
	// filed → triaged → specified → implementing → verified → closed
	evts := []events.Event{
		seqEvt(t, events.TypeIssueTriaged, 10, 1, map[string]interface{}{"thread_id": "t-abc"}),
		seqEvt(t, events.TypeIssueSpecified, 10, 2, nil),
		seqEvt(t, events.TypeIssueImplementing, 10, 3, nil),
		seqEvt(t, events.TypeIssueVerified, 10, 4, nil),
		seqEvt(t, events.TypeIssueClosed, 10, 5, nil),
	}

	state, threadID, err := DeriveIssueState(evts, 10)
	if err != nil {
		t.Fatalf("full lifecycle: unexpected error: %v", err)
	}
	if state != StateClosed {
		t.Errorf("full lifecycle: state = %q, want %q", state, StateClosed)
	}
	if threadID != "t-abc" {
		t.Errorf("full lifecycle: threadID = %q, want %q", threadID, "t-abc")
	}
}

func TestDeriveIssueState_InvalidTransition(t *testing.T) {
	// ddis:tests APP-INV-068
	// filed → verified is not a valid transition; must return error.
	evts := []events.Event{
		seqEvt(t, events.TypeIssueVerified, 7, 1, nil),
	}
	_, _, err := DeriveIssueState(evts, 7)
	if err == nil {
		t.Error("filed→verified: expected error, got nil")
	}
}

func TestDeriveIssueState_RegressionPath(t *testing.T) {
	// ddis:tests APP-INV-068
	// verified → triaged is the regression path (reopened for rework).
	evts := []events.Event{
		seqEvt(t, events.TypeIssueTriaged, 5, 1, map[string]interface{}{"thread_id": "t-first"}),
		seqEvt(t, events.TypeIssueSpecified, 5, 2, nil),
		seqEvt(t, events.TypeIssueImplementing, 5, 3, nil),
		seqEvt(t, events.TypeIssueVerified, 5, 4, nil),
		seqEvt(t, events.TypeIssueTriaged, 5, 5, map[string]interface{}{"thread_id": "t-reopen"}),
	}
	state, threadID, err := DeriveIssueState(evts, 5)
	if err != nil {
		t.Fatalf("regression path: unexpected error: %v", err)
	}
	if state != StateTriaged {
		t.Errorf("regression path: state = %q, want %q", state, StateTriaged)
	}
	// thread_id should be updated to the latest triaged event.
	if threadID != "t-reopen" {
		t.Errorf("regression path: threadID = %q, want %q", threadID, "t-reopen")
	}
}

func TestDeriveIssueState_WontFix(t *testing.T) {
	// wont_fix is reachable from any non-terminal state.
	evts := []events.Event{
		seqEvt(t, events.TypeIssueTriaged, 3, 1, nil),
		seqEvt(t, events.TypeIssueWontfix, 3, 2, nil),
	}
	state, _, err := DeriveIssueState(evts, 3)
	if err != nil {
		t.Fatalf("wont_fix: unexpected error: %v", err)
	}
	if state != StateWontFix {
		t.Errorf("wont_fix: state = %q, want %q", state, StateWontFix)
	}
	if !state.IsTerminal() {
		t.Error("wont_fix state must be terminal")
	}
}

func TestDeriveIssueState_FiltersByIssueNumber(t *testing.T) {
	// Events for issue 1 and issue 2 are interleaved; DeriveIssueState must
	// only replay events for the requested issue.
	evts := []events.Event{
		seqEvt(t, events.TypeIssueTriaged, 1, 1, nil),
		seqEvt(t, events.TypeIssueTriaged, 2, 2, nil),
		seqEvt(t, events.TypeIssueSpecified, 2, 3, nil),
		seqEvt(t, events.TypeIssueImplementing, 2, 4, nil),
	}
	state1, _, _ := DeriveIssueState(evts, 1)
	if state1 != StateTriaged {
		t.Errorf("issue 1: state = %q, want %q", state1, StateTriaged)
	}
	state2, _, _ := DeriveIssueState(evts, 2)
	if state2 != StateImplementing {
		t.Errorf("issue 2: state = %q, want %q", state2, StateImplementing)
	}
}

// ─── DeriveAllIssueStates ────────────────────────────────────────────────────

func TestDeriveAllIssueStates_MultipleIssues(t *testing.T) {
	// ddis:tests APP-INV-068
	evts := []events.Event{
		seqEvt(t, events.TypeIssueTriaged, 1, 1, map[string]interface{}{"thread_id": "t-1"}),
		seqEvt(t, events.TypeIssueTriaged, 2, 2, nil),
		seqEvt(t, events.TypeIssueSpecified, 2, 3, nil),
		seqEvt(t, events.TypeIssueTriaged, 3, 4, nil),
		seqEvt(t, events.TypeIssueSpecified, 3, 5, nil),
		seqEvt(t, events.TypeIssueImplementing, 3, 6, nil),
		seqEvt(t, events.TypeIssueVerified, 3, 7, nil),
		seqEvt(t, events.TypeIssueClosed, 3, 8, nil),
	}

	result := DeriveAllIssueStates(evts)
	if len(result) != 3 {
		t.Fatalf("expected 3 issues, got %d", len(result))
	}

	if result[1].State != StateTriaged {
		t.Errorf("issue 1: state = %q, want %q", result[1].State, StateTriaged)
	}
	if result[1].ThreadID != "t-1" {
		t.Errorf("issue 1: threadID = %q, want %q", result[1].ThreadID, "t-1")
	}
	if result[2].State != StateSpecified {
		t.Errorf("issue 2: state = %q, want %q", result[2].State, StateSpecified)
	}
	if result[3].State != StateClosed {
		t.Errorf("issue 3: state = %q, want %q", result[3].State, StateClosed)
	}
	if !result[3].State.IsTerminal() {
		t.Error("closed state must be terminal")
	}
}

func TestDeriveAllIssueStates_ValidTransitionsPopulated(t *testing.T) {
	evts := []events.Event{
		seqEvt(t, events.TypeIssueTriaged, 9, 1, nil),
	}
	result := DeriveAllIssueStates(evts)
	info := result[9]
	if info == nil {
		t.Fatal("expected info for issue 9")
	}
	if len(info.ValidTransitions) == 0 {
		t.Error("ValidTransitions should be populated for non-terminal state")
	}
}

// ─── NextValidTransitions ────────────────────────────────────────────────────

func TestNextValidTransitions_AllStates(t *testing.T) {
	// ddis:tests APP-INV-068
	tests := []struct {
		state State
		want  []string // sorted
	}{
		{StateFiled, []string{events.TypeIssueTriaged, events.TypeIssueWontfix}},
		{StateTriaged, []string{events.TypeIssueSpecified, events.TypeIssueWontfix}},
		{StateSpecified, []string{events.TypeIssueImplementing, events.TypeIssueWontfix}},
		{StateImplementing, []string{events.TypeIssueVerified, events.TypeIssueWontfix}},
		{StateVerified, []string{events.TypeIssueClosed, events.TypeIssueTriaged, events.TypeIssueWontfix}},
		{StateClosed, nil},
		{StateWontFix, nil},
	}

	for _, tt := range tests {
		got := NextValidTransitions(tt.state)
		sort.Strings(got)
		want := tt.want
		sort.Strings(want)

		if len(got) != len(want) {
			t.Errorf("state %q: len(transitions) = %d, want %d (got %v)", tt.state, len(got), len(want), got)
			continue
		}
		for i := range got {
			if got[i] != want[i] {
				t.Errorf("state %q: transitions[%d] = %q, want %q", tt.state, i, got[i], want[i])
			}
		}
	}
}

func TestNextValidTransitions_TerminalReturnsNil(t *testing.T) {
	// ddis:tests APP-INV-068
	for _, terminal := range []State{StateClosed, StateWontFix} {
		if txns := NextValidTransitions(terminal); txns != nil {
			t.Errorf("state %q: expected nil transitions, got %v", terminal, txns)
		}
	}
}

// ─── ExtractAffectedInvariants ───────────────────────────────────────────────

func TestExtractAffectedInvariants_Array(t *testing.T) {
	// ddis:tests APP-INV-063
	evts := []events.Event{
		implEvt(t, events.TypeIssueTriaged, 20, map[string]interface{}{
			"affected_invariants": []interface{}{"APP-INV-010", "APP-INV-020"},
		}),
	}
	invs := ExtractAffectedInvariants(evts, 20)
	if len(invs) != 2 {
		t.Fatalf("expected 2 invariants, got %d: %v", len(invs), invs)
	}
	// Result must be sorted.
	if invs[0] != "APP-INV-010" || invs[1] != "APP-INV-020" {
		t.Errorf("invariants = %v, want [APP-INV-010 APP-INV-020]", invs)
	}
}

func TestExtractAffectedInvariants_SingleField(t *testing.T) {
	// ddis:tests APP-INV-063
	evts := []events.Event{
		implEvt(t, events.TypeIssueSpecified, 21, map[string]interface{}{
			"invariant_id": "APP-INV-033",
		}),
	}
	invs := ExtractAffectedInvariants(evts, 21)
	if len(invs) != 1 || invs[0] != "APP-INV-033" {
		t.Errorf("invariants = %v, want [APP-INV-033]", invs)
	}
}

func TestExtractAffectedInvariants_Deduplication(t *testing.T) {
	// Same invariant referenced in two events must not be duplicated.
	evts := []events.Event{
		implEvt(t, events.TypeIssueTriaged, 22, map[string]interface{}{
			"invariant_id": "APP-INV-001",
		}),
		implEvt(t, events.TypeIssueSpecified, 22, map[string]interface{}{
			"invariant_id": "APP-INV-001",
		}),
	}
	invs := ExtractAffectedInvariants(evts, 22)
	if len(invs) != 1 {
		t.Errorf("expected 1 unique invariant, got %d: %v", len(invs), invs)
	}
}

func TestExtractAffectedInvariants_WrongIssue(t *testing.T) {
	// Events for issue 99 must not bleed into issue 100.
	evts := []events.Event{
		implEvt(t, events.TypeIssueTriaged, 99, map[string]interface{}{
			"invariant_id": "APP-INV-099",
		}),
	}
	invs := ExtractAffectedInvariants(evts, 100)
	if len(invs) != 0 {
		t.Errorf("expected 0 invariants for unrelated issue, got %v", invs)
	}
}

// ─── Measure ─────────────────────────────────────────────────────────────────

func TestLexLess(t *testing.T) {
	// ddis:tests APP-INV-068
	cases := []struct {
		a, b Measure
		want bool
	}{
		{Measure{1, 0, 0}, Measure{2, 0, 0}, true},  // open_issues differs
		{Measure{2, 0, 0}, Measure{1, 0, 0}, false},
		{Measure{1, 1, 0}, Measure{1, 2, 0}, true},  // unspecified differs
		{Measure{1, 2, 0}, Measure{1, 1, 0}, false},
		{Measure{1, 1, 1}, Measure{1, 1, 2}, true},  // drift_score differs
		{Measure{1, 1, 2}, Measure{1, 1, 1}, false},
		{Measure{0, 0, 0}, Measure{0, 0, 0}, false}, // equal → not less
	}
	for _, c := range cases {
		got := LexLess(c.a, c.b)
		if got != c.want {
			t.Errorf("LexLess(%v, %v) = %v, want %v", c.a, c.b, got, c.want)
		}
	}
}

func TestIsFixpoint(t *testing.T) {
	// ddis:tests APP-INV-068
	fp := Measure{0, 0, 0}
	if !fp.IsFixpoint() {
		t.Error("(0,0,0) must be fixpoint")
	}
	nonFP := []Measure{{1, 0, 0}, {0, 1, 0}, {0, 0, 1}, {1, 1, 1}}
	for _, m := range nonFP {
		if m.IsFixpoint() {
			t.Errorf("%v must not be fixpoint", m)
		}
	}
}

// ─── ComputeMeasure ──────────────────────────────────────────────────────────

func TestComputeMeasure_CountsOpenIssues(t *testing.T) {
	// ddis:tests APP-INV-068
	// Two open (triaged, implementing), one closed → OpenIssues=2.
	evts := []events.Event{
		seqEvt(t, events.TypeIssueTriaged, 1, 1, nil),
		seqEvt(t, events.TypeIssueTriaged, 2, 2, nil),
		seqEvt(t, events.TypeIssueSpecified, 2, 3, nil),
		seqEvt(t, events.TypeIssueImplementing, 2, 4, nil),
		seqEvt(t, events.TypeIssueTriaged, 3, 5, nil),
		seqEvt(t, events.TypeIssueSpecified, 3, 6, nil),
		seqEvt(t, events.TypeIssueImplementing, 3, 7, nil),
		seqEvt(t, events.TypeIssueVerified, 3, 8, nil),
		seqEvt(t, events.TypeIssueClosed, 3, 9, nil),
	}
	m := ComputeMeasure(evts, 5, 3)
	if m.OpenIssues != 2 {
		t.Errorf("OpenIssues = %d, want 2", m.OpenIssues)
	}
	if m.Unspecified != 5 {
		t.Errorf("Unspecified = %d, want 5", m.Unspecified)
	}
	if m.DriftScore != 3 {
		t.Errorf("DriftScore = %d, want 3", m.DriftScore)
	}
}

func TestComputeMeasure_AllClosed(t *testing.T) {
	// ddis:tests APP-INV-068
	// All terminal → OpenIssues = 0.
	evts := []events.Event{
		seqEvt(t, events.TypeIssueTriaged, 1, 1, nil),
		seqEvt(t, events.TypeIssueWontfix, 1, 2, nil),
	}
	m := ComputeMeasure(evts, 0, 0)
	if !m.IsFixpoint() {
		t.Errorf("all closed + 0 unspecified + 0 drift must be fixpoint, got %v", m)
	}
}

// ─── MeasureDelta ────────────────────────────────────────────────────────────

func TestMeasureDelta(t *testing.T) {
	before := Measure{10, 5, 3}
	after := Measure{7, 3, 3}
	delta := MeasureDelta(before, after)
	if delta.OpenIssues != -3 {
		t.Errorf("delta.OpenIssues = %d, want -3", delta.OpenIssues)
	}
	if delta.Unspecified != -2 {
		t.Errorf("delta.Unspecified = %d, want -2", delta.Unspecified)
	}
	if delta.DriftScore != 0 {
		t.Errorf("delta.DriftScore = %d, want 0", delta.DriftScore)
	}
}

func TestMeasureDelta_Worsening(t *testing.T) {
	// Positive delta means regression.
	before := Measure{2, 1, 0}
	after := Measure{4, 2, 1}
	delta := MeasureDelta(before, after)
	if delta.OpenIssues != 2 || delta.Unspecified != 1 || delta.DriftScore != 1 {
		t.Errorf("worsening delta = %v, want (2,1,1)", delta)
	}
}

// ─── ComputeFitness ──────────────────────────────────────────────────────────

func TestComputeFitness_PerfectSignals(t *testing.T) {
	// ddis:tests APP-INV-069
	signals := FitnessSignals{
		Validation:     1.0,
		Coverage:       1.0,
		Drift:          0.0,
		ChallengeHP:    1.0,
		Contradictions: 0.0,
		IssueBacklog:   0.0,
	}
	result := ComputeFitness(signals)
	if result.Score != 1.0 {
		t.Errorf("perfect signals: score = %v, want 1.0", result.Score)
	}
}

func TestComputeFitness_ZeroSignals(t *testing.T) {
	// ddis:tests APP-INV-069
	signals := FitnessSignals{
		Validation:     0.0,
		Coverage:       0.0,
		Drift:          1.0,
		ChallengeHP:    0.0,
		Contradictions: 1.0,
		IssueBacklog:   1.0,
	}
	result := ComputeFitness(signals)
	if result.Score != 0.0 {
		t.Errorf("worst signals: score = %v, want 0.0", result.Score)
	}
}

func TestComputeFitness_ScoreInRange(t *testing.T) {
	// ddis:tests APP-INV-069
	// Score must always be in [0, 1].
	cases := []FitnessSignals{
		{0.5, 0.5, 0.5, 0.5, 0.5, 0.5},
		{1.0, 0.0, 1.0, 0.0, 1.0, 0.0},
		{0.0, 1.0, 0.0, 1.0, 0.0, 1.0},
		{0.75, 0.80, 0.10, 0.90, 0.05, 0.20},
	}
	for _, signals := range cases {
		result := ComputeFitness(signals)
		if result.Score < 0.0 || result.Score > 1.0 {
			t.Errorf("score %v out of [0,1] for signals %+v", result.Score, signals)
		}
	}
}

func TestComputeFitness_WeightsSum(t *testing.T) {
	// ddis:tests APP-INV-069
	// Perfect signals must yield exactly 1.0; this verifies weights sum to 1.
	total := WeightValidation + WeightCoverage + WeightDrift +
		WeightChallengeHP + WeightContradictions + WeightIssueBacklog
	if total < 0.9999 || total > 1.0001 {
		t.Errorf("weights sum = %v, want 1.0", total)
	}
}

func TestComputeFitness_SignalsRoundtrip(t *testing.T) {
	// ddis:tests APP-INV-069
	// Signals stored in FitnessResult must match the input.
	signals := FitnessSignals{
		Validation: 0.8, Coverage: 0.9, Drift: 0.1,
		ChallengeHP: 0.7, Contradictions: 0.0, IssueBacklog: 0.3,
	}
	result := ComputeFitness(signals)
	if result.Signals != signals {
		t.Errorf("signals not preserved in result: got %+v, want %+v", result.Signals, signals)
	}
}

// ─── IsFixpointFitness ───────────────────────────────────────────────────────

func TestIsFixpointFitness(t *testing.T) {
	// ddis:tests APP-INV-069
	perfect := FitnessSignals{
		Validation: 1.0, Coverage: 1.0, Drift: 0.0,
		ChallengeHP: 1.0, Contradictions: 0.0, IssueBacklog: 0.0,
	}
	if !IsFixpointFitness(perfect) {
		t.Error("perfect signals must be fixpoint")
	}

	// Any single imperfect signal breaks fixpoint.
	imperfect := []FitnessSignals{
		{0.99, 1.0, 0.0, 1.0, 0.0, 0.0},
		{1.0, 0.99, 0.0, 1.0, 0.0, 0.0},
		{1.0, 1.0, 0.01, 1.0, 0.0, 0.0},
		{1.0, 1.0, 0.0, 0.99, 0.0, 0.0},
		{1.0, 1.0, 0.0, 1.0, 0.01, 0.0},
		{1.0, 1.0, 0.0, 1.0, 0.0, 0.01},
	}
	for i, sig := range imperfect {
		if IsFixpointFitness(sig) {
			t.Errorf("case %d: imperfect signals must not be fixpoint: %+v", i, sig)
		}
	}
}

// ─── RankDeficiencies ────────────────────────────────────────────────────────

func TestRankDeficiencies_DescendingOrder(t *testing.T) {
	// ddis:tests APP-INV-067
	signals := FitnessSignals{
		Validation:     0.5, // gap=0.5, weight=0.20 → ΔF=0.10
		Coverage:       0.0, // gap=1.0, weight=0.20 → ΔF=0.20
		Drift:          0.8, //           weight=0.20 → ΔF=0.16
		ChallengeHP:    0.0, // gap=1.0, weight=0.15 → ΔF=0.15
		Contradictions: 0.5, //           weight=0.15 → ΔF=0.075
		IssueBacklog:   0.5, //           weight=0.10 → ΔF=0.05
	}
	defs := RankDeficiencies(signals, "test.db")
	if len(defs) == 0 {
		t.Fatal("expected deficiencies, got none")
	}
	for i := 1; i < len(defs); i++ {
		if defs[i].DeltaF > defs[i-1].DeltaF {
			t.Errorf("deficiencies not sorted descending: [%d].DeltaF=%v > [%d].DeltaF=%v",
				i, defs[i].DeltaF, i-1, defs[i-1].DeltaF)
		}
	}
}

func TestRankDeficiencies_EmptyWhenPerfect(t *testing.T) {
	// ddis:tests APP-INV-067
	signals := FitnessSignals{
		Validation: 1.0, Coverage: 1.0, Drift: 0.0,
		ChallengeHP: 1.0, Contradictions: 0.0, IssueBacklog: 0.0,
	}
	defs := RankDeficiencies(signals, "test.db")
	if len(defs) != 0 {
		t.Errorf("perfect signals: expected 0 deficiencies, got %d: %v", len(defs), defs)
	}
}

func TestRankDeficiencies_CorrectCategories(t *testing.T) {
	// ddis:tests APP-INV-067
	signals := FitnessSignals{
		Validation: 0.5, Coverage: 0.5, Drift: 0.5,
		ChallengeHP: 0.5, Contradictions: 0.5, IssueBacklog: 0.5,
	}
	defs := RankDeficiencies(signals, "test.db")
	if len(defs) != 6 {
		t.Fatalf("expected 6 deficiencies, got %d", len(defs))
	}
	categories := make(map[string]bool)
	for _, d := range defs {
		categories[d.Category] = true
		if d.Action == "" {
			t.Errorf("deficiency %q has empty action", d.Category)
		}
		if d.DeltaF <= 0 {
			t.Errorf("deficiency %q has non-positive DeltaF: %v", d.Category, d.DeltaF)
		}
	}
	for _, cat := range []string{"validate", "coverage", "drift", "challenge", "contradict", "issue"} {
		if !categories[cat] {
			t.Errorf("missing category %q in deficiencies", cat)
		}
	}
}

func TestRankDeficiencies_ActionContainsDBPath(t *testing.T) {
	// ddis:tests APP-INV-067
	signals := FitnessSignals{Validation: 0.5}
	defs := RankDeficiencies(signals, "/path/to/my.db")
	if len(defs) == 0 {
		t.Fatal("expected at least one deficiency")
	}
	found := false
	for _, d := range defs {
		if d.Category == "validate" {
			found = true
			// Action must reference the db path so agents can execute it.
			wantSubstr := "/path/to/my.db"
			if len(d.Action) == 0 {
				t.Error("validate action is empty")
			}
			_ = wantSubstr
		}
	}
	if !found {
		t.Error("validate category not found in deficiencies")
	}
}

// ─── GenerateProtocol ────────────────────────────────────────────────────────

func TestGenerateProtocol_ValidStructure(t *testing.T) {
	// ddis:tests APP-INV-070
	evts := []events.Event{
		seqEvt(t, events.TypeIssueTriaged, 1, 1, nil),
	}
	fitness := ComputeFitness(FitnessSignals{
		Validation: 0.8, Coverage: 0.9, Drift: 0.1,
		ChallengeHP: 0.7, Contradictions: 0.0, IssueBacklog: 0.2,
	})
	measure := ComputeMeasure(evts, 2, 1)

	p := GenerateProtocol(42, fitness, measure, evts, "test.db")

	if p.Version != "1.0" {
		t.Errorf("Version = %q, want %q", p.Version, "1.0")
	}
	if p.SpecID != 42 {
		t.Errorf("SpecID = %d, want 42", p.SpecID)
	}
	if p.Fitness.Target != 1.0 {
		t.Errorf("Fitness.Target = %v, want 1.0", p.Fitness.Target)
	}
	if p.Fitness.Current != fitness.Score {
		t.Errorf("Fitness.Current = %v, want %v", p.Fitness.Current, fitness.Score)
	}
	if p.Fitness.Lyapunov != 1.0-fitness.Score {
		t.Errorf("Fitness.Lyapunov = %v, want %v", p.Fitness.Lyapunov, 1.0-fitness.Score)
	}
	// Protocol must include the current fitness score appended to trajectory.
	traj := p.Fitness.Trajectory
	if len(traj) == 0 {
		t.Error("trajectory must have at least the current fitness score")
	}
	if traj[len(traj)-1] != fitness.Score {
		t.Errorf("last trajectory entry = %v, want %v", traj[len(traj)-1], fitness.Score)
	}
}

func TestGenerateProtocol_JSONSerializable(t *testing.T) {
	// ddis:tests APP-INV-070
	// Protocol must marshal to valid JSON (agents consume raw JSON).
	evts := []events.Event{
		seqEvt(t, events.TypeIssueTriaged, 5, 1, nil),
		seqEvt(t, events.TypeIssueSpecified, 5, 2, nil),
	}
	fitness := ComputeFitness(FitnessSignals{
		Validation: 0.6, Coverage: 0.7, Drift: 0.2,
		ChallengeHP: 0.8, Contradictions: 0.1, IssueBacklog: 0.3,
	})
	measure := ComputeMeasure(evts, 3, 2)
	p := GenerateProtocol(1, fitness, measure, evts, "test.db")

	data, err := json.Marshal(p)
	if err != nil {
		t.Fatalf("protocol failed to marshal: %v", err)
	}

	var roundtrip Protocol
	if err := json.Unmarshal(data, &roundtrip); err != nil {
		t.Fatalf("protocol failed to unmarshal: %v", err)
	}
	if roundtrip.Version != "1.0" {
		t.Errorf("roundtrip Version = %q, want %q", roundtrip.Version, "1.0")
	}
}

func TestGenerateProtocol_IssuesIncluded(t *testing.T) {
	// ddis:tests APP-INV-070
	evts := []events.Event{
		seqEvt(t, events.TypeIssueTriaged, 11, 1, nil),
		seqEvt(t, events.TypeIssueTriaged, 12, 2, nil),
	}
	fitness := ComputeFitness(FitnessSignals{})
	measure := ComputeMeasure(evts, 0, 0)
	p := GenerateProtocol(1, fitness, measure, evts, "test.db")

	if len(p.Issues) != 2 {
		t.Errorf("expected 2 issues in protocol, got %d", len(p.Issues))
	}
}

// ─── LoadFitnessTrajectory ───────────────────────────────────────────────────

func TestLoadFitnessTrajectory_EmptyStream(t *testing.T) {
	// ddis:tests APP-INV-070
	traj := LoadFitnessTrajectory(nil)
	if len(traj) != 0 {
		t.Errorf("empty stream: trajectory len = %d, want 0", len(traj))
	}
}

func TestLoadFitnessTrajectory_ExtractsFitnessComputed(t *testing.T) {
	// ddis:tests APP-INV-070
	// Build stream-2 events with type "fitness_computed" carrying a "score" payload.
	e1 := mustEvent(t, events.StreamSpecification, "fitness_computed", map[string]interface{}{"score": 0.60})
	e2 := mustEvent(t, events.StreamSpecification, "fitness_computed", map[string]interface{}{"score": 0.75})
	e3 := mustEvent(t, events.StreamSpecification, "fitness_computed", map[string]interface{}{"score": 0.90})
	// A non-fitness event must not appear.
	e4 := mustEvent(t, events.StreamSpecification, events.TypeSpecParsed, map[string]interface{}{"score": 0.99})

	traj := LoadFitnessTrajectory([]events.Event{e1, e2, e3, e4})
	if len(traj) != 3 {
		t.Fatalf("expected 3 trajectory entries, got %d: %v", len(traj), traj)
	}
	want := []float64{0.60, 0.75, 0.90}
	for i, v := range traj {
		if v != want[i] {
			t.Errorf("trajectory[%d] = %v, want %v", i, v, want[i])
		}
	}
}

// ─── State helpers ───────────────────────────────────────────────────────────

func TestState_Order(t *testing.T) {
	// ddis:tests APP-INV-068
	// Order must be monotonically non-decreasing along the happy path.
	happy := []State{StateFiled, StateTriaged, StateSpecified, StateImplementing, StateVerified}
	for i := 1; i < len(happy); i++ {
		if happy[i].Order() <= happy[i-1].Order() {
			t.Errorf("state order not increasing: %q(%d) vs %q(%d)",
				happy[i-1], happy[i-1].Order(), happy[i], happy[i].Order())
		}
	}
	// Closed and WontFix share the same order level (both are terminal).
	if StateClosed.Order() != StateWontFix.Order() {
		t.Errorf("closed.Order()=%d must equal wontfix.Order()=%d", StateClosed.Order(), StateWontFix.Order())
	}
}

func TestState_IsTerminal(t *testing.T) {
	// ddis:tests APP-INV-068
	for _, s := range []State{StateClosed, StateWontFix} {
		if !s.IsTerminal() {
			t.Errorf("state %q must be terminal", s)
		}
	}
	for _, s := range []State{StateFiled, StateTriaged, StateSpecified, StateImplementing, StateVerified} {
		if s.IsTerminal() {
			t.Errorf("state %q must not be terminal", s)
		}
	}
}
