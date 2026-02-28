package events

// ddis:implements APP-ADR-015 (three-stream event sourcing — event envelope)
// ddis:implements APP-INV-020 (event stream append-only — immutable records)
// ddis:maintains APP-INV-027 (thread topology primacy — event envelope carries thread_id as primary field)
// ddis:maintains APP-INV-029 (convergent thread selection — events carry thread_id from convergent selection)

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"time"
)

// Stream identifies which JSONL stream an event belongs to.
type Stream int

const (
	StreamDiscovery      Stream = 1 // .ddis/events/stream-1.jsonl — ideas, questions, decisions
	StreamSpecification  Stream = 2 // .ddis/events/stream-2.jsonl — parse, validate, drift
	StreamImplementation Stream = 3 // .ddis/events/stream-3.jsonl — issues, status, findings
)

// StreamFile returns the canonical filename for a stream.
func (s Stream) File() string {
	return fmt.Sprintf("stream-%d.jsonl", int(s))
}

// Event is the common envelope for all three streams.
// Matches the spec at code-bridge.md §Envelope Schema and
// event-sourcing.md §Event Struct Extension.
type Event struct {
	ID        string          `json:"id"`
	Type      string          `json:"type"`
	Timestamp string          `json:"timestamp"`
	SpecHash  string          `json:"spec_hash"`
	Stream    Stream          `json:"stream"`
	Payload   json.RawMessage `json:"payload"`
	Causes    []string        `json:"causes,omitempty"`  // APP-INV-074: IDs of causally preceding events
	Version   int             `json:"version,omitempty"` // APP-INV-072: schema version for forward compat
}

// ContentHash computes SHA-256 of the canonical JSON payload.
// Event equality: equal(e_a, e_b) = sha256(json(e_a.payload)) == sha256(json(e_b.payload))
// per APP-INV-020.
func (e *Event) ContentHash() string {
	h := sha256.Sum256(e.Payload)
	return fmt.Sprintf("sha256:%x", h)
}

// NewEvent creates a new event with auto-populated ID and timestamp.
// The caller provides stream, type, spec_hash, and payload.
func NewEvent(stream Stream, eventType string, specHash string, payload interface{}) (*Event, error) {
	data, err := json.Marshal(payload)
	if err != nil {
		return nil, fmt.Errorf("marshal event payload: %w", err)
	}

	now := time.Now().UTC()
	id := fmt.Sprintf("evt-%s-%d", now.Format("20060102"), now.UnixMilli()%100000)

	return &Event{
		ID:        id,
		Type:      eventType,
		Timestamp: now.Format(time.RFC3339),
		SpecHash:  specHash,
		Stream:    stream,
		Payload:   json.RawMessage(data),
	}, nil
}

// EventFilters controls which events are returned by ReadStream.
type EventFilters struct {
	Type        string // filter by event type
	Since       string // filter events after this RFC3339 timestamp
	Limit       int    // max events to return (0 = unlimited)
	ArtifactRef string // filter by artifact reference in payload
}
