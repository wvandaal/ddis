package drift

// Classify determines the direction, severity, and intentionality of drift.
func Classify(report *DriftReport) Classification {
	c := Classification{
		Direction:      classifyDirection(report),
		Severity:       classifySeverity(report),
		Intentionality: classifyIntentionality(report),
	}
	return c
}

func classifyDirection(report *DriftReport) string {
	hasUnspecified := report.ImplDrift.Unspecified > 0
	hasUnimplemented := report.ImplDrift.Unimplemented > 0
	hasContradictions := report.ImplDrift.Contradictions > 0

	if hasContradictions {
		return "contradictory"
	}
	if hasUnspecified && !hasUnimplemented {
		return "impl-ahead"
	}
	if hasUnimplemented && !hasUnspecified {
		return "spec-ahead"
	}
	if hasUnspecified && hasUnimplemented {
		return "mutual"
	}
	return "aligned"
}

func classifySeverity(report *DriftReport) string {
	if report.ImplDrift.Contradictions > 0 {
		return "contradictory"
	}
	// Structural: high coherence drift relative to total
	if report.QualityBreakdown.Coherence > 5 &&
		report.QualityBreakdown.Coherence > report.ImplDrift.Total {
		return "structural"
	}
	return "additive"
}

func classifyIntentionality(report *DriftReport) string {
	if report.PlannedDivergences > 0 {
		return "planned"
	}
	return "organic"
}
