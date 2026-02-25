package discovery

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"sort"
)

// ddis:maintains APP-INV-039 (task derivation completeness)

// ReduceToState reads a discovery JSONL file and reduces it to current state.
// The reduction is idempotent: same input produces same output (last-write-wins).
func ReduceToState(jsonlPath string) (*DiscoveryState, error) {
	f, err := os.Open(jsonlPath)
	if err != nil {
		return nil, fmt.Errorf("open discovery JSONL: %w", err)
	}
	defer f.Close()

	// Parse all events.
	var events []DiscoveryEvent
	scanner := bufio.NewScanner(f)
	scanner.Buffer(make([]byte, 1024*1024), 1024*1024) // 1MB line buffer
	lineNum := 0
	for scanner.Scan() {
		lineNum++
		line := scanner.Bytes()
		if len(line) == 0 {
			continue
		}
		var ev DiscoveryEvent
		if err := json.Unmarshal(line, &ev); err != nil {
			return nil, fmt.Errorf("line %d: %w", lineNum, err)
		}
		events = append(events, ev)
	}
	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("read discovery JSONL: %w", err)
	}

	// Sort by timestamp for deterministic replay.
	sort.Slice(events, func(i, j int) bool {
		return events[i].Timestamp < events[j].Timestamp
	})

	// Replay events to build state.
	state := &DiscoveryState{
		ArtifactMap:   make(map[string]*ArtifactEntry),
		Findings:      make(map[string]interface{}),
		OpenQuestions: make(map[string]interface{}),
		Threads:       make(map[string]*ThreadState),
	}

	for _, ev := range events {
		switch ev.Type {
		case "finding_recorded":
			id := getString(ev.Data, "id")
			if id != "" {
				state.Findings[id] = ev.Data
			}

		case "question_opened":
			id := getString(ev.Data, "id")
			if id != "" {
				state.OpenQuestions[id] = ev.Data
			}

		case "question_resolved":
			id := getString(ev.Data, "id")
			delete(state.OpenQuestions, id)

		case "decision_crystallized":
			artID := getString(ev.Data, "artifact_id")
			if artID == "" {
				continue
			}
			entry := &ArtifactEntry{
				ArtifactID:       artID,
				ArtifactType:     getString(ev.Data, "artifact_type"),
				Title:            getString(ev.Data, "title"),
				Domain:           getString(ev.Data, "domain"),
				Status:           "active",
				Tests:            getString(ev.Data, "tests"),
				ValidationMethod: getString(ev.Data, "validation_method"),
				Text:             getString(ev.Data, "text"),
				Data:             ev.Data,
			}
			state.ArtifactMap[artID] = entry

		case "artifact_amended":
			artID := getString(ev.Data, "artifact_id")
			if existing, ok := state.ArtifactMap[artID]; ok {
				existing.Amendments = append(existing.Amendments, ev.Data)
			}

		case "artifact_deleted":
			artID := getString(ev.Data, "artifact_id")
			if existing, ok := state.ArtifactMap[artID]; ok {
				existing.Status = "deleted"
			}

		case "thread_created":
			tid := ev.ThreadID
			if tid == "" {
				tid = getString(ev.Data, "thread_id")
			}
			if tid != "" {
				state.Threads[tid] = &ThreadState{
					ThreadID: tid,
					Status:   "active",
				}
			}

		case "thread_parked":
			tid := ev.ThreadID
			if tid == "" {
				tid = getString(ev.Data, "thread_id")
			}
			if ts, ok := state.Threads[tid]; ok {
				ts.Status = "parked"
			}

		case "thread_merged":
			tid := ev.ThreadID
			if tid == "" {
				tid = getString(ev.Data, "thread_id")
			}
			if ts, ok := state.Threads[tid]; ok {
				ts.Status = "merged"
			}
		}
	}

	return state, nil
}

// getString extracts a string value from a map, returning "" if absent or wrong type.
func getString(m map[string]interface{}, key string) string {
	if v, ok := m[key]; ok {
		if s, ok := v.(string); ok {
			return s
		}
	}
	return ""
}
