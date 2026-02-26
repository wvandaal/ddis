package events

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestStreamFile(t *testing.T) {
	tests := []struct {
		stream Stream
		want   string
	}{
		{StreamDiscovery, "stream-1.jsonl"},
		{StreamSpecification, "stream-2.jsonl"},
		{StreamImplementation, "stream-3.jsonl"},
	}
	for _, tt := range tests {
		got := tt.stream.File()
		if got != tt.want {
			t.Errorf("Stream(%d).File() = %q, want %q", tt.stream, got, tt.want)
		}
	}
}

func TestNewEvent(t *testing.T) {
	payload := map[string]string{"question": "how should caching work?"}
	evt, err := NewEvent(StreamDiscovery, TypeQuestionOpened, "sha256:abc123", payload)
	if err != nil {
		t.Fatalf("NewEvent failed: %v", err)
	}
	if evt.Stream != StreamDiscovery {
		t.Errorf("stream = %d, want %d", evt.Stream, StreamDiscovery)
	}
	if evt.Type != TypeQuestionOpened {
		t.Errorf("type = %q, want %q", evt.Type, TypeQuestionOpened)
	}
	if evt.SpecHash != "sha256:abc123" {
		t.Errorf("spec_hash = %q, want %q", evt.SpecHash, "sha256:abc123")
	}
	if evt.ID == "" {
		t.Error("ID should not be empty")
	}
	if evt.Timestamp == "" {
		t.Error("Timestamp should not be empty")
	}
}

func TestContentHash(t *testing.T) {
	evt1, _ := NewEvent(StreamDiscovery, TypeFindingRecorded, "", map[string]string{"a": "b"})
	evt2, _ := NewEvent(StreamDiscovery, TypeFindingRecorded, "", map[string]string{"a": "b"})
	evt3, _ := NewEvent(StreamDiscovery, TypeFindingRecorded, "", map[string]string{"a": "c"})

	if evt1.ContentHash() != evt2.ContentHash() {
		t.Error("identical payloads should have same content hash")
	}
	if evt1.ContentHash() == evt3.ContentHash() {
		t.Error("different payloads should have different content hashes")
	}
}

func TestValidateEvent(t *testing.T) {
	// Valid event.
	evt, _ := NewEvent(StreamDiscovery, TypeQuestionOpened, "sha256:abc", nil)
	if err := ValidateEvent(evt); err != nil {
		t.Errorf("valid event should pass: %v", err)
	}

	// Wrong stream for type.
	bad := &Event{ID: "x", Timestamp: "t", Stream: StreamSpecification, Type: TypeQuestionOpened}
	if err := ValidateEvent(bad); err == nil {
		t.Error("discovery type on spec stream should fail")
	}

	// Invalid stream number.
	bad2 := &Event{ID: "x", Timestamp: "t", Stream: 4, Type: "foo"}
	if err := ValidateEvent(bad2); err == nil {
		t.Error("stream 4 should fail")
	}

	// Missing ID.
	bad3 := &Event{Timestamp: "t", Stream: StreamDiscovery, Type: TypeFindingRecorded}
	if err := ValidateEvent(bad3); err == nil {
		t.Error("missing ID should fail")
	}
}

func TestAppendAndReadStream(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "stream-1.jsonl")

	// Append two events.
	evt1, _ := NewEvent(StreamDiscovery, TypeFindingRecorded, "sha256:aaa", map[string]string{"finding": "caching helps"})
	evt2, _ := NewEvent(StreamDiscovery, TypeQuestionOpened, "sha256:aaa", map[string]string{"question": "which cache?"})

	if err := AppendEvent(path, evt1); err != nil {
		t.Fatalf("AppendEvent 1: %v", err)
	}
	if err := AppendEvent(path, evt2); err != nil {
		t.Fatalf("AppendEvent 2: %v", err)
	}

	// Read all.
	all, err := ReadStream(path, EventFilters{})
	if err != nil {
		t.Fatalf("ReadStream: %v", err)
	}
	if len(all) != 2 {
		t.Fatalf("expected 2 events, got %d", len(all))
	}

	// Read with type filter.
	filtered, err := ReadStream(path, EventFilters{Type: TypeQuestionOpened})
	if err != nil {
		t.Fatalf("ReadStream filtered: %v", err)
	}
	if len(filtered) != 1 {
		t.Fatalf("expected 1 filtered event, got %d", len(filtered))
	}
	if filtered[0].Type != TypeQuestionOpened {
		t.Errorf("filtered type = %q, want %q", filtered[0].Type, TypeQuestionOpened)
	}

	// Read with limit.
	limited, err := ReadStream(path, EventFilters{Limit: 1})
	if err != nil {
		t.Fatalf("ReadStream limited: %v", err)
	}
	if len(limited) != 1 {
		t.Fatalf("expected 1 limited event, got %d", len(limited))
	}
}

