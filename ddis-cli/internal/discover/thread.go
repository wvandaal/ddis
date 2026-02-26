package discover

// ddis:implements APP-ADR-019 (threads over sessions)
// ddis:implements APP-INV-027 (thread topology primacy — threads persist across sessions, LLMs, and humans)
// ddis:implements APP-INV-029 (convergent thread selection — ConvergeThread infers from content; user_override always available)

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// ConvergeThread selects or creates a thread based on content matching.
// Uses keyword overlap as a lightweight proxy for LSI/BM25 similarity.
// If threadOverride is non-empty, uses that thread directly.
// If no thread scores above 0.4, creates a new one.
func ConvergeThread(content string, threads []Thread, threadOverride string) ThreadMatchResult {
	// Direct override: user explicitly chose a thread.
	if threadOverride != "" {
		return ThreadMatchResult{
			ThreadID: threadOverride,
			Score:    1.0,
			Method:   "user_override",
		}
	}

	if len(threads) == 0 || content == "" {
		return newThreadResult()
	}

	contentWords := uniqueWords(content)
	if len(contentWords) == 0 {
		return newThreadResult()
	}

	var bestThread string
	var bestScore float64

	now := time.Now().UTC()

	for _, t := range threads {
		if t.Status != "active" {
			continue
		}
		summaryWords := uniqueWords(t.Summary)
		if len(summaryWords) == 0 {
			continue
		}

		// Count overlapping unique words.
		overlap := 0
		for w := range contentWords {
			if summaryWords[w] {
				overlap++
			}
		}

		denom := len(contentWords)
		if len(summaryWords) > denom {
			denom = len(summaryWords)
		}
		score := float64(overlap) / float64(denom)

		// Recency boost: +0.1 if last event within 24 hours.
		if t.LastEventAt != "" {
			if lastEvent, err := time.Parse(time.RFC3339, t.LastEventAt); err == nil {
				if now.Sub(lastEvent) < 24*time.Hour {
					score += 0.1
				}
			}
		}

		if score > bestScore {
			bestScore = score
			bestThread = t.ID
		}
	}

	if bestScore >= 0.4 {
		return ThreadMatchResult{
			ThreadID: bestThread,
			Score:    bestScore,
			Method:   "convergent",
		}
	}

	return newThreadResult()
}

func newThreadResult() ThreadMatchResult {
	return ThreadMatchResult{
		ThreadID: fmt.Sprintf("t-%d", time.Now().UnixMilli()),
		Score:    0.0,
		Method:   "new_thread",
	}
}

// uniqueWords splits text into lowercase unique words.
func uniqueWords(text string) map[string]bool {
	words := make(map[string]bool)
	for _, w := range strings.Fields(strings.ToLower(text)) {
		// Strip common punctuation for better matching.
		w = strings.Trim(w, ".,;:!?\"'()[]{}#*")
		if len(w) > 1 {
			words[w] = true
		}
	}
	return words
}

// LoadThreads reads all thread definitions from the threads JSONL file.
func LoadThreads(eventsDir string) ([]Thread, error) {
	path := filepath.Join(eventsDir, "threads.jsonl")
	f, err := os.Open(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("open threads file: %w", err)
	}
	defer f.Close()

	var threads []Thread
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" {
			continue
		}
		var t Thread
		if err := json.Unmarshal([]byte(line), &t); err != nil {
			return nil, fmt.Errorf("parse thread line: %w", err)
		}
		threads = append(threads, t)
	}
	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("read threads file: %w", err)
	}
	return threads, nil
}

// SaveThread appends a new or updated thread to the threads JSONL file.
func SaveThread(eventsDir string, thread Thread) error {
	if err := os.MkdirAll(eventsDir, 0755); err != nil {
		return fmt.Errorf("create events dir: %w", err)
	}

	path := filepath.Join(eventsDir, "threads.jsonl")
	f, err := os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
	if err != nil {
		return fmt.Errorf("open threads file for append: %w", err)
	}
	defer f.Close()

	data, err := json.Marshal(thread)
	if err != nil {
		return fmt.Errorf("marshal thread: %w", err)
	}
	if _, err := fmt.Fprintf(f, "%s\n", data); err != nil {
		return fmt.Errorf("write thread line: %w", err)
	}
	return nil
}

// ParkThread marks a thread as parked.
func ParkThread(eventsDir string, threadID string) error {
	return updateThreadStatus(eventsDir, threadID, "parked")
}

// MergeThread marks a source thread as merged into a target.
func MergeThread(eventsDir string, sourceID, targetID string) error {
	threads, err := LoadThreads(eventsDir)
	if err != nil {
		return err
	}

	found := false
	for i := range threads {
		if threads[i].ID == sourceID {
			threads[i].Status = "merged"
			found = true
			break
		}
	}
	if !found {
		return fmt.Errorf("thread %s not found", sourceID)
	}

	// Verify target exists.
	targetFound := false
	for _, t := range threads {
		if t.ID == targetID {
			targetFound = true
			break
		}
	}
	if !targetFound {
		return fmt.Errorf("target thread %s not found", targetID)
	}

	return rewriteThreads(eventsDir, threads)
}

// updateThreadStatus changes the status of a thread by ID.
func updateThreadStatus(eventsDir string, threadID, status string) error {
	threads, err := LoadThreads(eventsDir)
	if err != nil {
		return err
	}

	found := false
	for i := range threads {
		if threads[i].ID == threadID {
			threads[i].Status = status
			found = true
			break
		}
	}
	if !found {
		return fmt.Errorf("thread %s not found", threadID)
	}

	return rewriteThreads(eventsDir, threads)
}

// rewriteThreads overwrites the threads JSONL file with the given threads.
func rewriteThreads(eventsDir string, threads []Thread) error {
	path := filepath.Join(eventsDir, "threads.jsonl")
	f, err := os.Create(path)
	if err != nil {
		return fmt.Errorf("create threads file: %w", err)
	}
	defer f.Close()

	for _, t := range threads {
		data, err := json.Marshal(t)
		if err != nil {
			return fmt.Errorf("marshal thread: %w", err)
		}
		if _, err := fmt.Fprintf(f, "%s\n", data); err != nil {
			return fmt.Errorf("write thread line: %w", err)
		}
	}
	return nil
}
