package discover

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// ---------------------------------------------------------------------------
// ConvergeThread
// ---------------------------------------------------------------------------

func TestConvergeThread_UserOverride(t *testing.T) {
	threads := []Thread{
		{ID: "t-1", Status: "active", Summary: "auth flow design"},
	}
	result := ConvergeThread("something unrelated", threads, "t-override")
	if result.ThreadID != "t-override" {
		t.Errorf("expected thread ID 't-override', got %q", result.ThreadID)
	}
	if result.Method != "user_override" {
		t.Errorf("expected method 'user_override', got %q", result.Method)
	}
	if result.Score != 1.0 {
		t.Errorf("expected score 1.0, got %f", result.Score)
	}
}

func TestConvergeThread_EmptyThreads(t *testing.T) {
	result := ConvergeThread("some content", nil, "")
	if result.Method != "new_thread" {
		t.Errorf("expected method 'new_thread' when no threads, got %q", result.Method)
	}
	if !strings.HasPrefix(result.ThreadID, "t-") {
		t.Errorf("expected new thread ID starting with 't-', got %q", result.ThreadID)
	}
}

func TestConvergeThread_EmptyContent(t *testing.T) {
	threads := []Thread{
		{ID: "t-1", Status: "active", Summary: "auth flow design"},
	}
	result := ConvergeThread("", threads, "")
	if result.Method != "new_thread" {
		t.Errorf("expected method 'new_thread' for empty content, got %q", result.Method)
	}
}

func TestConvergeThread_MatchingKeywords(t *testing.T) {
	threads := []Thread{
		{ID: "t-auth", Status: "active", Summary: "authentication authorization flow design"},
		{ID: "t-data", Status: "active", Summary: "database migration schema"},
	}
	// Content overlaps heavily with the auth thread
	result := ConvergeThread("authentication flow authorization login", threads, "")
	if result.Method != "convergent" {
		t.Errorf("expected method 'convergent', got %q", result.Method)
	}
	if result.ThreadID != "t-auth" {
		t.Errorf("expected match to 't-auth', got %q", result.ThreadID)
	}
	if result.Score < 0.4 {
		t.Errorf("expected score >= 0.4, got %f", result.Score)
	}
}

func TestConvergeThread_NoMatchCreatesNewThread(t *testing.T) {
	threads := []Thread{
		{ID: "t-auth", Status: "active", Summary: "authentication authorization flow"},
	}
	// Content has no overlap with the thread summary
	result := ConvergeThread("completely unrelated banana elephant", threads, "")
	if result.Method != "new_thread" {
		t.Errorf("expected method 'new_thread' for low overlap, got %q", result.Method)
	}
}

func TestConvergeThread_SkipsParkedThreads(t *testing.T) {
	threads := []Thread{
		{ID: "t-parked", Status: "parked", Summary: "authentication flow design"},
		{ID: "t-active", Status: "active", Summary: "database migration schema"},
	}
	result := ConvergeThread("authentication flow design", threads, "")
	// The parked thread has better overlap but should be skipped
	if result.ThreadID == "t-parked" {
		t.Error("should not match a parked thread")
	}
}

func TestConvergeThread_RecencyBoost(t *testing.T) {
	now := time.Now().UTC()
	recent := now.Add(-1 * time.Hour).Format(time.RFC3339)
	old := now.Add(-48 * time.Hour).Format(time.RFC3339)

	threads := []Thread{
		{ID: "t-old", Status: "active", Summary: "common word design", LastEventAt: old},
		{ID: "t-recent", Status: "active", Summary: "common word design", LastEventAt: recent},
	}
	// Both threads have identical summaries; recency boost should favor t-recent.
	result := ConvergeThread("common word design pattern", threads, "")
	if result.Method == "convergent" && result.ThreadID != "t-recent" {
		t.Errorf("expected recency boost to favor 't-recent', got %q", result.ThreadID)
	}
}

