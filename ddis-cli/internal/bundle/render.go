package bundle

import (
	"encoding/json"
	"fmt"
	"strings"
)

// Render formats a BundleResult as human-readable text, JSON, or raw content.
func Render(result *BundleResult, asJSON bool, contentOnly bool) (string, error) {
	if contentOnly {
		return result.Content, nil
	}

	if asJSON {
		data, err := json.MarshalIndent(result, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal: %w", err)
		}
		return string(data) + "\n", nil
	}

	var b strings.Builder
	fmt.Fprintf(&b, "Domain Bundle: %s\n", result.Domain)
	fmt.Fprintf(&b, "  Constitution:       %d lines\n", result.ConstitutionLines)
	fmt.Fprintf(&b, "  Modules (%d):        %d lines\n", len(result.Modules), result.ModuleLines)
	for _, m := range result.Modules {
		fmt.Fprintf(&b, "    %s\n", m.Name)
	}
	fmt.Fprintf(&b, "  Interface stubs (%d): %d lines\n", len(result.InterfaceElements), result.InterfaceLines)
	for _, ie := range result.InterfaceElements {
		fmt.Fprintf(&b, "    %s (%s) — %s\n", ie.ID, ie.OwnerDomain, ie.Title)
	}
	fmt.Fprintln(&b, "  ─────────────────────────────────")
	fmt.Fprintf(&b, "  Total:             %d lines (%.0f%% of %d ceiling)\n\n",
		result.TotalLines, result.Budget.Usage*100, result.Budget.Ceiling)

	fmt.Fprintln(&b, result.Content)
	return b.String(), nil
}
