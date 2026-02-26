package consistency

import (
	"context"
	"fmt"
	"testing"
)

// mockLLMProvider simulates an LLM for deterministic testing.
type mockLLMProvider struct {
	available bool
	responses []string
	callIdx   int
}

func (m *mockLLMProvider) Available() bool { return m.available }
func (m *mockLLMProvider) ModelID() string { return "mock-llm-v1" }
func (m *mockLLMProvider) Complete(_ context.Context, _ string) (string, error) {
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

func TestClassifyLLMResponse(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"contradictory", "contradictory"},
		{"Contradictory", "contradictory"},
		{"contradiction", "contradictory"},
		{"compatible", "compatible"},
		{"Compatible.", "compatible"},
		{"consistent", "compatible"},
		{"independent", "independent"},
		{"", "independent"},
		{"I think they are compatible because...", "independent"}, // Multi-word → first word = "i"
	}
	for _, tc := range tests {
		got := classifyLLMResponse(tc.input)
		if got != tc.expected {
			t.Errorf("classifyLLMResponse(%q) = %q, want %q", tc.input, got, tc.expected)
		}
	}
}

func TestLLMAvailable_Default(t *testing.T) {
	// Default provider reads ANTHROPIC_API_KEY which is likely empty in test.
	// LLMAvailable should return false without error.
	old := LLMProvider
	LLMProvider = nil
	defer func() { LLMProvider = old }()

	// Just verify it doesn't panic.
	_ = LLMAvailable()
}

func TestLLMAvailable_Mock(t *testing.T) {
	old := LLMProvider
	defer func() { LLMProvider = old }()

	SetLLMProvider(&mockLLMProvider{available: true})
	if !LLMAvailable() {
		t.Error("expected LLMAvailable() = true with mock provider")
	}

	SetLLMProvider(&mockLLMProvider{available: false})
	if LLMAvailable() {
		t.Error("expected LLMAvailable() = false with unavailable mock")
	}
}

func TestTierLLM_String(t *testing.T) {
	if TierLLM.String() != "LLM" {
		t.Errorf("TierLLM.String() = %q, want %q", TierLLM.String(), "LLM")
	}
}

func TestLLMSemanticConflict_Type(t *testing.T) {
	if LLMSemanticConflict != "llm_semantic_conflict" {
		t.Errorf("expected LLMSemanticConflict = %q, got %q", "llm_semantic_conflict", LLMSemanticConflict)
	}
}
