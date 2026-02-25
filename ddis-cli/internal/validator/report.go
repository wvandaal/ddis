package validator

// ddis:implements APP-ADR-032 (Gestalt-optimized CLI output)
// ddis:maintains APP-INV-043 (invariant statement inline — CheckResult fields)
// ddis:maintains APP-INV-044 (warning collapse — top-5 rendering)

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

	// Partition results into failed and passed
	var failed, passed []CheckResult
	for _, res := range r.Results {
		if res.Passed {
			passed = append(passed, res)
		} else {
			failed = append(failed, res)
		}
	}

	// FAILURES first (the important part)
	if len(failed) > 0 {
		fmt.Fprintf(&b, "FAILURES (%d):\n", len(failed))
		for _, res := range failed {
			renderCheckGestalt(&b, res)
		}
		b.WriteString("\n")
	}

	// PASSED collapsed to one line
	if len(passed) > 0 {
		ids := make([]string, 0, len(passed))
		for _, res := range passed {
			ids = append(ids, fmt.Sprintf("%d", res.CheckID))
		}
		fmt.Fprintf(&b, "PASSED (%d): Checks %s\n", len(passed), strings.Join(ids, ", "))
	}

	b.WriteString("\n───────────────────────────────────────────\n")
	fmt.Fprintf(&b, "Total: %d checks, %d passed, %d failed (%d errors, %d warnings)\n",
		r.TotalChecks, r.Passed, r.Failed, r.Errors, r.Warnings)

	return b.String()
}

// renderCheckGestalt renders a single failing check with Gestalt-optimized output:
// spec-first framing (invariant statement), warning collapse (count + top-5).
func renderCheckGestalt(b *strings.Builder, res CheckResult) {
	// Header with invariant ID if available
	header := res.CheckName
	if res.InvariantID != "" {
		header = fmt.Sprintf("%s: %s", res.InvariantID, res.CheckName)
	}
	fmt.Fprintf(b, "  [FAIL] Check %d: %s\n", res.CheckID, header)

	// Inline invariant statement (spec-first framing)
	if res.InvariantStatement != "" {
		fmt.Fprintf(b, "         %q\n", res.InvariantStatement)
	}

	// Count findings by severity
	errorFindings := 0
	warningFindings := 0
	for _, f := range res.Findings {
		switch f.Severity {
		case SeverityError:
			errorFindings++
		case SeverityWarning:
			warningFindings++
		}
	}

	// Summary line
	if res.Summary != "" {
		fmt.Fprintf(b, "         %s\n", res.Summary)
	}

	// Show errors inline (usually few)
	if errorFindings > 0 {
		shown := 0
		for _, f := range res.Findings {
			if f.Severity != SeverityError {
				continue
			}
			loc := ""
			if f.Location != "" {
				loc = fmt.Sprintf(" (%s)", f.Location)
			}
			fmt.Fprintf(b, "    ERROR%s: %s\n", loc, f.Message)
			shown++
			if shown >= 10 {
				if errorFindings > 10 {
					fmt.Fprintf(b, "    ... and %d more errors\n", errorFindings-10)
				}
				break
			}
		}
	}

	// Collapse warnings: count + top-5
	if warningFindings > 0 {
		if warningFindings <= 5 {
			for _, f := range res.Findings {
				if f.Severity != SeverityWarning {
					continue
				}
				loc := ""
				if f.Location != "" {
					loc = fmt.Sprintf(" (%s)", f.Location)
				}
				fmt.Fprintf(b, "    WARN%s: %s\n", loc, f.Message)
			}
		} else {
			fmt.Fprintf(b, "    %d warnings (top 5):\n", warningFindings)
			shown := 0
			for _, f := range res.Findings {
				if f.Severity != SeverityWarning {
					continue
				}
				loc := ""
				if f.Location != "" {
					loc = fmt.Sprintf(" (%s)", f.Location)
				}
				fmt.Fprintf(b, "      %s: %s\n", loc, f.Message)
				shown++
				if shown >= 5 {
					break
				}
			}
		}
	}
}
