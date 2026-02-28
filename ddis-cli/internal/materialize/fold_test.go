package materialize

// ddis:tests APP-INV-073 (fold determinism — same events → same state)
// ddis:tests APP-INV-074 (causal ordering — topological sort)
// ddis:tests APP-INV-075 (materialization idempotency — replay produces identical state)

import (
	"encoding/json"
	"fmt"
	"testing"

	"github.com/wvandaal/ddis/internal/events"
)

// mockApplier records all operations for verification.
type mockApplier struct {
	calls []string
}

func (m *mockApplier) InsertSection(p events.SectionPayload) error {
	m.calls = append(m.calls, "InsertSection:"+p.Path)
	return nil
}
func (m *mockApplier) UpdateSection(p events.SectionUpdatePayload) error {
	m.calls = append(m.calls, "UpdateSection:"+p.Path)
	return nil
}
func (m *mockApplier) RemoveSection(p events.SectionRemovePayload) error {
	m.calls = append(m.calls, "RemoveSection:"+p.Path)
	return nil
}
func (m *mockApplier) InsertInvariant(p events.InvariantPayload) error {
	m.calls = append(m.calls, "InsertInvariant:"+p.ID)
	return nil
}
func (m *mockApplier) UpdateInvariant(p events.InvariantUpdatePayload) error {
	m.calls = append(m.calls, "UpdateInvariant:"+p.ID)
	return nil
}
func (m *mockApplier) RemoveInvariant(p events.InvariantRemovePayload) error {
	m.calls = append(m.calls, "RemoveInvariant:"+p.ID)
	return nil
}
func (m *mockApplier) InsertADR(p events.ADRPayload) error {
	m.calls = append(m.calls, "InsertADR:"+p.ID)
	return nil
}
func (m *mockApplier) UpdateADR(p events.ADRUpdatePayload) error {
	m.calls = append(m.calls, "UpdateADR:"+p.ID)
	return nil
}
func (m *mockApplier) SupersedeADR(p events.ADRSupersededPayload) error {
	m.calls = append(m.calls, "SupersedeADR:"+p.ID)
	return nil
}
func (m *mockApplier) InsertWitness(p events.WitnessPayload) error {
	m.calls = append(m.calls, "InsertWitness:"+p.InvariantID)
	return nil
}
func (m *mockApplier) RevokeWitness(p events.WitnessRevokePayload) error {
	m.calls = append(m.calls, "RevokeWitness:"+p.InvariantID)
	return nil
}
func (m *mockApplier) InsertChallenge(p events.ChallengePayload) error {
	m.calls = append(m.calls, "InsertChallenge:"+p.InvariantID)
	return nil
}
func (m *mockApplier) InsertModule(p events.ModulePayload) error {
	m.calls = append(m.calls, "InsertModule:"+p.Name)
	return nil
}
func (m *mockApplier) InsertGlossaryTerm(p events.GlossaryTermPayload) error {
	m.calls = append(m.calls, "InsertGlossaryTerm:"+p.Term)
	return nil
}
func (m *mockApplier) InsertCrossRef(p events.CrossRefPayload) error {
	m.calls = append(m.calls, "InsertCrossRef:"+p.Target)
	return nil
}
func (m *mockApplier) InsertNegativeSpec(p events.NegativeSpecPayload) error {
	m.calls = append(m.calls, "InsertNegativeSpec:"+p.Pattern)
	return nil
}
func (m *mockApplier) InsertQualityGate(p events.QualityGatePayload) error {
	m.calls = append(m.calls, "InsertQualityGate:"+p.Title)
	return nil
}

func makeEvent(id, typ, ts string, payload interface{}, causes []string) *events.Event {
	data, _ := json.Marshal(payload)
	return &events.Event{
		ID:        id,
		Type:      typ,
		Timestamp: ts,
		Stream:    events.StreamSpecification,
		Payload:   json.RawMessage(data),
		Causes:    causes,
	}
}

func TestApply_InvariantCrystallized(t *testing.T) {
	m := &mockApplier{}
	evt := makeEvent("evt-1", events.TypeInvariantCrystallized, "2026-01-01T00:00:00Z",
		events.InvariantPayload{ID: "APP-INV-071", Title: "Log Canonicality"}, nil)

	err := Apply(m, evt)
	if err != nil {
		t.Fatalf("Apply: %v", err)
	}
	if len(m.calls) != 1 || m.calls[0] != "InsertInvariant:APP-INV-071" {
		t.Errorf("expected InsertInvariant:APP-INV-071, got %v", m.calls)
	}
}

func TestApply_ADRCrystallized(t *testing.T) {
	m := &mockApplier{}
	evt := makeEvent("evt-2", events.TypeADRCrystallized, "2026-01-01T00:00:00Z",
		events.ADRPayload{ID: "APP-ADR-058", Title: "JSONL as Canonical"}, nil)

	err := Apply(m, evt)
	if err != nil {
		t.Fatalf("Apply: %v", err)
	}
	if len(m.calls) != 1 || m.calls[0] != "InsertADR:APP-ADR-058" {
		t.Errorf("expected InsertADR:APP-ADR-058, got %v", m.calls)
	}
}