// ---------------------------------------------------------------------------
// ClassifyMode
// ---------------------------------------------------------------------------

func TestClassifyMode_EmptyEvents(t *testing.T) {
	mode := ClassifyMode(nil)
	if mode.Mode != "divergent" {
		t.Errorf("expected 'divergent' for empty events, got %q", mode.Mode)
	}
	if mode.Confidence != 0.0 {
		t.Errorf("expected confidence 0.0, got %f", mode.Confidence)
	}
	if mode.DoFHint != "very_high" {
		t.Errorf("expected DoF hint 'very_high', got %q", mode.DoFHint)
	}
}

func TestClassifyMode_CrystallizationDominant(t *testing.T) {
	events := []Event{
		{Type: "decision_crystallized", Timestamp: "2026-01-01T01:00:00Z"},
		{Type: "decision_crystallized", Timestamp: "2026-01-01T01:01:00Z"},
		{Type: "decision_crystallized", Timestamp: "2026-01-01T01:02:00Z"},
		{Type: "finding_recorded", Timestamp: "2026-01-01T01:03:00Z"},
	}
	mode := ClassifyMode(events)
	if mode.Mode != "crystallization" {
		t.Errorf("expected 'crystallization' when decision_crystallized is dominant, got %q", mode.Mode)
	}
	if mode.DoFHint != "very_low" {
		t.Errorf("expected DoF hint 'very_low', got %q", mode.DoFHint)
	}
}

func TestClassifyMode_DialecticalEvents(t *testing.T) {
	events := []Event{
		{Type: "challenge_posed", Timestamp: "2026-01-01T01:00:00Z"},
		{Type: "finding_recorded", Timestamp: "2026-01-01T01:01:00Z"},
		{Type: "challenge_posed", Timestamp: "2026-01-01T01:02:00Z"},
		{Type: "finding_recorded", Timestamp: "2026-01-01T01:03:00Z"},
	}
	mode := ClassifyMode(events)
	if mode.Mode != "dialectical" {
		t.Errorf("expected 'dialectical' for challenge+finding mix, got %q", mode.Mode)
	}
}

func TestClassifyMode_DivergentEvents(t *testing.T) {
	events := []Event{
		{Type: "question_opened", Timestamp: "2026-01-01T01:00:00Z"},
		{Type: "question_opened", Timestamp: "2026-01-01T01:01:00Z"},
		{Type: "finding_recorded", Timestamp: "2026-01-01T01:02:00Z"},
	}
	mode := ClassifyMode(events)
	if mode.Mode != "divergent" {
		t.Errorf("expected 'divergent' for question+finding mix, got %q", mode.Mode)
	}
}

func TestClassifyMode_ConvergentEvents(t *testing.T) {
	events := []Event{
		{Type: "question_closed", Timestamp: "2026-01-01T01:00:00Z"},
		{Type: "question_closed", Timestamp: "2026-01-01T01:01:00Z"},
		{Type: "question_closed", Timestamp: "2026-01-01T01:02:00Z"},
		{Type: "question_closed", Timestamp: "2026-01-01T01:03:00Z"},
	}
	mode := ClassifyMode(events)
	if mode.Mode != "convergent" {
		t.Errorf("expected 'convergent' for question_closed events, got %q", mode.Mode)
	}
}

func TestClassifyMode_IncubationLongGap(t *testing.T) {
	events := []Event{
		{Type: "finding_recorded", Timestamp: "2026-01-01T01:00:00Z"},
		{Type: "finding_recorded", Timestamp: "2026-01-01T05:00:00Z"}, // 4 hour gap
	}
	mode := ClassifyMode(events)
	if mode.Mode != "incubation" {
		t.Errorf("expected 'incubation' for >2h gap, got %q", mode.Mode)
	}
	if mode.DoFHint != "mid" {
		t.Errorf("expected DoF hint 'mid', got %q", mode.DoFHint)
	}
}

