package discovery

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// ---------------------------------------------------------------------------
// ReduceToState
// ---------------------------------------------------------------------------

func TestReduceToState_EmptyFile(t *testing.T) {
	f := filepath.Join(t.TempDir(), "empty.jsonl")
	if err := os.WriteFile(f, []byte(""), 0644); err != nil {
		t.Fatal(err)
	}
	state, err := ReduceToState(f)
	if err != nil {
		t.Fatalf("ReduceToState on empty file: %v", err)
	}
	if len(state.ArtifactMap) != 0 {
		t.Errorf("expected empty artifact map, got %d entries", len(state.ArtifactMap))
	}
	if len(state.Findings) != 0 {
		t.Errorf("expected empty findings, got %d", len(state.Findings))
	}
	if len(state.OpenQuestions) != 0 {
		t.Errorf("expected empty open questions, got %d", len(state.OpenQuestions))
	}
	if len(state.Threads) != 0 {
		t.Errorf("expected empty threads, got %d", len(state.Threads))
	}
}

func TestReduceToState_Crystallize(t *testing.T) {
	events := []DiscoveryEvent{
		{
			Timestamp: "2026-01-15T10:00:00Z",
			Type:      "decision_crystallized",
			Data: map[string]interface{}{
				"artifact_id":   "INV-001",
				"artifact_type": "invariant",
				"title":         "Never delete data",
				"domain":        "storage",
			},
		},
		{
			Timestamp: "2026-01-15T10:01:00Z",
			Type:      "decision_crystallized",
			Data: map[string]interface{}{
				"artifact_id":   "ADR-001",
				"artifact_type": "adr",
				"title":         "Use SQLite",
				"domain":        "storage",
			},
		},
	}
	f := writeEventsToJSONL(t, events)

	state, err := ReduceToState(f)
	if err != nil {
		t.Fatalf("ReduceToState: %v", err)
	}
	if len(state.ArtifactMap) != 2 {
		t.Fatalf("expected 2 artifacts, got %d", len(state.ArtifactMap))
	}

	inv, ok := state.ArtifactMap["INV-001"]
	if !ok {
		t.Fatal("INV-001 not found in artifact map")
	}
	if inv.ArtifactType != "invariant" {
		t.Errorf("expected type 'invariant', got %q", inv.ArtifactType)
	}
	if inv.Title != "Never delete data" {
		t.Errorf("expected title 'Never delete data', got %q", inv.Title)
	}
	if inv.Status != "active" {
		t.Errorf("expected status 'active', got %q", inv.Status)
	}
	if inv.Domain != "storage" {
		t.Errorf("expected domain 'storage', got %q", inv.Domain)
	}
}

func TestReduceToState_FindingAndQuestionLifecycle(t *testing.T) {
	events := []DiscoveryEvent{
		{Timestamp: "2026-01-01T01:00:00Z", Type: "finding_recorded", Data: map[string]interface{}{"id": "f1"}},
		{Timestamp: "2026-01-01T01:01:00Z", Type: "question_opened", Data: map[string]interface{}{"id": "q1"}},
		{Timestamp: "2026-01-01T01:02:00Z", Type: "question_opened", Data: map[string]interface{}{"id": "q2"}},
		{Timestamp: "2026-01-01T01:03:00Z", Type: "question_resolved", Data: map[string]interface{}{"id": "q1"}},
	}
	f := writeEventsToJSONL(t, events)

	state, err := ReduceToState(f)
	if err != nil {
		t.Fatalf("ReduceToState: %v", err)
	}
	if len(state.Findings) != 1 {
		t.Errorf("expected 1 finding, got %d", len(state.Findings))
	}
	if len(state.OpenQuestions) != 1 {
		t.Errorf("expected 1 open question (q2), got %d", len(state.OpenQuestions))
	}
	if _, ok := state.OpenQuestions["q2"]; !ok {
		t.Error("expected q2 to remain open")
	}
}

