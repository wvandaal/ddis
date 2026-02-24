package cascade

import (
	"encoding/json"
	"fmt"
	"strings"
)

// Render formats a CascadeResult for human-readable or JSON output.
func Render(result *CascadeResult, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(result, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal: %w", err)
		}
		return string(data) + "\n", nil
	}

	var b strings.Builder
	fmt.Fprintf(&b, "Cascade Analysis: %s — %s\n", result.ChangedElement, result.Title)
	if result.OwnerModule != "" {
		fmt.Fprintf(&b, "Owner: %s (%s)\n", result.OwnerModule, result.OwnerDomain)
	}
	fmt.Fprintln(&b)

	if len(result.AffectedModules) > 0 {
		fmt.Fprintln(&b, "Affected Modules:")
		for _, m := range result.AffectedModules {
			fmt.Fprintf(&b, "  %-25s %-15s %s\n", m.Module, m.Domain, m.Relationship)
		}
		fmt.Fprintln(&b)
	}

	if len(result.AffectedDomains) > 0 {
		fmt.Fprintf(&b, "Affected Domains: %s (%d domains)\n\n",
			strings.Join(result.AffectedDomains, ", "), len(result.AffectedDomains))
	}

	fmt.Fprintln(&b, result.Summary)
	return b.String(), nil
}