func TestClassifyMode_AbductiveFindings(t *testing.T) {
	// All finding_recorded, no questions, no challenges, no crystallization.
	// Both divergent and abductive score equally (finding_recorded/total = 1.0),
	// but divergent appears first in the candidates list, so it wins the tie.
	// The abductive candidate IS added, but does not beat divergent's score.
	events := []Event{
		{Type: "finding_recorded", Timestamp: "2026-01-01T01:00:00Z"},
		{Type: "finding_recorded", Timestamp: "2026-01-01T01:01:00Z"},
		{Type: "finding_recorded", Timestamp: "2026-01-01T01:02:00Z"},
	}
	mode := ClassifyMode(events)
	// divergent scores: (finding_recorded + question_opened) / total = 3/3 = 1.0
	// abductive scores: finding_recorded / total = 3/3 = 1.0
	// divergent appears first in candidates, wins tie via ">" comparison.
	if mode.Mode != "divergent" && mode.Mode != "abductive" {
		t.Errorf("expected 'divergent' or 'abductive' for only finding_recorded events, got %q", mode.Mode)
	}
}

func TestClassifyMode_WindowLimitedToLast5(t *testing.T) {
	// First 6 events are finding_recorded, last 5 are question_closed.
	// Should classify based on the last 5 only.
	events := make([]Event, 11)
	for i := 0; i < 6; i++ {
		events[i] = Event{Type: "finding_recorded", Timestamp: "2026-01-01T01:00:00Z"}
	}
	for i := 6; i < 11; i++ {
		events[i] = Event{Type: "question_closed", Timestamp: "2026-01-01T02:00:00Z"}
	}
	mode := ClassifyMode(events)
	if mode.Mode != "convergent" {
		t.Errorf("expected 'convergent' based on last 5 events, got %q", mode.Mode)
	}
}

// ---------------------------------------------------------------------------
// RecordEvent + LoadEvents round-trip
// ---------------------------------------------------------------------------

func TestRecordEvent_And_LoadEvents(t *testing.T) {
	dir := t.TempDir()

	// Freeze time for predictable timestamps.
	frozenTime := time.Date(2026, 1, 15, 10, 0, 0, 0, time.UTC)
	timeNow = func() time.Time { return frozenTime }
	defer func() { timeNow = time.Now }()

	data := map[string]interface{}{"key": "value"}
	err := RecordEvent(dir, "t-abc", "finding_recorded", data)
	if err != nil {
		t.Fatalf("RecordEvent failed: %v", err)
	}

	// Record a second event.
	err = RecordEvent(dir, "t-abc", "question_opened", nil)
	if err != nil {
		t.Fatalf("RecordEvent (second) failed: %v", err)
	}

	// Record an event for a different thread.
	err = RecordEvent(dir, "t-other", "session_started", nil)
	if err != nil {
		t.Fatalf("RecordEvent (other thread) failed: %v", err)
	}

	// Load all events (no filter).
	all, err := LoadEvents(dir, "")
	if err != nil {
		t.Fatalf("LoadEvents (all) failed: %v", err)
	}
	if len(all) != 3 {
		t.Fatalf("expected 3 events total, got %d", len(all))
	}

	// Load filtered by thread.
	threadEvents, err := LoadEvents(dir, "t-abc")
	if err != nil {
		t.Fatalf("LoadEvents (filtered) failed: %v", err)
	}
	if len(threadEvents) != 2 {
		t.Fatalf("expected 2 events for t-abc, got %d", len(threadEvents))
	}

	// Verify first event content.
	ev := threadEvents[0]
	if ev.Version != 1 {
		t.Errorf("expected version 1, got %d", ev.Version)
	}
	if ev.Type != "finding_recorded" {
		t.Errorf("expected type 'finding_recorded', got %q", ev.Type)
	}
	if ev.ThreadID != "t-abc" {
		t.Errorf("expected thread_id 't-abc', got %q", ev.ThreadID)
	}
	if ev.Sequence != 0 {
		t.Errorf("expected sequence 0 for first event, got %d", ev.Sequence)
	}

	// Verify second event sequence number.
	if threadEvents[1].Sequence != 1 {
		t.Errorf("expected sequence 1 for second event, got %d", threadEvents[1].Sequence)
	}
}