func TestApply_UnknownType(t *testing.T) {
	m := &mockApplier{}
	evt := makeEvent("evt-3", "unknown_future_type", "2026-01-01T00:00:00Z", map[string]string{"foo": "bar"}, nil)

	err := Apply(m, evt)
	if err != nil {
		t.Fatalf("unknown types should be no-ops, got: %v", err)
	}
	if len(m.calls) != 0 {
		t.Errorf("expected no calls for unknown type, got %v", m.calls)
	}
}

func TestCausalSort_NoCauses(t *testing.T) {
	evts := []*events.Event{
		makeEvent("c", "t", "2026-01-03T00:00:00Z", nil, nil),
		makeEvent("a", "t", "2026-01-01T00:00:00Z", nil, nil),
		makeEvent("b", "t", "2026-01-02T00:00:00Z", nil, nil),
	}

	sorted, err := CausalSort(evts)
	if err != nil {
		t.Fatalf("CausalSort: %v", err)
	}

	// Without causes, should sort by timestamp
	if sorted[0].ID != "a" || sorted[1].ID != "b" || sorted[2].ID != "c" {
		t.Errorf("expected [a,b,c], got [%s,%s,%s]", sorted[0].ID, sorted[1].ID, sorted[2].ID)
	}
}

func TestCausalSort_WithCauses(t *testing.T) {
	// b depends on a, c depends on b
	evts := []*events.Event{
		makeEvent("c", "t", "2026-01-01T00:00:00Z", nil, []string{"b"}),
		makeEvent("a", "t", "2026-01-03T00:00:00Z", nil, nil),
		makeEvent("b", "t", "2026-01-02T00:00:00Z", nil, []string{"a"}),
	}

	sorted, err := CausalSort(evts)
	if err != nil {
		t.Fatalf("CausalSort: %v", err)
	}

	// Must respect causal order: a before b before c
	if sorted[0].ID != "a" || sorted[1].ID != "b" || sorted[2].ID != "c" {
		t.Errorf("expected [a,b,c], got [%s,%s,%s]", sorted[0].ID, sorted[1].ID, sorted[2].ID)
	}
}

func TestCausalSort_CycleDetection(t *testing.T) {
	evts := []*events.Event{
		makeEvent("a", "t", "2026-01-01T00:00:00Z", nil, []string{"b"}),
		makeEvent("b", "t", "2026-01-02T00:00:00Z", nil, []string{"a"}),
	}

	_, err := CausalSort(evts)
	if err == nil {
		t.Fatal("expected cycle detection error")
	}
}

func TestFold_Determinism(t *testing.T) {
	// APP-INV-073: same events → same calls in same order
	evts := []*events.Event{
		makeEvent("e1", events.TypeInvariantCrystallized, "2026-01-01T00:00:00Z",
			events.InvariantPayload{ID: "INV-A", Title: "First"}, nil),
		makeEvent("e2", events.TypeADRCrystallized, "2026-01-02T00:00:00Z",
			events.ADRPayload{ID: "ADR-B", Title: "Second"}, nil),
	}

	// Run twice
	m1 := &mockApplier{}
	r1, err := Fold(m1, evts)
	if err != nil {
		t.Fatalf("Fold run 1: %v", err)
	}

	m2 := &mockApplier{}
	r2, err := Fold(m2, evts)
	if err != nil {
		t.Fatalf("Fold run 2: %v", err)
	}

	// Same results
	if r1.EventsProcessed != r2.EventsProcessed {
		t.Errorf("determinism: processed %d vs %d", r1.EventsProcessed, r2.EventsProcessed)
	}
	if len(m1.calls) != len(m2.calls) {
		t.Fatalf("determinism: calls %d vs %d", len(m1.calls), len(m2.calls))
	}
	for i := range m1.calls {
		if m1.calls[i] != m2.calls[i] {
			t.Errorf("determinism: call[%d] = %s vs %s", i, m1.calls[i], m2.calls[i])
		}
	}
}

func TestFold_Idempotency(t *testing.T) {
	// APP-INV-075: delete and replay → identical
	evts := []*events.Event{
		makeEvent("e1", events.TypeModuleRegistered, "2026-01-01T00:00:00Z",
			events.ModulePayload{Name: "parse-pipeline", Domain: "parsing"}, nil),
		makeEvent("e2", events.TypeInvariantCrystallized, "2026-01-02T00:00:00Z",
			events.InvariantPayload{ID: "INV-001", Title: "Round Trip"}, nil),
	}

	m := &mockApplier{}
	result, err := Fold(m, evts)
	if err != nil {
		t.Fatalf("Fold: %v", err)
	}
	if result.EventsProcessed != 2 {
		t.Errorf("expected 2 processed, got %d", result.EventsProcessed)
	}
	if result.EventsSkipped != 0 {
		t.Errorf("expected 0 skipped, got %d", result.EventsSkipped)
	}
}

