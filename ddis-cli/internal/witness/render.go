package witness

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// Render formats a WitnessSummary for output.
func Render(summary *WitnessSummary, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(summary, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal witness summary: %w", err)
		}
		return string(data) + "\n", nil
	}
	return renderHumanSummary(summary), nil
}

// RenderSingle formats a single witness for output.
func RenderSingle(w *storage.InvariantWitness, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(w, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal witness: %w", err)
		}
		return string(data) + "\n", nil
	}
	return renderHumanSingle(w), nil
}

// RenderList formats a list of witnesses for output.
func RenderList(witnesses []storage.InvariantWitness, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(witnesses, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal witnesses: %w", err)
		}
		return string(data) + "\n", nil
	}
	return renderHumanList(witnesses), nil
}

func renderHumanSummary(s *WitnessSummary) string {
	var b strings.Builder

	fmt.Fprintf(&b, "Witness Status: %d/%d valid (%s coverage)\n", s.Valid, s.Total, s.Coverage)
	b.WriteString(strings.Repeat("═", 60) + "\n")

	if s.Stale > 0 {
		fmt.Fprintf(&b, "  Stale: %d\n", s.Stale)
	}
	if s.Missing > 0 {
		fmt.Fprintf(&b, "  Missing: %d\n", s.Missing)
	}
	b.WriteString("\n")

	for _, item := range s.Items {
		mark := " "
		switch item.Status {
		case "valid":
			mark = "+"
		case "missing":
			mark = "-"
		default:
			mark = "!"
		}
		detail := ""
		if item.ProvenBy != "" {
			detail = fmt.Sprintf(" [%s by %s]", item.EvidenceType, item.ProvenBy)
		}
		if item.StaleReason != "" {
			detail += fmt.Sprintf(" (%s)", item.StaleReason)
		}
		fmt.Fprintf(&b, "  %s %-18s %-7s %s%s\n", mark, item.InvariantID, item.Status, item.Title, detail)
	}

	return b.String()
}

func renderHumanSingle(w *storage.InvariantWitness) string {
	var b strings.Builder
	fmt.Fprintf(&b, "Witness: %s\n", w.InvariantID)
	fmt.Fprintf(&b, "  Status:    %s\n", w.Status)
	fmt.Fprintf(&b, "  Type:      %s\n", w.EvidenceType)
	fmt.Fprintf(&b, "  By:        %s\n", w.ProvenBy)
	if w.Model != "" {
		fmt.Fprintf(&b, "  Model:     %s\n", w.Model)
	}
	fmt.Fprintf(&b, "  At:        %s\n", w.ProvenAt)
	fmt.Fprintf(&b, "  Spec hash: %s\n", w.SpecHash)
	if w.CodeHash != "" {
		fmt.Fprintf(&b, "  Code hash: %s\n", w.CodeHash)
	}
	fmt.Fprintf(&b, "  Evidence:  %s\n", truncate(w.Evidence, 120))
	if w.Notes != "" {
		fmt.Fprintf(&b, "  Notes:     %s\n", w.Notes)
	}
	return b.String()
}

func renderHumanList(witnesses []storage.InvariantWitness) string {
	if len(witnesses) == 0 {
		return "No witnesses recorded.\n"
	}

	var b strings.Builder
	fmt.Fprintf(&b, "Witnesses (%d total)\n", len(witnesses))
	b.WriteString(strings.Repeat("═", 60) + "\n")

	for _, w := range witnesses {
		modelStr := ""
		if w.Model != "" {
			modelStr = " (" + w.Model + ")"
		}
		fmt.Fprintf(&b, "  %-18s %-7s %-12s by %s%s  %s\n",
			w.InvariantID, w.Status, w.EvidenceType, w.ProvenBy, modelStr, w.ProvenAt)
	}
	return b.String()
}
