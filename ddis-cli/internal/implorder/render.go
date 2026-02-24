package implorder

import (
	"encoding/json"
	"fmt"
	"strings"
)

// Render formats an ImplOrderResult for human-readable or JSON output.
func Render(result *ImplOrderResult, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(result, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal: %w", err)
		}
		return string(data) + "\n", nil
	}

	var b strings.Builder
	fmt.Fprintf(&b, "Implementation Order: %d elements, %d phases\n\n",
		result.TotalElements, result.CriticalPath)

	for _, phase := range result.Phases {
		fmt.Fprintf(&b, "Phase %d: %s\n", phase.PhaseNum, phase.Label)
		for _, elem := range phase.Elements {
			fmt.Fprintf(&b, "  %-15s %-15s %.2f  %s\n",
				elem.ID, elem.Domain, elem.Authority, elem.Title)
		}
		fmt.Fprintln(&b)
	}

	if len(result.CyclesDetected) > 0 {
		fmt.Fprintf(&b, "Cycles detected: %s\n",
			strings.Join(result.CyclesDetected, ", "))
	}
	return b.String(), nil
}
