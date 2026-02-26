package events

// ddis:implements APP-ADR-041 (challenge-feedback loop — event stream wiring)
// ddis:maintains APP-INV-053 (event stream completeness)

import (
	"os"
	"path/filepath"
)

// EmitSpecEvent creates and appends an event to Stream 2 (Specification).
// The eventsDir should be the .ddis/events directory.
// Silently skips if the events directory doesn't exist.
func EmitSpecEvent(eventsDir, eventType, specHash string, payload interface{}) error {
	return emitToStream(eventsDir, StreamSpecification, eventType, specHash, payload)
}

// EmitImplEvent creates and appends an event to Stream 3 (Implementation).
// Silently skips if the events directory doesn't exist.
func EmitImplEvent(eventsDir, eventType, specHash string, payload interface{}) error {
	return emitToStream(eventsDir, StreamImplementation, eventType, specHash, payload)
}

func emitToStream(eventsDir string, stream Stream, eventType, specHash string, payload interface{}) error {
	if _, err := os.Stat(eventsDir); os.IsNotExist(err) {
		return nil // no events directory, skip silently
	}

	evt, err := NewEvent(stream, eventType, specHash, payload)
	if err != nil {
		return err
	}

	streamFile := filepath.Join(eventsDir, stream.File())
	return AppendEvent(streamFile, evt)
}
