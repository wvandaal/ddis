package triage

// ddis:implements APP-ADR-054 (fixpoint convergence via well-founded ordering — measure computation)
// ddis:maintains APP-INV-068 (fixpoint termination — μ(S) = (open, unspecified, drift) ∈ ℕ³)

import (
	"github.com/wvandaal/ddis/internal/events"
)

// ComputeMeasure computes the triage measure μ(S) from the event stream and spec state.
// The three components map to distinct quality dimensions:
//   - OpenIssues: issue resolution
//   - Unspecified: spec completeness
//   - DriftScore: implementation alignment
func ComputeMeasure(evts []events.Event, unspecified int, driftScore int) Measure {
	issues := DeriveAllIssueStates(evts)
	open := 0
	for _, info := range issues {
		if !info.State.IsTerminal() {
			open++
		}
	}
	return Measure{
		OpenIssues:  open,
		Unspecified: unspecified,
		DriftScore:  driftScore,
	}
}

// MeasureDelta returns (after - before) for display purposes.
// Negative values mean improvement.
func MeasureDelta(before, after Measure) Measure {
	return Measure{
		OpenIssues:  after.OpenIssues - before.OpenIssues,
		Unspecified: after.Unspecified - before.Unspecified,
		DriftScore:  after.DriftScore - before.DriftScore,
	}
}
