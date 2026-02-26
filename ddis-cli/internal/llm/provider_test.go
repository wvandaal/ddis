package llm

import (
	"context"
	"os"
	"testing"
)

func TestAvailable_WithKey(t *testing.T) {
	p := &AnthropicProvider{apiKey: "sk-test-123"}
	if !p.Available() {
		t.Error("expected Available() = true when API key is set")
	}
}

func TestAvailable_WithoutKey(t *testing.T) {
	p := &AnthropicProvider{apiKey: ""}
	if p.Available() {
		t.Error("expected Available() = false when API key is empty")
	}
}

func TestNewProvider_ReadsEnv(t *testing.T) {
	// Save and restore env.
	orig := os.Getenv("ANTHROPIC_API_KEY")
	defer os.Setenv("ANTHROPIC_API_KEY", orig)

	os.Setenv("ANTHROPIC_API_KEY", "sk-test-from-env")
	p := NewProvider()
	if !p.Available() {
		t.Error("expected Available() = true with env var set")
	}
	if p.ModelID() == "" {
		t.Error("ModelID should return a non-empty string")
	}

	os.Setenv("ANTHROPIC_API_KEY", "")
	p2 := NewProvider()
	if p2.Available() {
		t.Error("expected Available() = false with empty env var")
	}
}

func TestComplete_WithoutKey(t *testing.T) {
	p := &AnthropicProvider{apiKey: ""}
	_, err := p.Complete(context.Background(), "hello")
	if err == nil {
		t.Error("expected error when calling Complete without API key")
	}
}

func TestModelID(t *testing.T) {
	p := NewProvider()
	if p.ModelID() == "" {
		t.Error("ModelID must return a non-empty string")
	}
}
