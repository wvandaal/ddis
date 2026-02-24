package progress

import (
	"encoding/json"
	"fmt"
	"strings"
)

// Render formats a ProgressResult for human-readable or JSON output.
func Render(result *ProgressResult, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(result, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal progress result: %w", err)
		}
		return string(data) + "\n", nil
	}
	return renderHuman(result), nil
}

func renderHuman(result *ProgressResult) string {
	var b strings.Builder

	fmt.Fprintf(&b, "Progress: %s\n", result.Progress)

	if result.NextRecommended != "" {
		fmt.Fprintf(&b, "Next recommended: %s\n", result.NextRecommended)
	}

	// Done section
	if len(result.Done) > 0 {
		fmt.Fprintf(&b, "\nDone (%d):\n", len(result.Done))
		for _, d := range result.Done {
			fmt.Fprintf(&b, "  %-15s %-15s %s\n", d.ID, d.Domain, d.Title)
		}
	}

	// Frontier section
	if len(result.Frontier) > 0 {
		fmt.Fprintf(&b, "\nFrontier (%d):\n", len(result.Frontier))
		for _, f := range result.Frontier {
			unblockStr := ""
			if f.Unblocks > 0 {
				unblockStr = fmt.Sprintf(" (unblocks %d)", f.Unblocks)
			}
			fmt.Fprintf(&b, "  %-15s %-15s %.2f  %s%s\n",
				f.ID, f.Domain, f.Authority, f.Title, unblockStr)
		}
	}

	// Blocked section
	if len(result.Blocked) > 0 {
		fmt.Fprintf(&b, "\nBlocked (%d):\n", len(result.Blocked))
		for _, bl := range result.Blocked {
			fmt.Fprintf(&b, "  %-15s %-15s %s\n", bl.ID, bl.Domain, bl.Title)
			fmt.Fprintf(&b, "                  waiting on: %s\n",
				strings.Join(bl.WaitingOn, ", "))
		}
	}

	return b.String()
}
