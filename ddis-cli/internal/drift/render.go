package drift

import (
	"encoding/json"
	"fmt"
	"strings"
)

// Render produces output from a DriftReport.
func Render(report *DriftReport, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(report, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal drift report: %w", err)
		}
		return string(data) + "\n", nil
	}
	return renderHuman(report), nil
}

func renderHuman(r *DriftReport) string {
	var b strings.Builder

	b.WriteString("Drift Report\n")
	b.WriteString(strings.Repeat("═", 47) + "\n")

	b.WriteString(fmt.Sprintf("  Implementation drift:  %d", r.ImplDrift.Total))
	if r.ImplDrift.Total > 0 {
		b.WriteString(fmt.Sprintf(" (%d unspecified, %d unimplemented, %d contradictions)",
			r.ImplDrift.Unspecified, r.ImplDrift.Unimplemented, r.ImplDrift.Contradictions))
	}
	b.WriteString("\n")

	b.WriteString(fmt.Sprintf("  Intent drift:           %d", r.IntentDrift.Total))
	if r.IntentDrift.Total > 0 {
		b.WriteString(fmt.Sprintf(" (%d uncovered non-negotiable(s))",
			r.IntentDrift.UncoveredNonnegotiables))
	}
	b.WriteString("\n")

	b.WriteString(fmt.Sprintf("  Planned divergences:    %d\n", r.PlannedDivergences))
	b.WriteString("  " + strings.Repeat("─", 45) + "\n")
	b.WriteString(fmt.Sprintf("  Effective drift:       %d\n\n", r.EffectiveDrift))

	// Quality breakdown
	b.WriteString("  Quality breakdown:\n")
	b.WriteString(fmt.Sprintf("    Correctness:  %d", r.QualityBreakdown.Correctness))
	if r.QualityBreakdown.Correctness == 0 {
		b.WriteString("   (nothing violates the spec)")
	}
	b.WriteString("\n")
	b.WriteString(fmt.Sprintf("    Depth:       %d", r.QualityBreakdown.Depth))
	if r.QualityBreakdown.Depth > 0 {
		b.WriteString("   (code outpaced the spec — formalize these)")
	}
	b.WriteString("\n")
	b.WriteString(fmt.Sprintf("    Coherence:    %d", r.QualityBreakdown.Coherence))
	if r.QualityBreakdown.Coherence > 0 {
		b.WriteString(fmt.Sprintf("   (%d cross-ref gap(s))", r.QualityBreakdown.Coherence))
	}
	b.WriteString("\n\n")

	// Classification
	b.WriteString(fmt.Sprintf("  Direction:     %-14s Severity: %-14s Intentionality: %s\n\n",
		r.Classification.Direction, r.Classification.Severity, r.Classification.Intentionality))

	// Top drift details (limit to 10)
	if len(r.ImplDrift.Details) > 0 {
		limit := 10
		if len(r.ImplDrift.Details) < limit {
			limit = len(r.ImplDrift.Details)
		}

		// Group by type
		byType := make(map[string][]string)
		for _, d := range r.ImplDrift.Details[:limit] {
			byType[d.Type] = append(byType[d.Type], d.Element)
		}

		for _, typ := range []string{"unimplemented", "unspecified", "coherence"} {
			if items, ok := byType[typ]; ok {
				b.WriteString(fmt.Sprintf("  Top %s: %s", typ, strings.Join(items, ", ")))
				if len(r.ImplDrift.Details) > limit {
					b.WriteString("...")
				}
				b.WriteString("\n")
			}
		}
	}

	// Recommendation
	b.WriteString(renderRecommendation(r))

	return b.String()
}

func renderRecommendation(r *DriftReport) string {
	if r.EffectiveDrift == 0 {
		return "  Recommendation: Spec and implementation are aligned.\n"
	}

	q := r.QualityBreakdown
	var rec string
	if q.Correctness > q.Depth && q.Correctness > q.Coherence {
		rec = "  Recommendation: Correctness drift dominant — fix implementation to match spec.\n"
	} else if q.Coherence > q.Depth {
		rec = "  Recommendation: Coherence drift dominant — repair cross-references with `ddis validate`.\n"
	} else {
		rec = "  Recommendation: Depth drift dominant — run `ddis progress` to plan spec formalization.\n"
	}

	// Workflow hint: guide the agent to the next step
	rec += "\n  Workflow: ddis drift → apply guidance → ddis parse && ddis drift (drift must not increase)\n"
	return rec
}

// RenderRemediation produces output from a RemediationPackage.
func RenderRemediation(pkg *RemediationPackage, asJSON bool) (string, error) {
	if pkg == nil {
		if asJSON {
			return "{\"status\": \"aligned\", \"message\": \"No drift detected.\"}\n", nil
		}
		return "No drift detected. Spec and implementation are aligned.\n", nil
	}

	if asJSON {
		data, err := json.MarshalIndent(pkg, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal remediation: %w", err)
		}
		return string(data) + "\n", nil
	}
	return renderRemediationHuman(pkg), nil
}

func renderRemediationHuman(pkg *RemediationPackage) string {
	var b strings.Builder

	b.WriteString(fmt.Sprintf("Next: Formalize %q (%s, priority %d/%d)\n",
		pkg.Target, pkg.DriftType, pkg.Priority, pkg.TotalDrift))
	b.WriteString(strings.Repeat("═", 60) + "\n\n")

	if pkg.Title != "" {
		b.WriteString(fmt.Sprintf("  Title: %s\n\n", pkg.Title))
	}

	// Guidance
	if len(pkg.Guidance) > 0 {
		b.WriteString("Guidance:\n")
		for i, g := range pkg.Guidance {
			b.WriteString(fmt.Sprintf("  %d. %s\n", i+1, g))
		}
		b.WriteString("\n")
	}

	b.WriteString(fmt.Sprintf("  After writing: ddis parse && ddis drift (expect drift: %d → %d)\n",
		pkg.TotalDrift, pkg.ExpectedDrift))

	return b.String()
}
