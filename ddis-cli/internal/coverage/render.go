package coverage

import (
	"encoding/json"
	"fmt"
	"sort"
	"strings"
)

// Render produces either JSON or human-readable output for a CoverageResult.
func Render(result *CoverageResult, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(result, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal: %w", err)
		}
		return string(data) + "\n", nil
	}

	var b strings.Builder
	fmt.Fprintf(&b, "Coverage: %s — %.0f%% complete (%d/%d invariants, %d/%d ADRs)\n\n",
		result.Spec, result.Summary.Score*100,
		result.Summary.InvariantsComplete, result.Summary.InvariantsTotal,
		result.Summary.ADRsComplete, result.Summary.ADRsTotal)

	if len(result.Domains) > 0 {
		fmt.Fprintln(&b, "Domains:")
		domains := sortedDomainKeys(result.Domains)
		for _, domain := range domains {
			dc := result.Domains[domain]
			bar := progressBar(dc.Coverage, 20)
			fmt.Fprintf(&b, "  %-15s %d module(s)  %d invariants  %3.0f%%  %s\n",
				domain, dc.Modules, dc.Invariants, dc.Coverage*100, bar)
		}
		fmt.Fprintln(&b)
	}

	if len(result.Gaps) > 0 {
		fmt.Fprintf(&b, "Gaps (%d):\n", len(result.Gaps))
		for _, gap := range result.Gaps {
			fmt.Fprintf(&b, "  %s\n", gap)
		}
	}

	return b.String(), nil
}

func progressBar(ratio float64, width int) string {
	filled := int(ratio * float64(width))
	if filled > width {
		filled = width
	}
	if filled < 0 {
		filled = 0
	}
	return strings.Repeat("\u2588", filled) + strings.Repeat("\u2591", width-filled)
}

func sortedDomainKeys(m map[string]DomainCoverage) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}