func TestFold_Empty(t *testing.T) {
	m := &mockApplier{}
	result, err := Fold(m, nil)
	if err != nil {
		t.Fatalf("Fold empty: %v", err)
	}
	if result.EventsProcessed != 0 {
		t.Errorf("expected 0 processed, got %d", result.EventsProcessed)
	}
}

func TestFoldWithProcessors_InvokesProcessors(t *testing.T) {
	// APP-INV-080: processors fire after each content event
	var fired []string
	engine := New()
	engine.RegisterProcessor(Processor{
		Name: "test-proc",
		Handle: func(evt *events.Event, db interface{}) ([]*events.Event, error) {
			fired = append(fired, evt.ID)
			return nil, nil
		},
	})

	m := &mockApplier{}
	evts := []*events.Event{
		makeEvent("e1", events.TypeInvariantCrystallized, "2026-01-01T00:00:00Z",
			events.InvariantPayload{ID: "INV-A", Title: "First"}, nil),
		makeEvent("e2", events.TypeADRCrystallized, "2026-01-02T00:00:00Z",
			events.ADRPayload{ID: "ADR-B", Title: "Second"}, nil),
	}

	result, err := engine.FoldWithProcessors(m, evts, nil)
	if err != nil {
		t.Fatalf("FoldWithProcessors: %v", err)
	}
	if result.EventsProcessed != 2 {
		t.Errorf("expected 2 processed, got %d", result.EventsProcessed)
	}
	if len(fired) != 2 {
		t.Errorf("expected processor fired 2 times, got %d: %v", len(fired), fired)
	}
}

func TestFoldWithProcessors_FailureIsolation(t *testing.T) {
	// APP-INV-091: processor error is non-fatal
	engine := New()
	engine.RegisterProcessor(Processor{
		Name: "failing-proc",
		Handle: func(evt *events.Event, db interface{}) ([]*events.Event, error) {
			return nil, fmt.Errorf("processor exploded")
		},
	})

	m := &mockApplier{}
	evts := []*events.Event{
		makeEvent("e1", events.TypeInvariantCrystallized, "2026-01-01T00:00:00Z",
			events.InvariantPayload{ID: "INV-A", Title: "First"}, nil),
	}

	result, err := engine.FoldWithProcessors(m, evts, nil)
	if err != nil {
		t.Fatalf("FoldWithProcessors should not fail: %v", err)
	}
	// Event should still be processed
	if result.EventsProcessed != 1 {
		t.Errorf("expected 1 processed, got %d", result.EventsProcessed)
	}
	// Processor error should be captured
	if len(result.Errors) != 1 {
		t.Errorf("expected 1 error from processor, got %d", len(result.Errors))
	}
}

func TestFoldWithProcessors_SkipsDerivedEvents(t *testing.T) {
	// APP-ADR-071: processors don't re-fire on derived events
	var fireCount int
	engine := New()
	engine.RegisterProcessor(Processor{
		Name: "counting-proc",
		Handle: func(evt *events.Event, db interface{}) ([]*events.Event, error) {
			fireCount++
			// Return a derived event
			derivedPayload := map[string]interface{}{
				"finding":    "test finding",
				"derived_by": "counting-proc",
			}
			derived := makeEvent("derived-1", "implementation_finding", "2026-01-01T01:00:00Z",
				derivedPayload, []string{evt.ID})
			return []*events.Event{derived}, nil
		},
	})

	m := &mockApplier{}
	evts := []*events.Event{
		makeEvent("e1", events.TypeInvariantCrystallized, "2026-01-01T00:00:00Z",
			events.InvariantPayload{ID: "INV-A", Title: "First"}, nil),
	}

	_, err := engine.FoldWithProcessors(m, evts, nil)
	if err != nil {
		t.Fatalf("FoldWithProcessors: %v", err)
	}
	// Processor should fire exactly once (on e1), not on the derived event
	if fireCount != 1 {
		t.Errorf("expected processor to fire 1 time, got %d", fireCount)
	}
}

func TestIsDerivedEvent(t *testing.T) {
	// Non-derived event
	evt := makeEvent("e1", events.TypeInvariantCrystallized, "2026-01-01T00:00:00Z",
		events.InvariantPayload{ID: "INV-A"}, nil)
	if isDerivedEvent(evt) {
		t.Error("regular event should not be derived")
	}

	// Derived event (has causes + derived_by in payload)
	derivedPayload := map[string]interface{}{
		"finding":    "test",
		"derived_by": "validation",
	}
	derived := makeEvent("d1", "implementation_finding", "2026-01-01T01:00:00Z",
		derivedPayload, []string{"e1"})
	if !isDerivedEvent(derived) {
		t.Error("event with causes + derived_by should be derived")
	}
}

func TestCausalSort_Empty(t *testing.T) {
	sorted, err := CausalSort(nil)
	if err != nil {
		t.Fatalf("CausalSort nil: %v", err)
	}
	if len(sorted) != 0 {
		t.Errorf("expected empty, got %d", len(sorted))
	}
}
