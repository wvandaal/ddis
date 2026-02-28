package witness

// ddis:implements APP-INV-055 (eval evidence statistical soundness — majority vote with 3 runs, 2/3 agreement)
// ddis:maintains APP-INV-054 (LLM provider graceful degradation — skips when provider unavailable)

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"

	"github.com/wvandaal/ddis/internal/llm"
	"github.com/wvandaal/ddis/internal/storage"
)

// EvalOptions controls the eval witness recording behavior.
type EvalOptions struct {
	InvariantID string
	ProvenBy    string
	CodeRoot    string
	Notes       string
	AsJSON      bool
	Provider    llm.Provider // Injected for testability
}

// EvalResult records the majority vote outcome for audit.
type EvalResult struct {
	InvariantID    string         `json:"invariant_id"`
	ModelID        string         `json:"model_id"`
	Runs           int            `json:"runs"`
	Agreement      int            `json:"agreement"`
	Confidence     float64        `json:"confidence"`
	PromptTemplate string         `json:"prompt_template"`
	VoteDistrib    map[string]int `json:"vote_distribution"`
	Responses      []string       `json:"raw_responses"`
	Verdict        string         `json:"verdict"` // "holds", "violated", "inconclusive"
}

const evalPromptTemplate = `You are a specification verification assistant. Given an invariant definition, determine whether the invariant holds based on its statement, semi-formal predicate, and violation scenario.

Invariant: %s
Statement: %s
Semi-formal: %s
Violation scenario: %s

Respond with EXACTLY one word: "holds" if the invariant is self-consistent and well-defined, "violated" if the invariant contains internal contradictions or is impossible to satisfy, or "inconclusive" if you cannot determine.`

const requiredRuns = 3

// RecordEval performs majority-vote LLM evaluation and records the witness.
// Requires 3 independent runs with 2/3 agreement for a valid result.
func RecordEval(db *sql.DB, specID int64, opts EvalOptions) (*EvalResult, error) {
	provider := opts.Provider
	if provider == nil {
		provider = llm.NewProvider()
	}

	if !provider.Available() {
		return nil, fmt.Errorf("eval witness requires an LLM provider (set ANTHROPIC_API_KEY)")
	}

	// Load invariant details for the prompt.
	inv, err := storage.GetInvariant(db, specID, opts.InvariantID)
	if err != nil {
		return nil, fmt.Errorf("invariant %s not found: %w", opts.InvariantID, err)
	}

	prompt := fmt.Sprintf(evalPromptTemplate,
		opts.InvariantID, inv.Statement, inv.SemiFormal, inv.ViolationScenario)

	// Run 3 independent evaluations.
	ctx := context.Background()
	votes := make(map[string]int)
	var responses []string

	for i := 0; i < requiredRuns; i++ {
		resp, err := provider.Complete(ctx, prompt)
		if err != nil {
			return nil, fmt.Errorf("LLM run %d failed: %w", i+1, err)
		}

		verdict := classifyResponse(resp)
		votes[verdict]++
		responses = append(responses, resp)
	}

	// Determine majority verdict.
	agreement, verdict := majorityVote(votes)

	// ddis:maintains APP-INV-102 (centralized LLM confidence constants)
	// Compute confidence per spec: unanimous or majority, reject otherwise.
	confidence := 0.0
	if agreement >= 3 {
		confidence = llm.ConfidenceUnanimous
	} else if agreement >= 2 {
		confidence = llm.ConfidenceMajority
	} else {
		// No majority — reject
		return &EvalResult{
			InvariantID:    opts.InvariantID,
			ModelID:        provider.ModelID(),
			Runs:           requiredRuns,
			Agreement:      agreement,
			Confidence:     0.0,
			PromptTemplate: evalPromptTemplate,
			VoteDistrib:    votes,
			Responses:      responses,
			Verdict:        "inconclusive",
		}, fmt.Errorf("no majority vote achieved (%v)", votes)
	}

	result := &EvalResult{
		InvariantID:    opts.InvariantID,
		ModelID:        provider.ModelID(),
		Runs:           requiredRuns,
		Agreement:      agreement,
		Confidence:     confidence,
		PromptTemplate: evalPromptTemplate,
		VoteDistrib:    votes,
		Responses:      responses,
		Verdict:        verdict,
	}

	// Record the witness with eval evidence type.
	evidenceJSON, _ := json.Marshal(result)
	w := &storage.InvariantWitness{
		SpecID:       specID,
		InvariantID:  opts.InvariantID,
		SpecHash:     inv.ContentHash,
		EvidenceType: "eval",
		Evidence:     string(evidenceJSON),
		ProvenBy:     opts.ProvenBy,
		Model:        provider.ModelID(),
		Status:       "valid",
		Notes:        opts.Notes,
	}

	if _, err := storage.InsertWitness(db, w); err != nil {
		return nil, fmt.Errorf("store eval witness: %w", err)
	}

	return result, nil
}

// classifyResponse normalizes an LLM response to one of: "holds", "violated", "inconclusive".
func classifyResponse(resp string) string {
	// Normalize: trim whitespace, lowercase, take first word.
	resp = trimToFirstWord(resp)
	switch resp {
	case "holds", "hold":
		return "holds"
	case "violated", "violates", "violation":
		return "violated"
	default:
		return "inconclusive"
	}
}

// majorityVote returns (agreement_count, winning_verdict) from the vote distribution.
func majorityVote(votes map[string]int) (int, string) {
	bestVerdict := "inconclusive"
	bestCount := 0
	for verdict, count := range votes {
		if count > bestCount {
			bestCount = count
			bestVerdict = verdict
		}
	}
	return bestCount, bestVerdict
}

// trimToFirstWord extracts and lowercases the first word from a response.
func trimToFirstWord(s string) string {
	// Strip quotes.
	s = stripPunctuation(s)
	// Trim whitespace.
	start := 0
	for start < len(s) && (s[start] == ' ' || s[start] == '\t' || s[start] == '\n') {
		start++
	}
	s = s[start:]
	end := len(s)
	for end > 0 && (s[end-1] == ' ' || s[end-1] == '\t' || s[end-1] == '\n') {
		end--
	}
	s = s[:end]
	// Take first word.
	for i, c := range s {
		if c == ' ' || c == '\n' || c == '\t' || c == '.' || c == ',' {
			s = s[:i]
			break
		}
	}
	return toLowerCase(s)
}

func stripPunctuation(s string) string {
	var result []byte
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c == '"' || c == '\'' || c == '`' {
			continue
		}
		result = append(result, c)
	}
	return string(result)
}

func toLowerCase(s string) string {
	b := make([]byte, len(s))
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c >= 'A' && c <= 'Z' {
			b[i] = c + 32
		} else {
			b[i] = c
		}
	}
	return string(b)
}
