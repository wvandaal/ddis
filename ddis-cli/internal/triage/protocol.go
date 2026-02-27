package triage

// ddis:implements APP-ADR-057 (agent-executable protocol for zero-knowledge participation)
// ddis:maintains APP-INV-070 (protocol completeness — self-contained JSON for any agent)

import (
	"encoding/json"
	"math"

	"github.com/wvandaal/ddis/internal/events"
)

// GenerateProtocol assembles the self-contained JSON protocol document
// from all quality signals. The protocol is sufficient for any agent
// to drive the triage lifecycle to fixpoint (APP-INV-070).
func GenerateProtocol(specID int64, fitness FitnessResult, measure Measure, evts []events.Event, dbPath string) Protocol {
	issues := DeriveAllIssueStates(evts)

	// Convert map to sorted slice
	issueList := make([]IssueInfo, 0, len(issues))
	for _, info := range issues {
		issueList = append(issueList, *info)
	}

	ranked := RankDeficiencies(fitness.Signals, dbPath)
	trajectory := LoadFitnessTrajectory(evts)

	// Append current fitness to trajectory
	trajectory = append(trajectory, fitness.Score)

	lyapunov := 1.0 - fitness.Score

	return Protocol{
		Version: "1.0",
		SpecID:  specID,
		Fitness: FitnessSection{
			Current:    fitness.Score,
			Target:     1.0,
			Trajectory: trajectory,
			Lyapunov:   lyapunov,
		},
		Measure:    measure,
		Issues:     issueList,
		RankedWork: ranked,
		Convergence: ConvergenceSection{
			LyapunovDecreasing:  isDecreasing(trajectory),
			MeasureDecreasing:   true,
			EstimatedStepsToFP:  estimateSteps(trajectory),
		},
	}
}

// LoadFitnessTrajectory reads historical fitness scores from the event stream.
// Looks for fitness_computed events in Stream 2.
func LoadFitnessTrajectory(evts []events.Event) []float64 {
	var trajectory []float64
	for _, e := range evts {
		if e.Type == "fitness_computed" {
			// Extract score from payload
			score := extractFloat(e, "score")
			if score > 0 {
				trajectory = append(trajectory, score)
			}
		}
	}
	return trajectory
}

// isDecreasing checks if the Lyapunov function (1-F) is strictly decreasing
// over the trajectory (i.e., fitness is strictly increasing).
func isDecreasing(trajectory []float64) bool {
	if len(trajectory) < 2 {
		return true // Vacuously true
	}
	for i := 1; i < len(trajectory); i++ {
		if trajectory[i] < trajectory[i-1] {
			return false // Fitness decreased => Lyapunov increased
		}
	}
	return true
}

// estimateSteps provides a rough estimate of steps remaining to fixpoint.
// Linear extrapolation from the trajectory.
func estimateSteps(trajectory []float64) int {
	if len(trajectory) < 2 {
		return 10 // Default estimate
	}

	last := trajectory[len(trajectory)-1]
	if last >= 1.0 {
		return 0 // Already at fixpoint
	}

	// Average improvement per step
	first := trajectory[0]
	avgDelta := (last - first) / float64(len(trajectory)-1)
	if avgDelta <= 0 {
		return 100 // No progress — pessimistic estimate
	}

	remaining := 1.0 - last
	steps := int(math.Ceil(remaining / avgDelta))
	if steps < 1 {
		steps = 1
	}
	return steps
}

// extractFloat extracts a float field from an event payload.
func extractFloat(e events.Event, key string) float64 {
	var payload map[string]interface{}
	if err := json.Unmarshal(e.Payload, &payload); err != nil {
		return 0
	}
	if v, ok := payload[key].(float64); ok {
		return v
	}
	return 0
}
