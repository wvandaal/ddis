package workspace

import (
	"encoding/json"
	"fmt"
	"strings"
)

// RenderText returns a human-readable summary of init results.
func RenderText(r *InitResult) string {
	var b strings.Builder
	fmt.Fprintf(&b, "Initialized DDIS workspace at %s\n", r.Root)
	for _, f := range r.Created {
		fmt.Fprintf(&b, "  + %s\n", f)
	}
	for _, f := range r.Skipped {
		fmt.Fprintf(&b, "  = %s (already exists)\n", f)
	}
	return b.String()
}

// RenderJSON returns JSON representation of init results.
func RenderJSON(r *InitResult) (string, error) {
	data, err := json.MarshalIndent(r, "", "  ")
	if err != nil {
		return "", fmt.Errorf("marshal init result: %w", err)
	}
	return string(data), nil
}
