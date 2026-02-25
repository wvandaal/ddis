package refine

// ddis:maintains APP-INV-022 (refinement drift monotonicity)

import (
	"database/sql"
	"fmt"
	"strconv"
	"strings"

	"github.com/wvandaal/ddis/internal/autoprompt"
	"github.com/wvandaal/ddis/internal/drift"
	"github.com/wvandaal/ddis/internal/state"
)

// Judge compares before/after drift to enforce monotonicity.
// If drift increased, it halts the loop and reports regression.
func Judge(db *sql.DB, specID int64, iteration int) (*autoprompt.CommandResult, error) {
	// 1. Get previous drift from state table
	var prevDrift int
	hasPrev := false
	if iteration > 0 {
		prevStr, err := state.Get(db, specID, "refine_drift_"+strconv.Itoa(iteration-1))
		if err == nil {
			v, parseErr := strconv.Atoi(prevStr)
			if parseErr == nil {
				prevDrift = v
				hasPrev = true
			}
		}
	}

	// 2. Compute current drift
	driftReport, err := drift.Analyze(db, specID, drift.Options{})
	if err != nil {
		return nil, fmt.Errorf("drift analysis: %w", err)
	}
	currDrift := driftReport.EffectiveDrift

	// 3. Compute spec-internal metrics for confidence snapshot
	sid, _, sidErr := computeSpecInternalDrift(db, specID)
	var conf [5]int
	limiting := "unknown"
	if sidErr == nil {
		conf = deriveConfidence(sid)
		limiting = selectLimitingFactor(conf)
	}

	// 4. Compare and generate output
	var output strings.Builder
	output.WriteString("=== RALPH Judge ===\n\n")
	fmt.Fprintf(&output, "Iteration: %d\n", iteration)
	fmt.Fprintf(&output, "Current drift: %d\n", currDrift)

	var suggestedNext []string
	dofHint := "low"
	mode := "convergent"

	if hasPrev {
		delta := currDrift - prevDrift
		fmt.Fprintf(&output, "Previous drift: %d\n", prevDrift)
		fmt.Fprintf(&output, "Delta: %d\n\n", delta)

		if delta > 0 {
			// REGRESSION
			output.WriteString("REGRESSION DETECTED: drift increased.\n")
			output.WriteString("The most recent edit introduced quality regressions.\n")
			fmt.Fprintf(&output, "Quality breakdown: correctness=%d, depth=%d, coherence=%d\n",
				driftReport.QualityBreakdown.Correctness,
				driftReport.QualityBreakdown.Depth,
				driftReport.QualityBreakdown.Coherence)
			output.WriteString("\nRecommendation: review and revert the last edit, then re-run 'ddis refine apply'.\n")

			suggestedNext = []string{
				"Review the last edit for regressions",
				"Revert the problematic change",
				fmt.Sprintf("Re-run 'ddis refine apply' for iteration %d", iteration),
			}
			dofHint = "high" // regression requires more freedom to fix
			mode = "dialectical"
			limiting = fmt.Sprintf("drift regression in iteration %d", iteration)
		} else if delta < 0 {
			// IMPROVEMENT
			fmt.Fprintf(&output, "Quality improved: drift %d -> %d (delta: %d)\n", prevDrift, currDrift, delta)
			if currDrift == 0 {
				output.WriteString("\nDrift is zero. Spec is fully converged.\n")
				suggestedNext = []string{"Spec has converged. No further refinement needed."}
			} else {
				output.WriteString("\nProgress is positive. Continue to next iteration.\n")
				suggestedNext = []string{
					fmt.Sprintf("Run 'ddis refine audit --iteration %d' for next cycle", iteration+1),
				}
			}
		} else {
			// NO CHANGE
			output.WriteString("No drift change. The edit was neutral.\n")
			output.WriteString("Consider targeting a different dimension or element.\n")
			suggestedNext = []string{
				fmt.Sprintf("Run 'ddis refine audit --iteration %d' with a different focus", iteration+1),
			}
		}
	} else {
		// No previous drift available (first iteration or missing state)
		output.WriteString("No previous drift baseline. Recording current drift as baseline.\n")
		fmt.Fprintf(&output, "Quality breakdown: correctness=%d, depth=%d, coherence=%d\n",
			driftReport.QualityBreakdown.Correctness,
			driftReport.QualityBreakdown.Depth,
			driftReport.QualityBreakdown.Coherence)
		suggestedNext = []string{
			"Baseline recorded. Apply an edit and re-run judge to compare.",
		}
	}

	// 5. Store current drift in state
	_ = state.Set(db, specID, "refine_drift_"+strconv.Itoa(iteration), strconv.Itoa(currDrift))

	// 6. Build CommandResult
	attenuation := autoprompt.Attenuation(iteration)

	return &autoprompt.CommandResult{
		Output: output.String(),
		State: autoprompt.StateSnapshot{
			ActiveThread:   "refine",
			Confidence:     conf,
			LimitingFactor: limiting,
			SpecDrift:      float64(currDrift),
			Iteration:      iteration,
			ModeObserved:   mode,
		},
		Guidance: autoprompt.Guidance{
			ObservedMode:  mode,
			DoFHint:       dofHint,
			SuggestedNext: suggestedNext,
			Attenuation:   attenuation,
		},
	}, nil
}