func TestLoadEvents_MissingFile(t *testing.T) {
	dir := t.TempDir()
	events, err := LoadEvents(dir, "")
	if err != nil {
		t.Fatalf("LoadEvents should return nil error for missing file, got: %v", err)
	}
	if events != nil {
		t.Errorf("expected nil events for missing file, got %d events", len(events))
	}
}

// ---------------------------------------------------------------------------
// SaveThread + LoadThreads round-trip
// ---------------------------------------------------------------------------

func TestSaveThread_And_LoadThreads(t *testing.T) {
	dir := t.TempDir()

	thread1 := Thread{
		ID:        "t-1",
		Status:    "active",
		Summary:   "auth flow",
		CreatedAt: "2026-01-15T10:00:00Z",
	}
	thread2 := Thread{
		ID:        "t-2",
		Status:    "active",
		Summary:   "data model",
		CreatedAt: "2026-01-15T11:00:00Z",
	}

	if err := SaveThread(dir, thread1); err != nil {
		t.Fatalf("SaveThread (1) failed: %v", err)
	}
	if err := SaveThread(dir, thread2); err != nil {
		t.Fatalf("SaveThread (2) failed: %v", err)
	}

	threads, err := LoadThreads(dir)
	if err != nil {
		t.Fatalf("LoadThreads failed: %v", err)
	}
	if len(threads) != 2 {
		t.Fatalf("expected 2 threads, got %d", len(threads))
	}

	if threads[0].ID != "t-1" || threads[0].Summary != "auth flow" {
		t.Errorf("thread 0 mismatch: got ID=%q Summary=%q", threads[0].ID, threads[0].Summary)
	}
	if threads[1].ID != "t-2" || threads[1].Summary != "data model" {
		t.Errorf("thread 1 mismatch: got ID=%q Summary=%q", threads[1].ID, threads[1].Summary)
	}
}

func TestLoadThreads_MissingFile(t *testing.T) {
	dir := t.TempDir()
	threads, err := LoadThreads(dir)
	if err != nil {
		t.Fatalf("LoadThreads should return nil error for missing file, got: %v", err)
	}
	if threads != nil {
		t.Errorf("expected nil threads for missing file, got %d threads", len(threads))
	}
}

func TestSaveThread_CreatesDirectory(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "nested", "events")
	thread := Thread{ID: "t-new", Status: "active", Summary: "test"}
	if err := SaveThread(dir, thread); err != nil {
		t.Fatalf("SaveThread should create directory, got: %v", err)
	}
	threads, err := LoadThreads(dir)
	if err != nil {
		t.Fatalf("LoadThreads failed: %v", err)
	}
	if len(threads) != 1 {
		t.Fatalf("expected 1 thread, got %d", len(threads))
	}
}

// ---------------------------------------------------------------------------
// ParkThread + MergeThread
// ---------------------------------------------------------------------------

func TestParkThread(t *testing.T) {
	dir := t.TempDir()

	// Save two threads.
	if err := SaveThread(dir, Thread{ID: "t-1", Status: "active", Summary: "thread one"}); err != nil {
		t.Fatal(err)
	}
	if err := SaveThread(dir, Thread{ID: "t-2", Status: "active", Summary: "thread two"}); err != nil {
		t.Fatal(err)
	}

	// Park the first thread.
	if err := ParkThread(dir, "t-1"); err != nil {
		t.Fatalf("ParkThread failed: %v", err)
	}

	threads, err := LoadThreads(dir)
	if err != nil {
		t.Fatalf("LoadThreads after park failed: %v", err)
	}

	var found bool
	for _, th := range threads {
		if th.ID == "t-1" {
			found = true
			if th.Status != "parked" {
				t.Errorf("expected t-1 status 'parked', got %q", th.Status)
			}
		}
	}
	if !found {
		t.Error("thread t-1 not found after parking")
	}
}

