package refine

// ddis:implements APP-INV-024 (ambiguity surfacing — surfaceAmbiguities returns questions only, never resolutions)
// ddis:implements APP-ADR-017 (gestalt theory integration)

import (
	"database/sql"
	"fmt"
	"strconv"
	"strings"

	"github.com/wvandaal/ddis/internal/autoprompt"
	"github.com/wvandaal/ddis/internal/state"
	"github.com/wvandaal/ddis/internal/storage"
)

// selectFocusDimension finds the dimension with the lowest confidence score.
// Tie-break order: completeness > coherence > depth > coverage > formality.
func selectFocusDimension(conf [5]int) (string, int) {
	minVal := 11
	minIdx := 0
	for _, idx := range autoprompt.DimensionPriority {
		if conf[idx] < minVal {
			minVal = conf[idx]
			minIdx = idx
		}
	}
	return autoprompt.DimensionNames[minIdx], minVal
}

// ambiguity represents a potential spec ambiguity surfaced as a question.
type ambiguity struct {
	ElementID string
	Question  string
}

// surfaceAmbiguities scans for invariant pairs with overlapping statements
// and ADRs with unexplored alternatives. Returns questions, not resolutions.
func surfaceAmbiguities(db *sql.DB, specID int64) []ambiguity {
	var ambiguities []ambiguity

	// Check for invariants that might contradict: overlapping statements
	// with differing semi_formal predicates
	invs, err := storage.ListInvariants(db, specID)
	if err != nil || len(invs) < 2 {
		return ambiguities
	}

	// Simple overlap detection: invariants in the same section with opposing keywords
	for i := 0; i < len(invs)-1; i++ {
		for j := i + 1; j < len(invs); j++ {
			if invs[i].SectionID == invs[j].SectionID {
				stmtI := strings.ToLower(invs[i].Statement)
				stmtJ := strings.ToLower(invs[j].Statement)
				// Check for potential tension
				if (strings.Contains(stmtI, "must") && strings.Contains(stmtJ, "must not")) ||
					(strings.Contains(stmtI, "always") && strings.Contains(stmtJ, "never")) {
					ambiguities = append(ambiguities, ambiguity{
						ElementID: invs[i].InvariantID + " vs " + invs[j].InvariantID,
						Question:  fmt.Sprintf("Do %s and %s create tension? %q vs %q", invs[i].InvariantID, invs[j].InvariantID, invs[i].Title, invs[j].Title),
					})
				}
			}
			if len(ambiguities) >= 5 {
				break
			}
		}
		if len(ambiguities) >= 5 {
			break
		}
	}

	// Check for ADRs that may have unexplored alternatives
	adrs, err := storage.ListADRs(db, specID)
	if err != nil {
		return ambiguities
	}
	for _, adr := range adrs {
		if adr.Consequences == "" {
			ambiguities = append(ambiguities, ambiguity{
				ElementID: adr.ADRID,
				Question:  fmt.Sprintf("Has %s (%s) considered all consequences of the chosen approach?", adr.ADRID, adr.Title),
			})
		}
		if len(ambiguities) >= 10 {
			break
		}
	}

	return ambiguities
}

// Plan selects exactly ONE quality dimension to focus on.
// DoF separation: only one dimension improves per iteration.
func Plan(db *sql.DB, specID int64, iteration int, doSurfaceAmbiguity bool) (*autoprompt.CommandResult, error) {
	// 1. Get confidence from audit state or recompute
	var conf [5]int
	confStr, err := state.Get(db, specID, "refine_confidence_"+strconv.Itoa(iteration))
	if err == nil {
		parts := strings.Split(confStr, ",")
		if len(parts) == 5 {
			for i, p := range parts {
				v, _ := strconv.Atoi(strings.TrimSpace(p))
				conf[i] = v
			}
		}
	} else {
		// Recompute from scratch
		sid, _, sidErr := computeSpecInternalDrift(db, specID)
		if sidErr != nil {
			return nil, fmt.Errorf("compute spec drift: %w", sidErr)
		}
		conf = deriveConfidence(sid)
	}

	// 2. Select focus dimension
	dimension, score := selectFocusDimension(conf)

	// 3. Build rationale
	var output strings.Builder
	fmt.Fprintf(&output, "=== RALPH Plan ===\n\n")
	fmt.Fprintf(&output, "Focus dimension: %s (score: %d/10)\n\n", dimension, score)

	switch dimension {
	case "completeness":
		output.WriteString("Rationale: Elements have missing required components.\n")
		output.WriteString("Action: Fill in missing invariant/ADR fields for incomplete elements.\n")
	case "coherence":
		output.WriteString("Rationale: Cross-references are unresolved or inconsistent.\n")
		output.WriteString("Action: Resolve dangling references and ensure consistent naming.\n")
	case "depth":
		output.WriteString("Rationale: Invariants lack full component depth.\n")
		output.WriteString("Action: Expand shallow invariants with violation scenarios and validation methods.\n")
	case "coverage":
		output.WriteString("Rationale: Many elements are missing one or more quality components.\n")
		output.WriteString("Action: Systematically complete elements across all domains.\n")
	case "formality":
		output.WriteString("Rationale: Invariants lack semi-formal predicates.\n")
		output.WriteString("Action: Add semi-formal predicate expressions to invariants.\n")
	}

	// 4. Surface ambiguities if requested (APP-INV-024: questions, never resolutions)
	if doSurfaceAmbiguity {
		ambiguities := surfaceAmbiguities(db, specID)
		if len(ambiguities) > 0 {
			output.WriteString("\nAmbiguities to consider (questions only, per APP-INV-024):\n")
			for _, a := range ambiguities {
				fmt.Fprintf(&output, "  ? [%s] %s\n", a.ElementID, a.Question)
			}
		} else {
			output.WriteString("\nNo ambiguities detected.\n")
		}
	}

	// 5. Store plan state
	_ = state.Set(db, specID, "refine_focus_"+strconv.Itoa(iteration), dimension)

	// 6. Build CommandResult
	attenuation := autoprompt.Attenuation(iteration)

	return &autoprompt.CommandResult{
		Output: output.String(),
		State: autoprompt.StateSnapshot{
			ActiveThread:   "refine",
			Confidence:     conf,
			LimitingFactor: dimension,
			Iteration:      iteration,
			ModeObserved:   "convergent",
		},
		Guidance: autoprompt.Guidance{
			ObservedMode:    "convergent",
			DoFHint:         "mid",
			SuggestedNext:   []string{"Run 'ddis refine apply' to generate improvement prompt"},
			TranslationHint: fmt.Sprintf("Plan selected %s as focus for iteration %d", dimension, iteration),
			Attenuation:     attenuation,
		},
	}, nil
}
