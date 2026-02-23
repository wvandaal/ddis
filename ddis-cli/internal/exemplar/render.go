package exemplar

import (
	"encoding/json"
	"fmt"
	"strings"
)

// Render produces the output string for an ExemplarResult.
// If asJSON is true, returns compact JSON. Otherwise, human-readable text.
func Render(result *ExemplarResult, asJSON bool) (string, error) {
	if asJSON {
		return renderJSON(result)
	}
	return renderHuman(result), nil
}

func renderJSON(result *ExemplarResult) (string, error) {
	data, err := json.MarshalIndent(result, "", "  ")
	if err != nil {
		return "", fmt.Errorf("marshal exemplar result: %w", err)
	}
	return string(data) + "\n", nil
}

func renderHuman(result *ExemplarResult) string {
	var b strings.Builder

	// Header
	fmt.Fprintf(&b, "Exemplar Analysis: %s", result.Target)
	if result.Title != "" {
		fmt.Fprintf(&b, " - %s", result.Title)
	}
	b.WriteString("\n")
	fmt.Fprintf(&b, "Type: %s\n", result.ElementType)

	// Gaps section
	if len(result.Gaps) == 0 {
		b.WriteString("\nNo gaps found.\n")
	} else {
		b.WriteString("\nGaps Found:\n")
		for _, gap := range result.Gaps {
			severity := strings.ToUpper(gap.Severity)
			fmt.Fprintf(&b, "  %-22s %-8s (score: %.2f)\n", gap.Component, severity, gap.WeakScore)
		}
	}

	// Exemplars section
	if len(result.Exemplars) > 0 {
		b.WriteString("\nBest Exemplars:\n")
		for i, ex := range result.Exemplars {
			fmt.Fprintf(&b, "\n  #%d  %s - %s  (score: %.2f)\n", i+1, ex.ElementID, ex.Title, ex.QualityScore)
			fmt.Fprintf(&b, "      Component: %s\n", ex.DemonstratedComponent)
			fmt.Fprintf(&b, "      Signals:   completeness=%.2f  substance=%.2f  authority=%.2f  similarity=%.2f\n",
				ex.Signals.Completeness, ex.Signals.Substance, ex.Signals.Authority, ex.Signals.Similarity)

			if ex.Content != "" {
				b.WriteString("\n      Content:\n")
				wrapped := wrapText(ex.Content, 64)
				for _, line := range strings.Split(wrapped, "\n") {
					fmt.Fprintf(&b, "        %s\n", line)
				}
			}

			if ex.SubstrateCue != "" {
				b.WriteString("\n")
				wrapped := wrapText(ex.SubstrateCue, 64)
				for _, line := range strings.Split(wrapped, "\n") {
					fmt.Fprintf(&b, "      %s\n", line)
				}
			}
		}
	}

	// Guidance
	if result.Guidance != "" {
		b.WriteString("\nGuidance:\n")
		wrapped := wrapText(result.Guidance, 68)
		for _, line := range strings.Split(wrapped, "\n") {
			fmt.Fprintf(&b, "  %s\n", line)
		}
	}

	return b.String()
}

// wrapText wraps text at the given width on word boundaries.
func wrapText(text string, width int) string {
	if width <= 0 {
		return text
	}
	words := strings.Fields(text)
	if len(words) == 0 {
		return ""
	}

	var lines []string
	currentLine := words[0]

	for _, word := range words[1:] {
		if len(currentLine)+1+len(word) > width {
			lines = append(lines, currentLine)
			currentLine = word
		} else {
			currentLine += " " + word
		}
	}
	lines = append(lines, currentLine)

	return strings.Join(lines, "\n")
}