func TestParkThread_NotFound(t *testing.T) {
	dir := t.TempDir()
	if err := SaveThread(dir, Thread{ID: "t-1", Status: "active", Summary: "x"}); err != nil {
		t.Fatal(err)
	}
	err := ParkThread(dir, "t-nonexistent")
	if err == nil {
		t.Error("expected error for parking nonexistent thread")
	}
	if !strings.Contains(err.Error(), "not found") {
		t.Errorf("expected 'not found' in error, got: %v", err)
	}
}

func TestMergeThread(t *testing.T) {
	dir := t.TempDir()

	if err := SaveThread(dir, Thread{ID: "t-src", Status: "active", Summary: "source"}); err != nil {
		t.Fatal(err)
	}
	if err := SaveThread(dir, Thread{ID: "t-tgt", Status: "active", Summary: "target"}); err != nil {
		t.Fatal(err)
	}

	if err := MergeThread(dir, "t-src", "t-tgt"); err != nil {
		t.Fatalf("MergeThread failed: %v", err)
	}

	threads, err := LoadThreads(dir)
	if err != nil {
		t.Fatalf("LoadThreads after merge failed: %v", err)
	}

	for _, th := range threads {
		if th.ID == "t-src" && th.Status != "merged" {
			t.Errorf("expected t-src status 'merged', got %q", th.Status)
		}
		if th.ID == "t-tgt" && th.Status != "active" {
			t.Errorf("expected t-tgt status to remain 'active', got %q", th.Status)
		}
	}
}

func TestMergeThread_SourceNotFound(t *testing.T) {
	dir := t.TempDir()
	if err := SaveThread(dir, Thread{ID: "t-1", Status: "active", Summary: "x"}); err != nil {
		t.Fatal(err)
	}
	err := MergeThread(dir, "t-nonexistent", "t-1")
	if err == nil {
		t.Error("expected error when source thread not found")
	}
}

func TestMergeThread_TargetNotFound(t *testing.T) {
	dir := t.TempDir()
	if err := SaveThread(dir, Thread{ID: "t-1", Status: "active", Summary: "x"}); err != nil {
		t.Fatal(err)
	}
	err := MergeThread(dir, "t-1", "t-nonexistent")
	if err == nil {
		t.Error("expected error when target thread not found")
	}
}

// ---------------------------------------------------------------------------
// uniqueWords (helper)
// ---------------------------------------------------------------------------

func TestUniqueWords_Basic(t *testing.T) {
	words := uniqueWords("Hello World hello")
	if len(words) != 2 {
		t.Errorf("expected 2 unique words, got %d: %v", len(words), words)
	}
	if !words["hello"] {
		t.Error("expected 'hello' in unique words")
	}
	if !words["world"] {
		t.Error("expected 'world' in unique words")
	}
}

func TestUniqueWords_PunctuationStripped(t *testing.T) {
	words := uniqueWords("hello, world! (test)")
	if !words["hello"] {
		t.Error("expected 'hello' after comma stripping")
	}
	if !words["test"] {
		t.Error("expected 'test' after paren stripping")
	}
}

func TestUniqueWords_SingleCharFiltered(t *testing.T) {
	words := uniqueWords("a b cd ef")
	if words["a"] || words["b"] {
		t.Error("single-char words should be filtered")
	}
	if !words["cd"] || !words["ef"] {
		t.Error("two-char words should be included")
	}
}

func TestUniqueWords_EmptyInput(t *testing.T) {
	words := uniqueWords("")
	if len(words) != 0 {
		t.Errorf("expected empty set for empty input, got %d", len(words))
	}
}

// ---------------------------------------------------------------------------
// itoa (helper)
// ---------------------------------------------------------------------------

