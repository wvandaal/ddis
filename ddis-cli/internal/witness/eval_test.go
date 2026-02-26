package witness

import (
	"context"
	"fmt"
	"testing"
)

// mockProvider simulates an LLM provider for deterministic testing.
type mockProvider struct {
	available bool
	responses []string
	callIdx   int
}

func (m *mockProvider) Available() bool { return m.available }
func (m *mockProvider) ModelID() string { return "mock-model-v1" }
func (m *mockProvider) Complete(_ context.Context, _ string) (string, error) {
	if !m.available {
		return "", fmt.Errorf("provider not available")
	}
	if m.callIdx >= len(m.responses) {
		return "", fmt.Errorf("no more mock responses")
	}
	resp := m.responses[m.callIdx]
	m.callIdx++
	return resp, nil
}

func TestClassifyResponse(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"holds", "holds"},
		{"Holds", "holds"},
		{"HOLDS", "holds"},
		{"\"holds\"", "holds"},
		{"violated", "violated"},
		{"Violated.", "violated"},
		{"violates", "violated"},
		{"inconclusive", "inconclusive"},
		{"I think it holds because...", "inconclusive"},
		{"", "inconclusive"},
		{"hold", "holds"},
	}
	for _, tc := range tests {
		got := classifyResponse(tc.input)
		if got != tc.expected {
			t.Errorf("classifyResponse(%q) = %q, want %q", tc.input, got, tc.expected)
		}
	}
}

func TestMajorityVote_Unanimous(t *testing.T) {
	votes := map[string]int{"holds": 3}
	count, verdict := majorityVote(votes)
	if count != 3 || verdict != "holds" {
		t.Errorf("expected (3, holds), got (%d, %s)", count, verdict)
	}
}

func TestMajorityVote_TwoThirds(t *testing.T) {
	votes := map[string]int{"holds": 2, "violated": 1}
	count, verdict := majorityVote(votes)
	if count != 2 || verdict != "holds" {
		t.Errorf("expected (2, holds), got (%d, %s)", count, verdict)
	}
}

func TestMajorityVote_NoMajority(t *testing.T) {
	votes := map[string]int{"holds": 1, "violated": 1, "inconclusive": 1}
	count, _ := majorityVote(votes)
	if count != 1 {
		t.Errorf("expected count 1 for no majority, got %d", count)
	}
}

func TestTrimToFirstWord(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"holds", "holds"},
		{"  Holds  ", "holds"},
		{"\"HOLDS\"", "holds"},
		{"violated. The invariant...", "violated"},
		{"", ""},
	}
	for _, tc := range tests {
		got := trimToFirstWord(tc.input)
		if got != tc.expected {
			t.Errorf("trimToFirstWord(%q) = %q, want %q", tc.input, got, tc.expected)
		}
	}
}
