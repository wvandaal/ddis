package discover

// ddis:maintains APP-INV-026 (classification non-prescriptive)
// ddis:implements APP-ADR-018 (observation over prescription)

import "time"

// ClassifyMode observes the cognitive mode from recent events.
// Classification is observational only -- it never prescribes (APP-INV-026).
func ClassifyMode(events []Event) ModeClassification {
	if len(events) == 0 {
		return ModeClassification{
			Mode:       "divergent",
			Confidence: 0.0,
			Evidence:   "no events to classify",
			DoFHint:    "very_high",
		}
	}

	// Look at the last 5 events.
	window := events
	if len(window) > 5 {
		window = window[len(window)-5:]
	}

	// Count event types in the window.
	typeCounts := make(map[string]int)
	for _, e := range window {
		typeCounts[e.Type]++
	}
	total := len(window)

	// Check for incubation: long time gap between last two events.
	if len(window) >= 2 {
		last := window[len(window)-1]
		prev := window[len(window)-2]
		if lastT, err1 := time.Parse(time.RFC3339, last.Timestamp); err1 == nil {
			if prevT, err2 := time.Parse(time.RFC3339, prev.Timestamp); err2 == nil {
				if lastT.Sub(prevT) > 2*time.Hour {
					return ModeClassification{
						Mode:       "incubation",
						Confidence: 0.7,
						Evidence:   "long time gap between events (>2h)",
						DoFHint:    "mid",
					}
				}
			}
		}
	}

	// Score each mode based on event type patterns.
	type modeScore struct {
		mode    string
		score   float64
		dofHint string
	}

	candidates := []modeScore{
		{
			mode:    "crystallization",
			score:   float64(typeCounts["decision_crystallized"]) / float64(total),
			dofHint: "very_low",
		},
		{
			mode:    "convergent",
			score:   float64(typeCounts["question_closed"]+typeCounts["decision_crystallized"]) / float64(total),
			dofHint: "low",
		},
		{
			mode:    "dialectical",
			score:   float64(typeCounts["challenge_posed"]+typeCounts["finding_recorded"]) / float64(total),
			dofHint: "high",
		},
		{
			mode:    "metacognitive",
			score:   float64(typeCounts["session_started"]+typeCounts["mode_observed"]) / float64(total),
			dofHint: "high",
		},
		{
			mode:    "divergent",
			score:   float64(typeCounts["finding_recorded"]+typeCounts["question_opened"]) / float64(total),
			dofHint: "very_high",
		},
	}

	// Abductive: finding_recorded dominant but with novel data patterns.
	// Approximate by finding_recorded being the sole dominant type.
	if typeCounts["finding_recorded"] > 0 && typeCounts["question_opened"] == 0 &&
		typeCounts["challenge_posed"] == 0 && typeCounts["decision_crystallized"] == 0 {
		candidates = append(candidates, modeScore{
			mode:    "abductive",
			score:   float64(typeCounts["finding_recorded"]) / float64(total),
			dofHint: "very_high",
		})
	}

	// Crystallization takes priority when decision_crystallized is dominant.
	// Check crystallization first.
	if typeCounts["decision_crystallized"] > 0 &&
		float64(typeCounts["decision_crystallized"])/float64(total) >= 0.5 {
		return ModeClassification{
			Mode:       "crystallization",
			Confidence: float64(typeCounts["decision_crystallized"]) / float64(total),
			Evidence:   "decision_crystallized events dominant",
			DoFHint:    "very_low",
		}
	}

	// Pick best scoring mode.
	best := candidates[0]
	for _, c := range candidates[1:] {
		if c.score > best.score {
			best = c
		}
	}

	// For dialectical, require both types present.
	if best.mode == "dialectical" && (typeCounts["challenge_posed"] == 0 || typeCounts["finding_recorded"] == 0) {
		// Fall back to divergent if only findings without challenges.
		if typeCounts["finding_recorded"] > 0 {
			best = modeScore{
				mode:    "divergent",
				score:   float64(typeCounts["finding_recorded"]+typeCounts["question_opened"]) / float64(total),
				dofHint: "very_high",
			}
		}
	}

	evidence := "event type distribution in last " + itoa(total) + " events"
	return ModeClassification{
		Mode:       best.mode,
		Confidence: best.score,
		Evidence:   evidence,
		DoFHint:    best.dofHint,
	}
}

// itoa is a minimal int-to-string helper to avoid importing strconv.
func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	if n < 0 {
		return "-" + itoa(-n)
	}
	digits := ""
	for n > 0 {
		digits = string(rune('0'+n%10)) + digits
		n /= 10
	}
	return digits
}
