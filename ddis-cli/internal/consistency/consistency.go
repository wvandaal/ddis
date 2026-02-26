package consistency

// ddis:implements APP-ADR-038 (Z3 subprocess as Tier 5 — orchestrator)
// ddis:maintains APP-ADR-013 (superseded — planned divergence subsumed by tiered consistency)
// ddis:maintains APP-ADR-034 (superseded — gophersat retained for fast propositional path)
// ddis:maintains APP-INV-019 (contradiction graph soundness — zero false positives)
// ddis:maintains APP-INV-024 (ambiguity surfacing — contradiction detection surfaces conflicts, never auto-resolves)

import (
	"database/sql"
	"fmt"
	"sort"
)

// Tier identifies the detection technique that found a contradiction.
type Tier int

const (
	TierGraph     Tier = 2 // Graph-based: typed edges, governance overlap, cycles
	TierSAT       Tier = 3 // SAT-based: semi-formal → propositional encoding
	TierHeuristic Tier = 4 // Heuristic: polarity, quantifier, numeric rules + LSI
	TierSMT       Tier = 5 // SMT-based: semi-formal → SMT-LIB2 via Z3 subprocess
	TierLLM       Tier = 6 // LLM-as-judge: semantic contradiction via Anthropic API
)

func (t Tier) String() string {
	switch t {
	case TierGraph:
		return "graph"
	case TierSAT:
		return "SAT"
	case TierHeuristic:
		return "heuristic"
	case TierSMT:
		return "SMT"
	case TierLLM:
		return "LLM"
	default:
		return fmt.Sprintf("tier-%d", int(t))
	}
}

// ConflictType classifies the nature of the contradiction.
type ConflictType string

const (
	GovernanceOverlap    ConflictType = "governance_overlap"
	NegSpecViolation     ConflictType = "negative_spec_violation"
	PolarityInversion    ConflictType = "polarity_inversion"
	QuantifierConflict   ConflictType = "quantifier_conflict"
	NumericBoundConflict ConflictType = "numeric_bound_conflict"
	CircularImplication  ConflictType = "circular_implication"
	SATUnsatisfiable     ConflictType = "sat_unsatisfiable"
	SMTUnsatisfiable     ConflictType = "smt_unsatisfiable"
	SemanticTension      ConflictType = "semantic_tension"
)

// Contradiction represents a detected conflict between two spec elements.
type Contradiction struct {
	Tier           Tier         `json:"tier"`
	Type           ConflictType `json:"type"`
	ElementA       string       `json:"element_a"`
	ElementB       string       `json:"element_b"`
	Description    string       `json:"description"`
	Evidence       string       `json:"evidence"`
	Confidence     float64      `json:"confidence"`
	ResolutionHint string       `json:"resolution_hint"`
}

// Result holds the complete output of contradiction analysis.
type Result struct {
	Contradictions []Contradiction `json:"contradictions"`
	TiersRun       []Tier          `json:"tiers_run"`
	ElementsScanned int            `json:"elements_scanned"`
}

// Options controls which tiers to run.
type Options struct {
	MaxTier Tier // Run tiers up to this level (default: TierHeuristic = 4)
}

// Analyze runs tiered contradiction detection on a spec.
func Analyze(db *sql.DB, specID int64, opts Options) (*Result, error) {
	if opts.MaxTier == 0 {
		opts.MaxTier = TierHeuristic
	}

	result := &Result{}

	// Tier 2: Graph analysis
	if opts.MaxTier >= TierGraph {
		graphResults, scanned, err := analyzeGraph(db, specID)
		if err != nil {
			return nil, fmt.Errorf("tier 2 (graph): %w", err)
		}
		result.Contradictions = append(result.Contradictions, graphResults...)
		result.TiersRun = append(result.TiersRun, TierGraph)
		result.ElementsScanned += scanned
	}

	// Tier 3: SAT analysis
	if opts.MaxTier >= TierSAT {
		satResults, scanned, err := analyzeSAT(db, specID)
		if err != nil {
			return nil, fmt.Errorf("tier 3 (SAT): %w", err)
		}
		result.Contradictions = append(result.Contradictions, satResults...)
		result.TiersRun = append(result.TiersRun, TierSAT)
		result.ElementsScanned += scanned
	}

	// Tier 4: Heuristic + semantic
	if opts.MaxTier >= TierHeuristic {
		heuristicResults, scanned, err := analyzeHeuristic(db, specID)
		if err != nil {
			return nil, fmt.Errorf("tier 4 (heuristic): %w", err)
		}
		result.Contradictions = append(result.Contradictions, heuristicResults...)

		semanticResults, scanned2, err := analyzeSemantic(db, specID)
		if err != nil {
			return nil, fmt.Errorf("tier 4 (semantic): %w", err)
		}
		result.Contradictions = append(result.Contradictions, semanticResults...)
		result.TiersRun = append(result.TiersRun, TierHeuristic)
		result.ElementsScanned += scanned + scanned2
	}

	// Tier 5: SMT analysis (Z3 subprocess)
	if opts.MaxTier >= TierSMT {
		if Z3Available() {
			smtResults, scanned, err := analyzeSMT(db, specID)
			if err != nil {
				return nil, fmt.Errorf("tier 5 (SMT): %w", err)
			}
			result.Contradictions = append(result.Contradictions, smtResults...)
			result.TiersRun = append(result.TiersRun, TierSMT)
			result.ElementsScanned += scanned
		}
		// If Z3 not available, silently skip Tier 5
	}

	// Tier 6: LLM-as-judge semantic analysis
	if opts.MaxTier >= TierLLM {
		if LLMAvailable() {
			llmResults, scanned, err := analyzeLLM(db, specID)
			if err != nil {
				return nil, fmt.Errorf("tier 6 (LLM): %w", err)
			}
			result.Contradictions = append(result.Contradictions, llmResults...)
			result.TiersRun = append(result.TiersRun, TierLLM)
			result.ElementsScanned += scanned
		}
		// If LLM not available, silently skip Tier 6
	}

	// Deduplicate: same element pair may appear from multiple tiers.
	// Keep the highest-confidence finding per pair.
	result.Contradictions = dedup(result.Contradictions)

	// Sort by confidence descending.
	sort.Slice(result.Contradictions, func(i, j int) bool {
		return result.Contradictions[i].Confidence > result.Contradictions[j].Confidence
	})

	return result, nil
}

// dedup keeps the highest-confidence contradiction per element pair.
func dedup(cs []Contradiction) []Contradiction {
	type pairKey struct{ a, b string }
	best := make(map[pairKey]Contradiction)
	for _, c := range cs {
		k := pairKey{c.ElementA, c.ElementB}
		if c.ElementA > c.ElementB {
			k = pairKey{c.ElementB, c.ElementA}
		}
		if existing, ok := best[k]; !ok || c.Confidence > existing.Confidence {
			best[k] = c
		}
	}
	out := make([]Contradiction, 0, len(best))
	for _, c := range best {
		out = append(out, c)
	}
	return out
}