func TestItoa(t *testing.T) {
	tests := []struct {
		input    int
		expected string
	}{
		{0, "0"},
		{1, "1"},
		{42, "42"},
		{100, "100"},
	}
	for _, tc := range tests {
		got := itoa(tc.input)
		if got != tc.expected {
			t.Errorf("itoa(%d) = %q, want %q", tc.input, got, tc.expected)
		}
	}
}

// ---------------------------------------------------------------------------
// JSONL round-trip fidelity check
// ---------------------------------------------------------------------------

func TestRecordEvent_JSONLFormat(t *testing.T) {
	dir := t.TempDir()

	frozenTime := time.Date(2026, 3, 1, 12, 0, 0, 0, time.UTC)
	timeNow = func() time.Time { return frozenTime }
	defer func() { timeNow = time.Now }()

	data := map[string]interface{}{"finding": "important"}
	if err := RecordEvent(dir, "t-fmt", "finding_recorded", data); err != nil {
		t.Fatal(err)
	}

	// Read raw file content and verify it is valid JSONL.
	raw, err := os.ReadFile(filepath.Join(dir, "discovery.jsonl"))
	if err != nil {
		t.Fatalf("failed to read JSONL file: %v", err)
	}
	lines := strings.Split(strings.TrimSpace(string(raw)), "\n")
	if len(lines) != 1 {
		t.Fatalf("expected 1 JSONL line, got %d", len(lines))
	}

	var parsed Event
	if err := json.Unmarshal([]byte(lines[0]), &parsed); err != nil {
		t.Fatalf("invalid JSON in JSONL line: %v", err)
	}
	if parsed.ThreadID != "t-fmt" {
		t.Errorf("expected thread_id 't-fmt', got %q", parsed.ThreadID)
	}
	if parsed.Type != "finding_recorded" {
		t.Errorf("expected type 'finding_recorded', got %q", parsed.Type)
	}
}

// ---------------------------------------------------------------------------
// countOpenQuestions (helper)
// ---------------------------------------------------------------------------

func TestCountOpenQuestions(t *testing.T) {
	events := []Event{
		{Type: "question_opened"},
		{Type: "question_opened"},
		{Type: "question_closed"},
		{Type: "finding_recorded"},
		{Type: "question_opened"},
	}
	count := countOpenQuestions(events)
	if count != 2 {
		t.Errorf("expected 2 open questions, got %d", count)
	}
}

func TestCountOpenQuestions_NeverNegative(t *testing.T) {
	events := []Event{
		{Type: "question_closed"},
		{Type: "question_closed"},
	}
	count := countOpenQuestions(events)
	if count != 0 {
		t.Errorf("expected 0 open questions (clamped from negative), got %d", count)
	}
}

func TestCountOpenQuestions_Empty(t *testing.T) {
	count := countOpenQuestions(nil)
	if count != 0 {
		t.Errorf("expected 0 open questions for nil events, got %d", count)
	}
}

// ---------------------------------------------------------------------------
// findLimitingFactor (helper)
// ---------------------------------------------------------------------------

func TestFindLimitingFactor(t *testing.T) {
	// Coverage=5, Depth=3, Coherence=7, Completeness=8, Formality=4
	conf := [5]int{5, 3, 7, 8, 4}
	limiting := findLimitingFactor(conf)
	if limiting != "depth" {
		t.Errorf("expected 'depth' as limiting factor (score 3), got %q", limiting)
	}
}

func TestFindLimitingFactor_TieBreakByInitialIndex(t *testing.T) {
	// All scores at 5. findLimitingFactor starts with minIdx=0, minVal=conf[0].
	// DimensionPriority walk uses strict "<", so no index beats the initial.
	// Result: DimensionNames[0] = "coverage".
	conf := [5]int{5, 5, 5, 5, 5}
	limiting := findLimitingFactor(conf)
	if limiting != "coverage" {
		t.Errorf("expected 'coverage' for tie (initial index wins), got %q", limiting)
	}
}

