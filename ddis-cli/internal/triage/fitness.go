package triage

// ddis:implements APP-ADR-056 (spec fitness function as endogenous quality signal)
// ddis:maintains APP-INV-069 (triage monotonic fitness — F(S) = weighted signal combination)

import (
	"math"
	"sort"
)

// Weights for the 6 fitness signals. Fixed constants — not configurable — to prevent gaming.
// Σwᵢ = 1.0. Causal ordering: foundation (0.20 each) > verification (0.15 each) > process (0.10).
const (
	WeightValidation    = 0.20
	WeightCoverage      = 0.20
	WeightDrift         = 0.20
	WeightChallengeHP   = 0.15
	WeightContradictions = 0.15
	WeightIssueBacklog  = 0.10
)

// ComputeFitness computes the Spec Fitness Function F(S) ∈ [0,1].
// F(S) = w₁·V(S) + w₂·C(S) + w₃·(1-D(S)) + w₄·H(S) + w₅·(1-K(S)) + w₆·(1-I(S))
func ComputeFitness(signals FitnessSignals) FitnessResult {
	score := WeightValidation*signals.Validation +
		WeightCoverage*signals.Coverage +
		WeightDrift*(1.0-signals.Drift) +
		WeightChallengeHP*signals.ChallengeHP +
		WeightContradictions*(1.0-signals.Contradictions) +
		WeightIssueBacklog*(1.0-signals.IssueBacklog)

	// Clamp to [0, 1]
	score = math.Max(0.0, math.Min(1.0, score))

	return FitnessResult{
		Score:   score,
		Signals: signals,
	}
}

// IsFixpointFitness returns true iff F(S) = 1.0, meaning all signals are perfect.
func IsFixpointFitness(signals FitnessSignals) bool {
	return signals.Validation == 1.0 &&
		signals.Coverage == 1.0 &&
		signals.Drift == 0.0 &&
		signals.ChallengeHP == 1.0 &&
		signals.Contradictions == 0.0 &&
		signals.IssueBacklog == 0.0
}

// RankDeficiencies identifies quality gaps and ranks them by estimated ΔF.
// For each signal where score < 1.0, it estimates the fitness improvement
// from addressing the deficiency. The list is sorted by ΔF descending.
func RankDeficiencies(signals FitnessSignals, dbPath string) []Deficiency {
	var defs []Deficiency

	if signals.Validation < 1.0 {
		gap := 1.0 - signals.Validation
		defs = append(defs, Deficiency{
			Category:    "validate",
			Description: "validation checks failing",
			Action:      "ddis validate " + dbPath,
			DeltaF:      gap * WeightValidation,
		})
	}

	if signals.Coverage < 1.0 {
		gap := 1.0 - signals.Coverage
		defs = append(defs, Deficiency{
			Category:    "coverage",
			Description: "coverage below 100%",
			Action:      "ddis coverage " + dbPath,
			DeltaF:      gap * WeightCoverage,
		})
	}

	if signals.Drift > 0 {
		defs = append(defs, Deficiency{
			Category:    "drift",
			Description: "spec-implementation drift detected",
			Action:      "ddis drift " + dbPath + " --report",
			DeltaF:      signals.Drift * WeightDrift,
		})
	}

	if signals.ChallengeHP < 1.0 {
		gap := 1.0 - signals.ChallengeHP
		defs = append(defs, Deficiency{
			Category:    "challenge",
			Description: "unchallenged or non-confirmed invariants",
			Action:      "ddis challenge --all " + dbPath + " --code-root .",
			DeltaF:      gap * WeightChallengeHP,
		})
	}

	if signals.Contradictions > 0 {
		defs = append(defs, Deficiency{
			Category:    "contradict",
			Description: "contradictions detected between invariants",
			Action:      "ddis contradict " + dbPath,
			DeltaF:      signals.Contradictions * WeightContradictions,
		})
	}

	if signals.IssueBacklog > 0 {
		defs = append(defs, Deficiency{
			Category:    "issue",
			Description: "open issues in backlog",
			Action:      "ddis issue status",
			DeltaF:      signals.IssueBacklog * WeightIssueBacklog,
		})
	}

	// Sort by ΔF descending (steepest descent direction)
	sort.Slice(defs, func(i, j int) bool {
		return defs[i].DeltaF > defs[j].DeltaF
	})

	return defs
}
