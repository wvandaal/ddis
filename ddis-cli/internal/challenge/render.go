package challenge

import (
	"encoding/json"
	"fmt"
	"strings"
)

// Render formats a single challenge result.
func Render(r *Result, asJSON bool) (string, error) {
	if asJSON {
		b, err := json.MarshalIndent(r, "", "  ")
		return string(b), err
	}
	return renderHuman(r), nil
}

// RenderAll formats multiple challenge results.
func RenderAll(results []Result, asJSON bool) (string, error) {
	if asJSON {
		b, err := json.MarshalIndent(results, "", "  ")
		return string(b), err
	}

	var lines []string
	confirmed, provisional, refuted, inconclusive := 0, 0, 0, 0
	for i := range results {
		lines = append(lines, renderHuman(&results[i]))
		switch results[i].Verdict {
		case Confirmed:
			confirmed++
		case Provisional:
			provisional++
		case Refuted:
			refuted++
		case Inconclusive:
			inconclusive++
		}
	}
	lines = append(lines, "")
	lines = append(lines, fmt.Sprintf("Summary: %d challenged — %d confirmed, %d provisional, %d refuted, %d inconclusive",
		len(results), confirmed, provisional, refuted, inconclusive))
	return strings.Join(lines, "\n"), nil
}

func renderHuman(r *Result) string {
	var b strings.Builder

	icon := "?"
	switch r.Verdict {
	case Confirmed:
		icon = "+"
	case Provisional:
		icon = "~"
	case Refuted:
		icon = "x"
	}

	fmt.Fprintf(&b, "Challenge: %s [%s %s]", r.InvariantID, icon, r.Verdict)

	if r.LevelFormal != nil {
		fmt.Fprintf(&b, "\n  L1 Formal:      parsed=%v consistent=%v", r.LevelFormal.Parsed, r.LevelFormal.SelfConsistent)
		if r.LevelFormal.Detail != "" {
			fmt.Fprintf(&b, " (%s)", r.LevelFormal.Detail)
		}
	}
	if r.LevelUncertainty != nil {
		fmt.Fprintf(&b, "\n  L2 Uncertainty: type=%s confidence=%.1f", r.LevelUncertainty.EvidenceType, r.LevelUncertainty.Confidence)
	}
	if r.LevelCausal != nil {
		fmt.Fprintf(&b, "\n  L3 Causal:      test_found=%v code_annotations=%d", r.LevelCausal.TestFound, r.LevelCausal.CodeAnnotations)
		if r.LevelCausal.TestName != "" {
			fmt.Fprintf(&b, " test=%s", r.LevelCausal.TestName)
		}
	}
	if r.LevelPractical != nil {
		fmt.Fprintf(&b, "\n  L4 Practical:   ran=%v passed=%v", r.LevelPractical.Ran, r.LevelPractical.Passed)
	}
	if r.LevelMeta != nil {
		fmt.Fprintf(&b, "\n  L5 Meta:        overlap=%.2f (inv=%d evid=%d shared=%d)", r.LevelMeta.Overlap, r.LevelMeta.InvTerms, r.LevelMeta.EvidTerms, r.LevelMeta.Shared)
	}
	if r.WitnessInvalidated {
		fmt.Fprintf(&b, "\n  Witness INVALIDATED (refuted)")
	}

	return b.String()
}
