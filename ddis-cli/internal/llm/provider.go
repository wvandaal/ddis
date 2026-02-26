package llm

// ddis:implements APP-INV-054 (LLM provider graceful degradation — Available() gates all LLM-dependent features)
// ddis:implements APP-ADR-040 (LLM-as-judge semantic contradictions via Anthropic SDK — Provider interface)

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"time"
)

// Provider abstracts LLM access with graceful degradation.
// When Available() returns false, all LLM-dependent features skip silently.
type Provider interface {
	// Available returns true when the provider is configured and reachable.
	Available() bool

	// Complete sends a prompt and returns the response text.
	Complete(ctx context.Context, prompt string) (string, error)

	// ModelID returns the model identifier for audit trails.
	ModelID() string
}

// AnthropicProvider implements Provider using the Anthropic Messages API.
type AnthropicProvider struct {
	apiKey  string
	model   string
	baseURL string
	client  *http.Client
}

// NewProvider creates a Provider using ANTHROPIC_API_KEY from the environment.
// Returns an AnthropicProvider that gracefully reports unavailable when the key is absent.
func NewProvider() Provider {
	return &AnthropicProvider{
		apiKey:  os.Getenv("ANTHROPIC_API_KEY"),
		model:   "claude-haiku-4-5-20251001",
		baseURL: "https://api.anthropic.com/v1/messages",
		client:  &http.Client{Timeout: 60 * time.Second},
	}
}

// Available returns true if ANTHROPIC_API_KEY is set and non-empty.
func (p *AnthropicProvider) Available() bool {
	return p.apiKey != ""
}

// ModelID returns the model used for API calls.
func (p *AnthropicProvider) ModelID() string {
	return p.model
}

// Complete sends a prompt to the Anthropic Messages API and returns the text response.
func (p *AnthropicProvider) Complete(ctx context.Context, prompt string) (string, error) {
	if !p.Available() {
		return "", fmt.Errorf("LLM provider not available (ANTHROPIC_API_KEY not set)")
	}

	reqBody := anthropicRequest{
		Model:     p.model,
		MaxTokens: 1024,
		Messages: []anthropicMessage{
			{Role: "user", Content: prompt},
		},
	}

	jsonBody, err := json.Marshal(reqBody)
	if err != nil {
		return "", fmt.Errorf("marshal request: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, "POST", p.baseURL, bytes.NewReader(jsonBody))
	if err != nil {
		return "", fmt.Errorf("create request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("x-api-key", p.apiKey)
	req.Header.Set("anthropic-version", "2023-06-01")

	resp, err := p.client.Do(req)
	if err != nil {
		return "", fmt.Errorf("API request: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return "", fmt.Errorf("read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("API error %d: %s", resp.StatusCode, truncateBytes(body, 200))
	}

	var result anthropicResponse
	if err := json.Unmarshal(body, &result); err != nil {
		return "", fmt.Errorf("parse response: %w", err)
	}

	if len(result.Content) == 0 {
		return "", fmt.Errorf("empty response from API")
	}

	return result.Content[0].Text, nil
}

// anthropicRequest is the Anthropic Messages API request body.
type anthropicRequest struct {
	Model     string              `json:"model"`
	MaxTokens int                 `json:"max_tokens"`
	Messages  []anthropicMessage  `json:"messages"`
}

// anthropicMessage is a single message in the conversation.
type anthropicMessage struct {
	Role    string `json:"role"`
	Content string `json:"content"`
}

// anthropicResponse is the Anthropic Messages API response body.
type anthropicResponse struct {
	Content []anthropicContent `json:"content"`
}

// anthropicContent is a content block in the response.
type anthropicContent struct {
	Type string `json:"type"`
	Text string `json:"text"`
}

func truncateBytes(b []byte, max int) string {
	if len(b) <= max {
		return string(b)
	}
	return string(b[:max]) + "..."
}
