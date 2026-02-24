package checklist

import (
	"encoding/json"
	"fmt"
	"strings"
)

// Render produces the output string for a ChecklistResult.
func Render(result *ChecklistResult, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(result, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal checklist: %w", err)
		}
		return string(data) + "\n", nil
	}
	return renderHuman(result), nil
}

func renderHuman(result *ChecklistResult) string {
	var b strings.Builder

	title := result.Spec
	if title == "" {
		title = "(unnamed spec)"
	}
	fmt.Fprintf(&b, "Verification Checklist: %s\n\n", title)

	for _, sec := range result.Sections {
		fmt.Fprintf(&b, "%s %s\n", sec.Section, sec.Title)
		for _, item := range sec.Items {
			fmt.Fprintf(&b, "  \u25a1 %s: %s\n", item.InvariantID, item.Title)
			for _, step := range item.Checklist {
				fmt.Fprintf(&b, "    - %s\n", step)
			}
		}
		fmt.Fprintln(&b)
	}

	fmt.Fprintf(&b, "Total: %d invariants, %d verification items\n",
		result.TotalInvariants, result.TotalItems)
	return b.String()
}
