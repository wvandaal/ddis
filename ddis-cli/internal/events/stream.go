package events

// ddis:implements APP-ADR-015 (three-stream event sourcing — write/read/correlate)
// ddis:implements APP-INV-020 (event stream append-only — O_APPEND, no modifications)
// ddis:maintains APP-INV-025 (discovery provenance chain — AppendEvent enforces append-only JSONL provenance records)

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// StreamDir returns the canonical events directory within a .ddis workspace.
func StreamDir(workspaceRoot string) string {
	return filepath.Join(workspaceRoot, ".ddis", "events")
}

// StreamPath returns the full path to a stream file.
func StreamPath(workspaceRoot string, stream Stream) string {
	return filepath.Join(StreamDir(workspaceRoot), stream.File())
}

// AppendEvent writes a single event to the appropriate stream file.
//
// Per APP-INV-020 and the negative spec at code-bridge.md:
// - Opens with O_APPEND|O_CREATE|O_WRONLY (no read-write handles)
// - No handle reuse across calls — opened and closed within this function
// - No Seek, Truncate, or WriteAt
// - A function that cannot seek cannot overwrite.
func AppendEvent(streamPath string, event *Event) error {
	if err := ValidateEvent(event); err != nil {
		return fmt.Errorf("invalid event: %w", err)
	}

	f, err := os.OpenFile(streamPath, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)
	if err != nil {
		return fmt.Errorf("open stream %s: %w", streamPath, err)
	}
	defer f.Close()

	enc := json.NewEncoder(f)
	enc.SetEscapeHTML(false)
	if err := enc.Encode(event); err != nil {
		return fmt.Errorf("write event to %s: %w", streamPath, err)
	}

	return nil
}

// ReadStream reads events from a JSONL stream file with optional filters.
//
// Per the spec read path:
// - Opens with os.Open (read-only)
// - bufio.Scanner with 10MB max line buffer
// - Returns matching events in chronological order
func ReadStream(streamPath string, filters EventFilters) ([]*Event, error) {
	f, err := os.Open(streamPath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil // empty stream is valid
		}
		return nil, fmt.Errorf("open stream %s: %w", streamPath, err)
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	scanner.Buffer(make([]byte, 0, 64*1024), 10*1024*1024) // 10MB max line

	var result []*Event
	for scanner.Scan() {
		line := scanner.Bytes()
		if len(line) == 0 {
			continue
		}

		var evt Event
		if err := json.Unmarshal(line, &evt); err != nil {
			continue // skip malformed lines
		}

		if !matchFilters(&evt, &filters) {
			continue
		}

		result = append(result, &evt)

		if filters.Limit > 0 && len(result) >= filters.Limit {
			break
		}
	}

	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("scan stream %s: %w", streamPath, err)
	}

	return result, nil
}

// CorrelateStreams reads all three streams filtered by artifact ID,
// merges by timestamp, and returns a unified timeline.
//
// Per the spec cross-stream correlation:
// 1. Read all three streams filtered by artifact_ref containing artifactID
// 2. Merge by timestamp
// 3. Return unified timeline
func CorrelateStreams(stream1, stream2, stream3, artifactID string) ([]*Event, error) {
	filters := EventFilters{ArtifactRef: artifactID}

	var all []*Event
	for _, path := range []string{stream1, stream2, stream3} {
		events, err := ReadStream(path, filters)
		if err != nil {
			return nil, err
		}
		all = append(all, events...)
	}

	sort.Slice(all, func(i, j int) bool {
		return all[i].Timestamp < all[j].Timestamp
	})

	return all, nil
}

// matchFilters checks whether an event matches the given filters.
func matchFilters(evt *Event, f *EventFilters) bool {
	if f.Type != "" && evt.Type != f.Type {
		return false
	}
	if f.Since != "" && evt.Timestamp < f.Since {
		return false
	}
	if f.ArtifactRef != "" {
		payload := string(evt.Payload)
		if !strings.Contains(payload, f.ArtifactRef) {
			return false
		}
	}
	return true
}