func TestReduceToState_ThreadParked(t *testing.T) {
	events := []DiscoveryEvent{
		{Timestamp: "2026-01-01T01:00:00Z", Type: "thread_created", ThreadID: "t-1"},
		{Timestamp: "2026-01-01T01:01:00Z", Type: "thread_parked", ThreadID: "t-1"},
	}
	f := writeEventsToJSONL(t, events)

	state, err := ReduceToState(f)
	if err != nil {
		t.Fatalf("ReduceToState: %v", err)
	}
	ts, ok := state.Threads["t-1"]
	if !ok {
		t.Fatal("thread t-1 not found")
	}
	if ts.Status != "parked" {
		t.Errorf("expected thread status 'parked', got %q", ts.Status)
	}
}

func TestReduceToState_ThreadMerged(t *testing.T) {
	events := []DiscoveryEvent{
		{Timestamp: "2026-01-01T01:00:00Z", Type: "thread_created", ThreadID: "t-src"},
		{Timestamp: "2026-01-01T01:01:00Z", Type: "thread_created", ThreadID: "t-tgt"},
		{Timestamp: "2026-01-01T01:02:00Z", Type: "thread_merged", ThreadID: "t-src"},
	}
	f := writeEventsToJSONL(t, events)

	state, err := ReduceToState(f)
	if err != nil {
		t.Fatalf("ReduceToState: %v", err)
	}
	if state.Threads["t-src"].Status != "merged" {
		t.Errorf("expected t-src status 'merged', got %q", state.Threads["t-src"].Status)
	}
	if state.Threads["t-tgt"].Status != "active" {
		t.Errorf("expected t-tgt status 'active', got %q", state.Threads["t-tgt"].Status)
	}
}

func TestReduceToState_ArtifactAmended(t *testing.T) {
	events := []DiscoveryEvent{
		{
			Timestamp: "2026-01-01T01:00:00Z",
			Type:      "decision_crystallized",
			Data: map[string]interface{}{
				"artifact_id":   "INV-001",
				"artifact_type": "invariant",
				"title":         "Original",
			},
		},
		{
			Timestamp: "2026-01-01T01:01:00Z",
			Type:      "artifact_amended",
			Data: map[string]interface{}{
				"artifact_id": "INV-001",
				"change":      "added violation scenario",
			},
		},
	}
	f := writeEventsToJSONL(t, events)

	state, err := ReduceToState(f)
	if err != nil {
		t.Fatalf("ReduceToState: %v", err)
	}
	inv := state.ArtifactMap["INV-001"]
	if len(inv.Amendments) != 1 {
		t.Fatalf("expected 1 amendment, got %d", len(inv.Amendments))
	}
}

func TestReduceToState_ArtifactDeleted(t *testing.T) {
	events := []DiscoveryEvent{
		{
			Timestamp: "2026-01-01T01:00:00Z",
			Type:      "decision_crystallized",
			Data:      map[string]interface{}{"artifact_id": "INV-001", "artifact_type": "invariant", "title": "X"},
		},
		{
			Timestamp: "2026-01-01T01:01:00Z",
			Type:      "artifact_deleted",
			Data:      map[string]interface{}{"artifact_id": "INV-001"},
		},
	}
	f := writeEventsToJSONL(t, events)

	state, err := ReduceToState(f)
	if err != nil {
		t.Fatalf("ReduceToState: %v", err)
	}
	if state.ArtifactMap["INV-001"].Status != "deleted" {
		t.Errorf("expected status 'deleted', got %q", state.ArtifactMap["INV-001"].Status)
	}
}

func TestReduceToState_MissingFile(t *testing.T) {
	_, err := ReduceToState("/nonexistent/path.jsonl")
	if err == nil {
		t.Error("expected error for missing file")
	}
}

