package consistency

// Tier 6: LLM-as-Judge semantic contradiction detection.
//
// ddis:implements APP-ADR-042 (Tier 6 LLM-as-judge semantic contradiction detection)
// ddis:implements APP-ADR-040 (LLM-as-judge semantic contradictions via Anthropic SDK)
// ddis:maintains APP-INV-054 (LLM provider graceful degradation — skips when unavailable)
// ddis:maintains APP-INV-055 (eval evidence statistical soundness — majority vote protocol)
//
// For each pair of invariants whose semi-formals failed Tiers 3-5 parsing,
// prompts the LLM to classify the relationship as compatible, contradictory,
// or independent. Uses majority vote (3 runs, 2/3 agreement) for precision.
// Graceful degradation when ANTHROPIC_API_KEY is absent.

import (
	"context"
	"database/sql"
	"fmt"
	"strings"

	"github.com/wvandaal/ddis/internal/llm"
	"github.com/wvandaal/ddis/internal/storage"
)

// LLMSemanticConflict is the conflict type for Tier 6 LLM-detected contradictions.
const LLMSemanticConflict ConflictType = "llm_semantic_conflict"

// LLMProvider is the provider used by Tier 6. Set via SetLLMProvider for testing.
var LLMProvider llm.Provider

// SetLLMProvider overrides the default LLM provider (for testing).
func SetLLMProvider(p llm.Provider) {
	LLMProvider = p
}

// LLMAvailable returns true if the LLM provider is configured and available.
func LLMAvailable() bool {
	p := getProvider()
	return p.Available()
}

func getProvider() llm.Provider {
	if LLMProvider != nil {
		return LLMProvider
	}
	return llm.NewProvider()
}

const pairwisePrompt = `You are a specification consistency checker. Given two invariants, determine whether they are semantically contradictory.

Invariant A: %s
Statement A: %s
Semi-formal A: %s

Invariant B: %s
Statement B: %s
Semi-formal B: %s

Respond with EXACTLY one word: "contradictory" if the two invariants cannot both hold simultaneously, "compatible" if they can coexist, or "independent" if they address unrelated concerns.`

const llmRunsRequired = 3

// analyzeLLM runs Tier 6 LLM-based pairwise semantic analysis.
// Only processes invariant pairs whose semi-formals could not be parsed by Tiers 3-5.
func analyzeLLM(db *sql.DB, specID int64) ([]Contradiction, int, error) {
	provider := getProvider()
	if !provider.Available() {
		return nil, 0, nil // Graceful degradation — skip silently.
	}

	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, 0, fmt.Errorf("list invariants: %w", err)
	}

	// Filter to invariants with semi-formals that Tiers 3-5 could NOT parse.
	type unparsed struct {
		inv *storage.Invariant
	}
	var candidates []unparsed
	for i := range invs {
		inv := &invs[i]
		if inv.SemiFormal == "" {
			continue
		}
		// If propositional (Tier 3) OR SMT (Tier 5) can parse it, skip.
		vm := NewVarMap()
		satCNF := ParseSemiFormal(inv.SemiFormal, vm)
		satOK := len(satCNF) > 0
		_, _, smtOK := TranslateSMTLIB2(inv.SemiFormal)
		if !satOK && !smtOK {
			candidates = append(candidates, unparsed{inv: inv})
		}
	}

	if len(candidates) < 2 {
		return nil, len(invs), nil
	}

	// Pairwise analysis with majority vote.
	ctx := context.Background()
	var results []Contradiction
	for i := 0; i < len(candidates); i++ {
		for j := i + 1; j < len(candidates); j++ {
			a, b := candidates[i].inv, candidates[j].inv
			verdict, confidence, err := classifyPair(ctx, provider, a, b)
			if err != nil {
				continue // LLM error — skip this pair
			}
			if verdict == "contradictory" {
				results = append(results, Contradiction{
					Tier:     TierLLM,
					Type:     LLMSemanticConflict,
					ElementA: a.InvariantID,
					ElementB: b.InvariantID,
					Description: fmt.Sprintf(
						"%s and %s are semantically contradictory (LLM-as-judge, %d/%d agreement).",
						a.InvariantID, b.InvariantID, llmRunsRequired, llmRunsRequired,
					),
					Evidence: fmt.Sprintf(
						"Statement A: %q. Statement B: %q. LLM verdict: contradictory.",
						truncate(a.Statement, 100), truncate(b.Statement, 100),
					),
					Confidence:     confidence,
					ResolutionHint: "LLM detected a semantic conflict. Review both invariant statements for implicit assumption clashes or scope overlaps.",
				})
			}
		}
	}

	return results, len(candidates), nil
}

// classifyPair runs majority-vote LLM evaluation on a pair of invariants.
// Returns (verdict, confidence, error).
func classifyPair(ctx context.Context, provider llm.Provider, a, b *storage.Invariant) (string, float64, error) {
	prompt := fmt.Sprintf(pairwisePrompt,
		a.InvariantID, a.Statement, a.SemiFormal,
		b.InvariantID, b.Statement, b.SemiFormal,
	)

	votes := make(map[string]int)
	for i := 0; i < llmRunsRequired; i++ {
		resp, err := provider.Complete(ctx, prompt)
		if err != nil {
			return "", 0, fmt.Errorf("LLM run %d: %w", i+1, err)
		}
		verdict := classifyLLMResponse(resp)
		votes[verdict]++
	}

	// Determine majority.
	bestVerdict := "independent"
	bestCount := 0
	for v, c := range votes {
		if c > bestCount {
			bestCount = c
			bestVerdict = v
		}
	}

	// ddis:maintains APP-INV-102 (centralized LLM confidence constants)
	if bestCount < 2 {
		return "independent", 0, nil // No majority
	}
	confidence := llm.ConfidenceMajority
	if bestCount >= 3 {
		confidence = llm.ConfidenceUnanimous
	}

	return bestVerdict, confidence, nil
}

// classifyLLMResponse normalizes an LLM response to one of: "contradictory", "compatible", "independent".
func classifyLLMResponse(resp string) string {
	resp = strings.ToLower(strings.TrimSpace(resp))
	// Take first word.
	if idx := strings.IndexAny(resp, " \n\t.,;"); idx >= 0 {
		resp = resp[:idx]
	}
	switch resp {
	case "contradictory", "contradiction":
		return "contradictory"
	case "compatible", "consistent":
		return "compatible"
	default:
		return "independent"
	}
}
