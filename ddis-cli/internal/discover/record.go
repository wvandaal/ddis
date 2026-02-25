package discover

// ddis:maintains APP-INV-025 (discovery provenance chain)

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// timeNow is a package-level variable so tests can override it.
var timeNow = time.Now

// RecordEvent appends a discovery event to the JSONL stream.
func RecordEvent(eventsDir string, threadID string, eventType string, data map[string]interface{}) error {
	if err := os.MkdirAll(eventsDir, 0755); err != nil {
		return fmt.Errorf("create events dir: %w", err)
	}

	// Count existing events for this thread to compute the next sequence number.
	existing, err := LoadEvents(eventsDir, threadID)
	if err != nil {
		return fmt.Errorf("load existing events: %w", err)
	}
	nextSequence := len(existing)

	event := Event{
		Version:   1,
		Type:      eventType,
		Timestamp: timeNow().UTC().Format(time.RFC3339),
		ThreadID:  threadID,
		Sequence:  nextSequence,
		Data:      data,
	}

	path := filepath.Join(eventsDir, "discovery.jsonl")
	f, err := os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
	if err != nil {
		return fmt.Errorf("open discovery events file: %w", err)
	}
	defer f.Close()

	line, err := json.Marshal(event)
	if err != nil {
		return fmt.Errorf("marshal event: %w", err)
	}
	if _, err := fmt.Fprintf(f, "%s\n", line); err != nil {
		return fmt.Errorf("write event line: %w", err)
	}
	return nil
}

// LoadEvents reads events from the discovery JSONL, optionally filtered by thread.
func LoadEvents(eventsDir string, threadFilter string) ([]Event, error) {
	path := filepath.Join(eventsDir, "discovery.jsonl")
	f, err := os.Open(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("open discovery events file: %w", err)
	}
	defer f.Close()

	var events []Event
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" {
			continue
		}
		var e Event
		if err := json.Unmarshal([]byte(line), &e); err != nil {
			return nil, fmt.Errorf("parse event line: %w", err)
		}
		if threadFilter != "" && e.ThreadID != threadFilter {
			continue
		}
		events = append(events, e)
	}
	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("read discovery events: %w", err)
	}
	return events, nil
}