func TestReduceToState_ThreadIDFromData(t *testing.T) {
	// When ThreadID is empty, it should fallback to data["thread_id"].
	events := []DiscoveryEvent{
		{
			Timestamp: "2026-01-01T01:00:00Z",
			Type:      "thread_created",
			ThreadID:  "",
			Data:      map[string]interface{}{"thread_id": "t-from-data"},
		},
	}
	f := writeEventsToJSONL(t, events)

	state, err := ReduceToState(f)
	if err != nil {
		t.Fatalf("ReduceToState: %v", err)
	}
	if _, ok := state.Threads["t-from-data"]; !ok {
		t.Error("expected thread 't-from-data' from data fallback")
	}
}

// ---------------------------------------------------------------------------
// DeriveTasks
// ---------------------------------------------------------------------------

func TestDeriveTasks_Invariant(t *testing.T) {
	state := &DiscoveryState{
		ArtifactMap: map[string]*ArtifactEntry{
			"INV-001": {
				ArtifactID:   "INV-001",
				ArtifactType: "invariant",
				Title:        "No data loss",
				Status:       "active",
			},
		},
	}
	result, err := DeriveTasks(state, nil)
	if err != nil {
		t.Fatalf("DeriveTasks: %v", err)
	}
	if result.TotalTasks != 2 {
		t.Fatalf("expected 2 tasks for invariant, got %d", result.TotalTasks)
	}
	// One should be impl, one should be test.
	types := map[string]bool{}
	for _, task := range result.Tasks {
		types[task.Type] = true
		if task.Metadata.DerivationRule != 2 {
			t.Errorf("expected derivation rule 2 for invariant, got %d", task.Metadata.DerivationRule)
		}
		if task.Priority != 1 {
			t.Errorf("expected priority 1 for invariant task, got %d", task.Priority)
		}
	}
	if !types["task"] || !types["test"] {
		t.Errorf("expected both 'task' and 'test' types, got %v", types)
	}
	if result.ByRule[2] != 2 {
		t.Errorf("expected ByRule[2]=2, got %d", result.ByRule[2])
	}
}

func TestDeriveTasks_ADR(t *testing.T) {
	state := &DiscoveryState{
		ArtifactMap: map[string]*ArtifactEntry{
			"ADR-001": {
				ArtifactID:   "ADR-001",
				ArtifactType: "adr",
				Title:        "Use SQLite",
				Status:       "active",
			},
		},
	}
	result, err := DeriveTasks(state, nil)
	if err != nil {
		t.Fatalf("DeriveTasks: %v", err)
	}
	if result.TotalTasks != 1 {
		t.Fatalf("expected 1 task for ADR, got %d", result.TotalTasks)
	}
	if result.Tasks[0].Metadata.DerivationRule != 1 {
		t.Errorf("expected derivation rule 1 for ADR, got %d", result.Tasks[0].Metadata.DerivationRule)
	}
}

func TestDeriveTasks_Gate(t *testing.T) {
	state := &DiscoveryState{
		ArtifactMap: map[string]*ArtifactEntry{
			"GATE-001": {
				ArtifactID:   "GATE-001",
				ArtifactType: "gate",
				Title:        "Validation pass",
				Status:       "active",
			},
		},
	}
	result, err := DeriveTasks(state, nil)
	if err != nil {
		t.Fatalf("DeriveTasks: %v", err)
	}
	if result.TotalTasks != 1 {
		t.Fatalf("expected 1 task for gate, got %d", result.TotalTasks)
	}
	if result.Tasks[0].Metadata.DerivationRule != 5 {
		t.Errorf("expected derivation rule 5 for gate, got %d", result.Tasks[0].Metadata.DerivationRule)
	}
}

func TestDeriveTasks_NegativeSpec(t *testing.T) {
	state := &DiscoveryState{
		ArtifactMap: map[string]*ArtifactEntry{
			"NEG-001": {
				ArtifactID:   "NEG-001",
				ArtifactType: "negative_spec",
				Title:        "Must not allow injection",
				Status:       "active",
			},
		},
	}
	result, err := DeriveTasks(state, nil)
	if err != nil {
		t.Fatalf("DeriveTasks: %v", err)
	}
	if result.TotalTasks != 2 {
		t.Fatalf("expected 2 tasks for negative_spec, got %d", result.TotalTasks)
	}
	if result.ByRule[3] != 2 {
		t.Errorf("expected ByRule[3]=2, got %d", result.ByRule[3])
	}
}