func TestReadStream_NonexistentFile(t *testing.T) {
	events, err := ReadStream("/nonexistent/stream.jsonl", EventFilters{})
	if err != nil {
		t.Fatalf("nonexistent file should return nil, not error: %v", err)
	}
	if events != nil {
		t.Error("nonexistent file should return nil events")
	}
}

func TestAppendEvent_CreatesFile(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "new-stream.jsonl")

	evt, _ := NewEvent(StreamSpecification, TypeSpecParsed, "sha256:bbb", nil)
	if err := AppendEvent(path, evt); err != nil {
		t.Fatalf("AppendEvent to new file: %v", err)
	}

	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Error("file should have been created")
	}
}

func TestAppendEvent_RejectsInvalid(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "stream.jsonl")

	// Wrong stream for type.
	bad := &Event{ID: "x", Timestamp: "t", Stream: StreamImplementation, Type: TypeSpecParsed}
	if err := AppendEvent(path, bad); err == nil {
		t.Error("invalid event should be rejected")
	}

	// File should NOT be created for rejected events.
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Error("file should not exist after rejected append")
	}
}

func TestCorrelateStreams(t *testing.T) {
	dir := t.TempDir()
	s1 := filepath.Join(dir, "stream-1.jsonl")
	s2 := filepath.Join(dir, "stream-2.jsonl")
	s3 := filepath.Join(dir, "stream-3.jsonl")

	// Write events with shared artifact ref.
	e1, _ := NewEvent(StreamDiscovery, TypeDecisionCrystallized, "", map[string]interface{}{
		"artifact_refs": []string{"INV-042"},
	})
	if err := AppendEvent(s1, e1); err != nil {
		t.Fatal(err)
	}

	e2, _ := NewEvent(StreamSpecification, TypeSpecParsed, "", map[string]interface{}{
		"invariants": 42,
	})
	if err := AppendEvent(s2, e2); err != nil {
		t.Fatal(err)
	}

	e3, _ := NewEvent(StreamImplementation, TypeImplementationFinding, "", map[string]interface{}{
		"affected_elements": []string{"INV-042"},
	})
	if err := AppendEvent(s3, e3); err != nil {
		t.Fatal(err)
	}

	// Correlate by INV-042.
	correlated, err := CorrelateStreams(s1, s2, s3, "INV-042")
	if err != nil {
		t.Fatalf("CorrelateStreams: %v", err)
	}

	// Should find events from stream 1 and 3 (both reference INV-042).
	if len(correlated) != 2 {
		t.Errorf("expected 2 correlated events, got %d", len(correlated))
	}
}

func TestAppendEvent_AppendOnly(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "stream.jsonl")

	// Write 3 events.
	for i := 0; i < 3; i++ {
		evt, _ := NewEvent(StreamDiscovery, TypeFindingRecorded, "", map[string]int{"n": i})
		if err := AppendEvent(path, evt); err != nil {
			t.Fatal(err)
		}
	}

	// Read back and verify order preserved.
	events, _ := ReadStream(path, EventFilters{})
	if len(events) != 3 {
		t.Fatalf("expected 3 events, got %d", len(events))
	}
	for i, evt := range events {
		var payload map[string]float64
		json.Unmarshal(evt.Payload, &payload)
		if int(payload["n"]) != i {
			t.Errorf("event %d: n=%v, want %d", i, payload["n"], i)
		}
	}
}

func TestArtifactRefFilter(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "stream-1.jsonl")

	// Event WITH artifact ref.
	e1, _ := NewEvent(StreamDiscovery, TypeDecisionCrystallized, "", map[string]interface{}{
		"artifact_refs": []string{"ADR-015"},
	})
	AppendEvent(path, e1)

	// Event WITHOUT the ref.
	e2, _ := NewEvent(StreamDiscovery, TypeFindingRecorded, "", map[string]string{
		"finding": "unrelated",
	})
	AppendEvent(path, e2)

	filtered, _ := ReadStream(path, EventFilters{ArtifactRef: "ADR-015"})
	if len(filtered) != 1 {
		t.Errorf("expected 1 filtered event, got %d", len(filtered))
	}
}
