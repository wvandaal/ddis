package annotate

import (
	"encoding/json"
	"fmt"
	"strings"
)

// RenderText renders the scan result as human-readable text.
func RenderText(result *ScanResult) string {
	var b strings.Builder

	b.WriteString("Scan Results\n")
	b.WriteString("═══════════════════════════════════════════════\n")
	fmt.Fprintf(&b, "  Files scanned:   %d\n", result.FilesScanned)
	fmt.Fprintf(&b, "  Files skipped:   %d\n", result.FilesSkipped)
	fmt.Fprintf(&b, "  Annotations:     %d\n", result.TotalFound)

	if len(result.ByVerb) > 0 {
		b.WriteString("\n  By verb:\n")
		for verb, count := range result.ByVerb {
			fmt.Fprintf(&b, "    %-16s %d\n", verb, count)
		}
	}

	if len(result.ByLanguage) > 0 {
		b.WriteString("\n  By language:\n")
		for lang, count := range result.ByLanguage {
			fmt.Fprintf(&b, "    %-16s %d\n", lang, count)
		}
	}

	if len(result.Annotations) > 0 {
		b.WriteString("\n  Annotations:\n")
		for _, a := range result.Annotations {
			qual := ""
			if a.Qualifier != "" {
				qual = fmt.Sprintf(" (%s)", a.Qualifier)
			}
			fmt.Fprintf(&b, "    %s:%d  %s %s%s\n", a.FilePath, a.Line, a.Verb, a.Target, qual)
		}
	}

	if result.VerifyReport != nil {
		b.WriteString("\nVerification Report\n")
		b.WriteString("───────────────────────────────────────────────\n")
		fmt.Fprintf(&b, "  Resolved:       %d annotations → spec elements\n", len(result.VerifyReport.Resolved))
		fmt.Fprintf(&b, "  Orphaned:       %d annotations → no spec element\n", len(result.VerifyReport.Orphaned))
		fmt.Fprintf(&b, "  Unimplemented:  %d spec elements → no annotations\n", len(result.VerifyReport.Unimplemented))

		if len(result.VerifyReport.Orphaned) > 0 {
			b.WriteString("\n  Orphaned annotations (target not in spec):\n")
			for _, a := range result.VerifyReport.Orphaned {
				fmt.Fprintf(&b, "    %s:%d  %s → %s\n", a.FilePath, a.Line, a.Verb, a.Target)
			}
		}

		if len(result.VerifyReport.Unimplemented) > 0 {
			b.WriteString("\n  Unimplemented spec elements (no code annotations):\n")
			for _, id := range result.VerifyReport.Unimplemented {
				fmt.Fprintf(&b, "    %s\n", id)
			}
		}
	}

	return b.String()
}

// RenderJSON renders the scan result as JSON.
func RenderJSON(result *ScanResult) (string, error) {
	data, err := json.MarshalIndent(result, "", "  ")
	if err != nil {
		return "", err
	}
	return string(data), nil
}