func TestDeriveTasks_Glossary(t *testing.T) {
	state := &DiscoveryState{
		ArtifactMap: map[string]*ArtifactEntry{
			"GLOSS-001": {
				ArtifactID:   "GLOSS-001",
				ArtifactType: "glossary",
				Title:        "Drift",
				Status:       "active",
			},
		},
	}
	result, err := DeriveTasks(state, nil)
	if err != nil {
		t.Fatalf("DeriveTasks: %v", err)
	}
	if result.TotalTasks != 1 {
		t.Fatalf("expected 1 task for glossary, got %d", result.TotalTasks)
	}
	if result.Tasks[0].Priority != 3 {
		t.Errorf("expected priority 3 for glossary task, got %d", result.Tasks[0].Priority)
	}
}

func TestDeriveTasks_CrossRef(t *testing.T) {
	state := &DiscoveryState{
		ArtifactMap: map[string]*ArtifactEntry{
			"XREF-001": {
				ArtifactID:   "XREF-001",
				ArtifactType: "cross_ref",
				Title:        "",
				Status:       "active",
				Data: map[string]interface{}{
					"source": "INV-001",
					"target": "INV-002",
				},
			},
		},
	}
	result, err := DeriveTasks(state, nil)
	if err != nil {
		t.Fatalf("DeriveTasks: %v", err)
	}
	if result.TotalTasks != 1 {
		t.Fatalf("expected 1 task for cross_ref, got %d", result.TotalTasks)
	}
	if result.ByRule[8] != 1 {
		t.Errorf("expected ByRule[8]=1, got %d", result.ByRule[8])
	}
}

func TestDeriveTasks_DeletedArtifact(t *testing.T) {
	state := &DiscoveryState{
		ArtifactMap: map[string]*ArtifactEntry{
			"INV-DEL": {
				ArtifactID:   "INV-DEL",
				ArtifactType: "invariant",
				Title:        "Deleted thing",
				Status:       "deleted",
			},
		},
	}
	result, err := DeriveTasks(state, nil)
	if err != nil {
		t.Fatalf("DeriveTasks: %v", err)
	}
	if result.TotalTasks != 3 {
		t.Fatalf("expected 3 tasks for deleted artifact, got %d", result.TotalTasks)
	}
	if result.ByRule[7] != 3 {
		t.Errorf("expected ByRule[7]=3, got %d", result.ByRule[7])
	}
}

func TestDeriveTasks_WithAmendments(t *testing.T) {
	state := &DiscoveryState{
		ArtifactMap: map[string]*ArtifactEntry{
			"ADR-001": {
				ArtifactID:   "ADR-001",
				ArtifactType: "adr",
				Title:        "Some ADR",
				Status:       "active",
				Amendments: []map[string]interface{}{
					{"change": "added tests"},
				},
			},
		},
	}
	result, err := DeriveTasks(state, nil)
	if err != nil {
		t.Fatalf("DeriveTasks: %v", err)
	}
	// 1 ADR impl task (rule 1) + 2 amendment tasks (rule 6) = 3
	if result.TotalTasks != 3 {
		t.Fatalf("expected 3 tasks (1 ADR + 2 amendment), got %d", result.TotalTasks)
	}
	if result.ByRule[6] != 2 {
		t.Errorf("expected ByRule[6]=2, got %d", result.ByRule[6])
	}
}

func TestDeriveTasks_EmptyState(t *testing.T) {
	state := &DiscoveryState{
		ArtifactMap: map[string]*ArtifactEntry{},
	}
	result, err := DeriveTasks(state, nil)
	if err != nil {
		t.Fatalf("DeriveTasks: %v", err)
	}
	if result.TotalTasks != 0 {
		t.Errorf("expected 0 tasks for empty state, got %d", result.TotalTasks)
	}
}

