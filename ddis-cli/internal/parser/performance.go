package parser

import (
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ExtractPerformanceBudgets finds performance budget tables.
func ExtractPerformanceBudgets(lines []string, sections []*SectionNode, specID int64, db storage.DB) error {
	for i, line := range lines {
		trimmed := strings.TrimSpace(line)

		// Look for a heading or bold text mentioning "performance budget"
		if !PerfBudgetHeaderRe.MatchString(trimmed) {
			continue
		}

		// Only match headings or bold text, not random mentions
		isHeading := HeadingRe.MatchString(trimmed)
		isBold := strings.HasPrefix(trimmed, "**") || strings.HasPrefix(trimmed, "| ")
		if !isHeading && !isBold {
			continue
		}

		startLine := i
		var rawLines []string
		rawLines = append(rawLines, line)

		// Find the table that follows
		var tableStart, tableEnd int
		var headers []string
		foundTable := false

		for j := i + 1; j < len(lines); j++ {
			jTrimmed := strings.TrimSpace(lines[j])
			rawLines = append(rawLines, lines[j])

			if HeadingRe.MatchString(jTrimmed) && j > i+1 {
				rawLines = rawLines[:len(rawLines)-1]
				tableEnd = j
				break
			}

			if jTrimmed == "---" && j > i+3 {
				tableEnd = j
				break
			}

			// Detect table header
			if !foundTable && TableRowRe.MatchString(jTrimmed) {
				cols := splitTableRow(jTrimmed)
				if len(cols) >= 2 {
					headers = cols
					tableStart = j
					foundTable = true
				}
			}

			tableEnd = j + 1
		}

		if !foundTable {
			continue
		}

		sec := FindSectionForLine(sections, startLine)
		var sectionID int64
		if sec != nil {
			sectionID = sec.DBID
		}

		// Extract design point if present
		var designPoint string
		for _, rl := range rawLines {
			rt := strings.TrimSpace(rl)
			if strings.Contains(strings.ToLower(rt), "design point") {
				designPoint = rt
				break
			}
		}

		pb := &storage.PerformanceBudget{
			SpecID:      specID,
			SectionID:   sectionID,
			DesignPoint: designPoint,
			LineStart:   startLine + 1,
			LineEnd:     tableEnd,
			RawText:     strings.Join(rawLines, "\n"),
		}

		pbID, err := storage.InsertPerformanceBudget(db, pb)
		if err != nil {
			return err
		}

		// Parse table rows into budget entries
		ordinal := 0
		for j := tableStart + 1; j < tableEnd && j < len(lines); j++ {
			jTrimmed := strings.TrimSpace(lines[j])
			if TableSepRe.MatchString(jTrimmed) {
				continue
			}
			if !TableRowRe.MatchString(jTrimmed) {
				continue
			}
			cols := splitTableRow(jTrimmed)
			if len(cols) < 2 {
				continue
			}

			ordinal++
			be := &storage.BudgetEntry{
				BudgetID: pbID,
				Ordinal:  ordinal,
			}

			// Map columns by header names
			for ci, col := range cols {
				if ci >= len(headers) {
					break
				}
				hdr := strings.ToLower(strings.TrimSpace(headers[ci]))
				val := strings.TrimSpace(col)
				switch {
				case strings.Contains(hdr, "metric") || strings.Contains(hdr, "id"):
					be.MetricID = val
				case strings.Contains(hdr, "operation") || strings.Contains(hdr, "what"):
					be.Operation = val
				case strings.Contains(hdr, "target") || strings.Contains(hdr, "budget"):
					be.Target = val
				case strings.Contains(hdr, "method") || strings.Contains(hdr, "how"):
					be.MeasurementMethod = val
				}
			}

			if be.Operation == "" && len(cols) >= 1 {
				be.Operation = strings.TrimSpace(cols[0])
			}
			if be.Target == "" && len(cols) >= 2 {
				be.Target = strings.TrimSpace(cols[1])
			}

			if _, err := storage.InsertBudgetEntry(db, be); err != nil {
				return err
			}
		}
	}
	return nil
}

func splitTableRow(line string) []string {
	line = strings.TrimSpace(line)
	line = strings.TrimPrefix(line, "|")
	line = strings.TrimSuffix(line, "|")
	parts := strings.Split(line, "|")
	var result []string
	for _, p := range parts {
		result = append(result, strings.TrimSpace(p))
	}
	return result
}