// ---------------------------------------------------------------------------
// modeGuidance (helper)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// APP-INV-026: Classification Non-Prescriptive
// Mode classification is observational — it reports what mode is observed
// but does not prescribe or mandate behavior.
// ---------------------------------------------------------------------------

// ddis:tests APP-INV-026
func TestAPPINV026_ClassificationNonPrescriptive(t *testing.T) {
	dir := t.TempDir()

	// Create a thread so BuildContext has something to work with.
	if err := SaveThread(dir, Thread{ID: "t-1", Status: "active", Summary: "test thread"}); err != nil {
		t.Fatal(err)
	}

	result, err := BuildContext(nil, 0, DiscoverOptions{
		Content:   "test classification",
		EventsDir: dir,
	})
	if err != nil {
		t.Fatalf("BuildContext failed: %v", err)
	}

	// 1. The state snapshot uses "ObservedMode" (observational, not prescriptive).
	if result.State.ModeObserved == "" {
		t.Error("ModeObserved should be set (classification must produce a mode)")
	}

	// 2. Guidance uses "SuggestedNext" — not "RequiredNext" or "MandatoryActions".
	if result.Guidance.SuggestedNext == nil {
		t.Error("SuggestedNext should be populated with mode-appropriate suggestions")
	}

	// 3. The guidance field is named "ObservedMode" — explicitly observational.
	if result.Guidance.ObservedMode == "" {
		t.Error("Guidance.ObservedMode should be set")
	}

	// 4. The Output contains mode as observation ("**Mode**: ..."), not as command.
	if !strings.Contains(result.Output, "**Mode**:") {
		t.Error("output should contain mode observation (\"**Mode**: ...\")")
	}

	// 5. Test all 7 defined modes produce guidance without prescriptive language.
	prescriptiveVerbs := []string{"you must", "you are required", "mandatory:", "required:"}
	modes := []string{"divergent", "convergent", "dialectical", "abductive",
		"metacognitive", "incubation", "crystallization"}
	conf := [5]int{5, 5, 5, 5, 5}
	for _, m := range modes {
		mc := ModeClassification{Mode: m, Confidence: 0.8, DoFHint: "mid"}
		suggestions := modeGuidance(mc, conf)
		for _, s := range suggestions {
			lower := strings.ToLower(s)
			for _, verb := range prescriptiveVerbs {
				if strings.Contains(lower, verb) {
					t.Errorf("mode %q suggestion %q contains prescriptive verb %q", m, s, verb)
				}
			}
		}
	}
}

// ---------------------------------------------------------------------------
// APP-INV-030: Contributor Topology Graceful Degradation
// BuildContext must succeed even without a database, git, or prior state.
// ---------------------------------------------------------------------------

// ddis:tests APP-INV-030
func TestAPPINV030_ContributorTopologyGracefulDegradation(t *testing.T) {
	dir := t.TempDir()

	// Call BuildContext with nil DB and zero specID — no database, no git, no prior state.
	result, err := BuildContext(nil, 0, DiscoverOptions{
		Content:   "graceful degradation test",
		EventsDir: dir,
	})
	if err != nil {
		t.Fatalf("BuildContext(nil, 0, ...) should succeed without DB, got: %v", err)
	}

	// Must produce a non-nil, non-empty result.
	if result == nil {
		t.Fatal("result should not be nil even without DB")
	}
	if result.Output == "" {
		t.Error("output should not be empty even in degraded mode")
	}

	// Confidence should be zero-valued (no DB to compute from), not an error.
	zeroConf := [5]int{}
	if result.State.Confidence != zeroConf {
		t.Errorf("expected zero confidence without DB, got %v", result.State.Confidence)
	}

	// Drift should be 0 (no DB to measure drift from).
	if result.State.SpecDrift != 0 {
		t.Errorf("expected zero spec drift without DB, got %f", result.State.SpecDrift)
	}

	// Should still produce a valid thread (new thread created).
	if result.State.ActiveThread == "" {
		t.Error("should have an active thread even without DB")
	}

	// Mode classification should work (defaults to divergent for empty events).
	if result.State.ModeObserved == "" {
		t.Error("mode classification should work even without DB")
	}

	// Guidance should still be populated.
	if len(result.Guidance.SuggestedNext) == 0 {
		t.Error("guidance suggestions should be populated even in degraded mode")
	}
}