// ---------------------------------------------------------------------------
// FormatBeads
// ---------------------------------------------------------------------------

func TestFormatBeads(t *testing.T) {
	result := &TasksResult{
		Tasks: []DerivedTask{
			{
				ID:                 "TASK-INV-001-impl",
				Title:              "Implement constraint",
				Type:               "task",
				Priority:           1,
				Labels:             []string{"constraint"},
				AcceptanceCriteria: "Test passes",
				Metadata: TaskMetadata{
					SourceArtifact: "INV-001",
					DerivationRule: 2,
				},
			},
		},
	}
	output := FormatBeads(result)
	if output == "" {
		t.Fatal("FormatBeads returned empty output")
	}

	// Should be valid JSONL (one JSON object per line).
	lines := strings.Split(strings.TrimSpace(output), "\n")
	if len(lines) != 1 {
		t.Fatalf("expected 1 line, got %d", len(lines))
	}

	var parsed map[string]interface{}
	if err := json.Unmarshal([]byte(lines[0]), &parsed); err != nil {
		t.Fatalf("invalid JSON in FormatBeads output: %v", err)
	}
	if parsed["title"] != "Implement constraint" {
		t.Errorf("expected title 'Implement constraint', got %v", parsed["title"])
	}
}

func TestFormatBeads_Empty(t *testing.T) {
	result := &TasksResult{}
	output := FormatBeads(result)
	if output != "" {
		t.Errorf("expected empty output for no tasks, got %q", output)
	}
}

// ---------------------------------------------------------------------------
// FormatJSON
// ---------------------------------------------------------------------------

func TestFormatJSON(t *testing.T) {
	result := &TasksResult{
		Tasks: []DerivedTask{
			{ID: "TASK-1", Title: "Do something", Type: "task"},
		},
		TotalTasks: 1,
		ByRule:     map[int]int{1: 1},
	}
	output, err := FormatJSON(result)
	if err != nil {
		t.Fatalf("FormatJSON: %v", err)
	}
	if output == "" {
		t.Fatal("FormatJSON returned empty string")
	}

	// Verify it is valid JSON.
	var parsed TasksResult
	if err := json.Unmarshal([]byte(output), &parsed); err != nil {
		t.Fatalf("FormatJSON output is not valid JSON: %v", err)
	}
	if parsed.TotalTasks != 1 {
		t.Errorf("expected total_tasks=1, got %d", parsed.TotalTasks)
	}
}

// ---------------------------------------------------------------------------
// FormatMarkdown
// ---------------------------------------------------------------------------

func TestFormatMarkdown(t *testing.T) {
	result := &TasksResult{
		Tasks: []DerivedTask{
			{
				ID:                 "TASK-1",
				Title:              "Implement feature",
				Type:               "task",
				Priority:           1,
				AcceptanceCriteria: "Feature works",
				Metadata:           TaskMetadata{SourceArtifact: "INV-001"},
			},
			{
				ID:                 "TASK-2",
				Title:              "Write tests",
				Type:               "test",
				Priority:           2,
				AcceptanceCriteria: "Tests pass",
				DependsOn:          []string{"TASK-1"},
				Metadata:           TaskMetadata{SourceArtifact: "INV-001"},
			},
		},
	}
	output := FormatMarkdown(result)
	if !strings.Contains(output, "## Tasks") {
		t.Error("expected markdown header '## Tasks'")
	}
	if !strings.Contains(output, "- [ ]") {
		t.Error("expected checkbox markers")
	}
	if !strings.Contains(output, "P1:") {
		t.Error("expected priority marker P1")
	}
	if !strings.Contains(output, "Depends on:") {
		t.Error("expected dependency information")
	}
	if !strings.Contains(output, "Acceptance:") {
		t.Error("expected acceptance criteria")
	}
}

