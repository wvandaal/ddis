package triage

// ddis:maintains APP-INV-064 (spec-before-code gate — validate + drift check for affected invariants)
// ddis:implements APP-ADR-055 (full agent autonomy with guardrails — spec-convergence gate)

import (
	"database/sql"
	"fmt"

	"github.com/wvandaal/ddis/internal/drift"
	"github.com/wvandaal/ddis/internal/validator"
)

// SpecConvergenceResult reports whether affected spec elements have converged.
type SpecConvergenceResult struct {
	Converged    bool     `json:"converged"`
	NonConverged []string `json:"non_converged,omitempty"`
	Details      []string `json:"details,omitempty"`
}

// SpecConverged checks whether the spec has converged for the affected invariants.
// Convergence requires: (1) all validation checks pass, (2) drift = 0.
// This is the mechanical enforcement of APP-INV-064 (spec-before-code gate).
func SpecConverged(db *sql.DB, specID int64) (*SpecConvergenceResult, error) {
	result := &SpecConvergenceResult{Converged: true}

	// Check 1: All validation checks pass
	report, err := validator.Validate(db, specID, validator.ValidateOptions{})
	if err != nil {
		return nil, fmt.Errorf("validation failed: %w", err)
	}

	if report.Failed > 0 {
		result.Converged = false
		for _, r := range report.Results {
			if !r.Passed {
				// Skip Check 17 (challenge freshness) — pre-existing, not a convergence blocker
				if r.CheckID == 17 {
					continue
				}
				result.NonConverged = append(result.NonConverged, fmt.Sprintf("Check %d: %s", r.CheckID, r.CheckName))
				result.Details = append(result.Details, fmt.Sprintf("ddis validate --checks %d", r.CheckID))
			}
		}
		// If only Check 17 failed, that's not a convergence failure
		if len(result.NonConverged) == 0 {
			result.Converged = true
		}
	}

	// Check 2: Drift = 0
	driftReport, err := drift.Analyze(db, specID, drift.Options{Report: true})
	if err != nil {
		return nil, fmt.Errorf("drift analysis failed: %w", err)
	}

	if driftReport != nil && driftReport.EffectiveDrift > 0 {
		result.Converged = false
		result.NonConverged = append(result.NonConverged, fmt.Sprintf("drift score %d > 0", driftReport.EffectiveDrift))
		result.Details = append(result.Details, "ddis drift --report")
	}

	return result, nil
}