// ---------------------------------------------------------------------------
// APP-INV-036: Human Format Transparency
// BuildContext output is human-readable markdown. No internal DDIS format
// artifacts (raw invariant patterns, violation scenarios, parser syntax) leak
// into user-facing output.
// ---------------------------------------------------------------------------

// ddis:tests APP-INV-036
func TestAPPINV036_HumanFormatTransparency(t *testing.T) {
	dir := t.TempDir()

	// Set up threads and events to produce rich output.
	if err := SaveThread(dir, Thread{ID: "t-1", Status: "active", Summary: "design thread", EventCount: 3}); err != nil {
		t.Fatal(err)
	}
	if err := SaveThread(dir, Thread{ID: "t-2", Status: "parked", Summary: "parked thread"}); err != nil {
		t.Fatal(err)
	}

	result, err := BuildContext(nil, 0, DiscoverOptions{
		Content:   "design the authentication flow",
		EventsDir: dir,
	})
	if err != nil {
		t.Fatalf("BuildContext failed: %v", err)
	}

	output := result.Output

	// 1. Output should be non-empty readable text.
	if len(output) < 50 {
		t.Errorf("output suspiciously short (%d chars), expected readable markdown", len(output))
	}

	// 2. Output should contain human-readable markdown headers.
	if !strings.Contains(output, "## Discovery Context") {
		t.Error("output should contain readable section header")
	}

	// 3. No internal DDIS format artifacts should leak into output.
	internalPatterns := []struct {
		pattern string
		desc    string
	}{
		{"**INV-", "raw invariant declaration"},
		{"**APP-INV-", "raw app invariant declaration"},
		{"**ADR-", "raw ADR declaration"},
		{"**APP-ADR-", "raw app ADR declaration"},
		{"Violation scenario:", "invariant violation scenario field"},
		{"Semi-formal:", "invariant semi-formal field"},
		{"Validation method:", "invariant validation method field"},
		{"#### Problem", "ADR problem subheading"},
		{"#### Options", "ADR options subheading"},
		{"#### Decision", "ADR decision subheading"},
		{"#### Consequences", "ADR consequences subheading"},
		{"ddis:implements", "code annotation syntax"},
		{"ddis:maintains", "code annotation syntax"},
		{"ddis:tests", "code annotation syntax"},
		{"```yaml", "raw YAML code block"},
		{"spec_id:", "internal DB field"},
		{"content_hash:", "internal DB field"},
	}

	for _, p := range internalPatterns {
		if strings.Contains(output, p.pattern) {
			t.Errorf("output contains internal DDIS format artifact %q (%s)", p.pattern, p.desc)
		}
	}

	// 4. Output should use human-readable formatting (bold labels, not raw keys).
	humanLabels := []string{"**Thread**:", "**Mode**:", "**Open questions**:"}
	for _, label := range humanLabels {
		if !strings.Contains(output, label) {
			t.Errorf("output missing human-readable label %q", label)
		}
	}
}

func TestModeGuidance_ReturnsNonEmpty(t *testing.T) {
	modes := []string{"divergent", "convergent", "dialectical", "abductive",
		"metacognitive", "incubation", "crystallization", "unknown_mode"}
	conf := [5]int{5, 3, 7, 8, 4}
	for _, m := range modes {
		mc := ModeClassification{Mode: m}
		suggestions := modeGuidance(mc, conf)
		if len(suggestions) == 0 {
			t.Errorf("modeGuidance(%q) returned empty suggestions", m)
		}
	}
}
