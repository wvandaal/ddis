package causal

// ddis:tests APP-INV-074 (causal ordering — DAG construction)
// ddis:tests APP-INV-081 (CRDT convergence — merge commutativity)
// ddis:tests APP-INV-082 (bisect correctness — binary search)
// ddis:tests APP-INV-084 (causal provenance — element-to-event tracing)

import (
	"encoding/json"
	"fmt"
	"testing"

	"github.com/wvandaal/ddis/internal/events"
)

func makeEvt(id, typ, ts string, payload interface{}, causes []string) *events.Event {
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

func TestNewDAG_ValidCauses(t *testing.T) {
	evts := []*events.Event{
		makeEvt("a", "t", "2026-01-01T00:00:00Z", nil, nil),
		makeEvt("b", "t", "2026-01-02T00:00:00Z", nil, []string{"a"}),
		makeEvt("c", "t", "2026-01-03T00:00:00Z", nil, []string{"a", "b"}),
	}

	dag, err := NewDAG(evts)
	if err != nil {
		t.Fatalf("NewDAG: %v", err)
	}

	if !dag.Reachable("a", "c") {
		t.Error("expected a → c reachable")
	}
	if !dag.Reachable("b", "c") {
		t.Error("expected b → c reachable")
	}
	if dag.Reachable("c", "a") {
		t.Error("c → a should not be reachable")
	}
}

func TestNewDAG_UnknownCause(t *testing.T) {
	evts := []*events.Event{
		makeEvt("a", "t", "2026-01-01T00:00:00Z", nil, []string{"nonexistent"}),
	}

	_, err := NewDAG(evts)
	if err == nil {
		t.Fatal("expected error for unknown cause")
	}
}

func TestDAG_Independent(t *testing.T) {
	evts := []*events.Event{
		makeEvt("a", "t", "2026-01-01T00:00:00Z", nil, nil),
		makeEvt("b", "t", "2026-01-02T00:00:00Z", nil, nil),
	}

	dag, err := NewDAG(evts)
	if err != nil {
		t.Fatalf("NewDAG: %v", err)
	}

	if !dag.Independent("a", "b") {
		t.Error("a and b should be independent (no causal link)")
	}
}

func TestDAG_NotIndependent(t *testing.T) {
	evts := []*events.Event{
		makeEvt("a", "t", "2026-01-01T00:00:00Z", nil, nil),
		makeEvt("b", "t", "2026-01-02T00:00:00Z", nil, []string{"a"}),
	}

	dag, err := NewDAG(evts)
	if err != nil {
		t.Fatalf("NewDAG: %v", err)
	}

	if dag.Independent("a", "b") {
		t.Error("a and b should NOT be independent (a causes b)")
	}
}

func TestMerge_Commutativity(t *testing.T) {
	// APP-INV-081: merge(A,B) = merge(B,A)
	streamA := []*events.Event{
		makeEvt("e1", events.TypeInvariantCrystallized, "2026-01-01T00:00:00Z",
			map[string]string{"id": "INV-001"}, nil),
		makeEvt("e2", events.TypeADRCrystallized, "2026-01-02T00:00:00Z",
			map[string]string{"id": "ADR-001"}, nil),
	}
	streamB := []*events.Event{
		makeEvt("e3", events.TypeInvariantCrystallized, "2026-01-03T00:00:00Z",
			map[string]string{"id": "INV-002"}, nil),
	}

	mergeAB := Merge(streamA, streamB)
	mergeBA := Merge(streamB, streamA)

	if len(mergeAB) != len(mergeBA) {
		t.Fatalf("commutativity: len(A∪B)=%d != len(B∪A)=%d", len(mergeAB), len(mergeBA))
	}

	for i := range mergeAB {
		if mergeAB[i].ID != mergeBA[i].ID {
			t.Errorf("commutativity: position %d: %s != %s", i, mergeAB[i].ID, mergeBA[i].ID)
		}
	}
}

func TestMerge_Deduplication(t *testing.T) {
	evt := makeEvt("e1", "t", "2026-01-01T00:00:00Z", map[string]string{"id": "X"}, nil)

	merged := Merge([]*events.Event{evt}, []*events.Event{evt})
	if len(merged) != 1 {
		t.Errorf("expected 1 event after dedup, got %d", len(merged))
	}
}

func TestMerge_LWWConflict(t *testing.T) {
	// Two concurrent updates to same element — latest timestamp wins
	e1 := makeEvt("e1", events.TypeInvariantCrystallized, "2026-01-01T00:00:00Z",
		map[string]string{"id": "INV-001"}, nil)
	e2 := makeEvt("e2", events.TypeInvariantCrystallized, "2026-01-02T00:00:00Z",
		map[string]string{"id": "INV-001"}, nil)

	merged := Merge([]*events.Event{e1}, []*events.Event{e2})
	if len(merged) != 1 {
		t.Fatalf("expected 1 event after LWW, got %d", len(merged))
	}
	if merged[0].ID != "e2" {
		t.Errorf("expected e2 (later timestamp) to win, got %s", merged[0].ID)
	}
}

func TestProvenance(t *testing.T) {
	// APP-INV-084: trace element to events
	evts := []*events.Event{
		makeEvt("e1", events.TypeInvariantCrystallized, "2026-01-01T00:00:00Z",
			map[string]string{"id": "INV-001"}, nil),
		makeEvt("e2", events.TypeInvariantUpdated, "2026-01-02T00:00:00Z",
			map[string]string{"invariant_id": "INV-001"}, nil),
		makeEvt("e3", events.TypeADRCrystallized, "2026-01-03T00:00:00Z",
			map[string]string{"id": "ADR-001"}, nil),
	}

	chain := Provenance(evts, "INV-001")
	if len(chain) != 2 {
		t.Fatalf("expected 2 events for INV-001, got %d", len(chain))
	}
	if chain[0].ID != "e1" || chain[1].ID != "e2" {
		t.Errorf("expected [e1,e2], got [%s,%s]", chain[0].ID, chain[1].ID)
	}

	// No provenance for unknown element
	empty := Provenance(evts, "INV-999")
	if len(empty) != 0 {
		t.Errorf("expected 0 events for INV-999, got %d", len(empty))
	}
}

func TestBisect_FindsIntroducingEvent(t *testing.T) {
	// APP-INV-082: binary search finds first defect-introducing event
	evts := make([]*events.Event, 10)
	for i := 0; i < 10; i++ {
		evts[i] = makeEvt(fmt.Sprintf("e%d", i), "t", fmt.Sprintf("2026-01-%02dT00:00:00Z", i+1), nil, nil)
	}

	// Defect introduced at position 5
	predicate := func(prefix []*events.Event) (bool, error) {
		return len(prefix) >= 6, nil // defect present when we have 6+ events
	}

	result, err := Bisect(evts, predicate)
	if err != nil {
		t.Fatalf("Bisect: %v", err)
	}
	if result.ID != "e5" {
		t.Errorf("expected introducing event e5, got %s", result.ID)
	}
}

func TestBisect_FirstEvent(t *testing.T) {
	evts := []*events.Event{
		makeEvt("e0", "t", "2026-01-01T00:00:00Z", nil, nil),
		makeEvt("e1", "t", "2026-01-02T00:00:00Z", nil, nil),
	}

	// Defect present from the very first event
	predicate := func(prefix []*events.Event) (bool, error) {
		return true, nil
	}

	result, err := Bisect(evts, predicate)
	if err != nil {
		t.Fatalf("Bisect: %v", err)
	}
	if result.ID != "e0" {
		t.Errorf("expected e0 (first event), got %s", result.ID)
	}
}

func TestBisect_NoDefect(t *testing.T) {
	evts := []*events.Event{
		makeEvt("e0", "t", "2026-01-01T00:00:00Z", nil, nil),
	}

	predicate := func(prefix []*events.Event) (bool, error) {
		return false, nil
	}

	_, err := Bisect(evts, predicate)
	if err == nil {
		t.Fatal("expected error when no defect found")
	}
}

func TestBisect_Empty(t *testing.T) {
	_, err := Bisect(nil, func(prefix []*events.Event) (bool, error) {
		return true, nil
	})
	if err == nil {
		t.Fatal("expected error for empty log")
	}
}