func TestFormatMarkdown_Empty(t *testing.T) {
	result := &TasksResult{}
	output := FormatMarkdown(result)
	if !strings.Contains(output, "## Tasks") {
		t.Error("expected header even for empty result")
	}
}

// ---------------------------------------------------------------------------
// getString (helper)
// ---------------------------------------------------------------------------

func TestGetString_Present(t *testing.T) {
	m := map[string]interface{}{"key": "value"}
	if got := getString(m, "key"); got != "value" {
		t.Errorf("expected 'value', got %q", got)
	}
}

func TestGetString_Missing(t *testing.T) {
	m := map[string]interface{}{"other": "value"}
	if got := getString(m, "key"); got != "" {
		t.Errorf("expected empty string for missing key, got %q", got)
	}
}

func TestGetString_WrongType(t *testing.T) {
	m := map[string]interface{}{"key": 42}
	if got := getString(m, "key"); got != "" {
		t.Errorf("expected empty string for non-string value, got %q", got)
	}
}

func TestGetString_NilMap(t *testing.T) {
	if got := getString(nil, "key"); got != "" {
		t.Errorf("expected empty string for nil map, got %q", got)
	}
}

// ---------------------------------------------------------------------------
// sortedArtifacts (helper)
// ---------------------------------------------------------------------------

func TestSortedArtifacts_Order(t *testing.T) {
	m := map[string]*ArtifactEntry{
		"C": {ArtifactID: "C"},
		"A": {ArtifactID: "A"},
		"B": {ArtifactID: "B"},
	}
	sorted := sortedArtifacts(m)
	if len(sorted) != 3 {
		t.Fatalf("expected 3 entries, got %d", len(sorted))
	}
	if sorted[0].ArtifactID != "A" || sorted[1].ArtifactID != "B" || sorted[2].ArtifactID != "C" {
		t.Errorf("expected alphabetical order, got %s %s %s",
			sorted[0].ArtifactID, sorted[1].ArtifactID, sorted[2].ArtifactID)
	}
}

func TestSortedArtifacts_Empty(t *testing.T) {
	sorted := sortedArtifacts(map[string]*ArtifactEntry{})
	if len(sorted) != 0 {
		t.Errorf("expected empty slice, got %d entries", len(sorted))
	}
}

// ---------------------------------------------------------------------------
// Phase dependencies in DeriveTasks
// ---------------------------------------------------------------------------

func TestDeriveTasks_InterPhaseDependencies(t *testing.T) {
	state := &DiscoveryState{
		ArtifactMap: map[string]*ArtifactEntry{
			"ADR-001": {ArtifactID: "ADR-001", ArtifactType: "adr", Title: "Phase 0 ADR", Status: "active"},
			"INV-001": {ArtifactID: "INV-001", ArtifactType: "invariant", Title: "Phase 1 INV", Status: "active"},
		},
	}
	phases := [][]string{
		{"ADR-001"},
		{"INV-001"},
	}
	result, err := DeriveTasks(state, phases)
	if err != nil {
		t.Fatalf("DeriveTasks: %v", err)
	}

	// Phase 1 tasks should depend on phase 0 tasks.
	for _, task := range result.Tasks {
		if task.Metadata.SourceArtifact == "INV-001" {
			if len(task.DependsOn) == 0 {
				t.Errorf("task %s should depend on phase 0 tasks", task.ID)
			}
		}
	}
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

func writeEventsToJSONL(t *testing.T, events []DiscoveryEvent) string {
	t.Helper()
	f := filepath.Join(t.TempDir(), "discovery.jsonl")
	var sb strings.Builder
	for _, ev := range events {
		line, err := json.Marshal(ev)
		if err != nil {
			t.Fatalf("marshal event: %v", err)
		}
		sb.Write(line)
		sb.WriteByte('\n')
	}
	if err := os.WriteFile(f, []byte(sb.String()), 0644); err != nil {
		t.Fatalf("write JSONL: %v", err)
	}
	return f
}
