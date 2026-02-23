package validator

import (
	"encoding/json"
	"fmt"
	"strings"
)

// RenderReport formats a validation report for output.
func RenderReport(report *Report, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(report, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal report: %w", err)
		}
		return string(data), nil
	}

	return renderHumanReport(report), nil
}

func renderHumanReport(r *Report) string {
	var b strings.Builder

	fmt.Fprintf(&b, "DDIS Validation Report: %s (%s)\n", r.SpecPath, r.SourceType)
	b.WriteString("═══════════════════════════════════════════\n\n")

	for _, res := range r.Results {
		status := "PASS"
		if !res.Passed {
			status = "FAIL"
		}
		fmt.Fprintf(&b, "[%s] Check %d: %s\n", status, res.CheckID, res.CheckName)
		if res.Summary != "" {
			fmt.Fprintf(&b, "       %s\n", res.Summary)
		}

		for _, f := range res.Findings {
			prefix := "  "
			switch f.Severity {
			case SeverityError:
				prefix = "  ERROR"
			case SeverityWarning:
				prefix = "  WARN "
			case SeverityInfo:
				prefix = "  INFO "
			}

			loc := ""
			if f.Location != "" {
				loc = fmt.Sprintf(" (%s)", f.Location)
			}
			fmt.Fprintf(&b, "%s%s: %s\n", prefix, loc, f.Message)
		}
		b.WriteString("\n")
	}

	b.WriteString("───────────────────────────────────────────\n")
	fmt.Fprintf(&b, "Total: %d checks, %d passed, %d failed (%d errors, %d warnings)\n",
		r.TotalChecks, r.Passed, r.Failed, r.Errors, r.Warnings)

	return b.String()
}
