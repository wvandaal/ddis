package parser

import (
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ExtractGates finds quality gate blocks within the given lines.
func ExtractGates(lines []string, sections []*SectionNode, specID int64, db storage.DB) error {
	for i, line := range lines {
		trimmed := strings.TrimSpace(line)
		m := GateRe.FindStringSubmatch(trimmed)
		if m == nil {
			continue
		}

		gateID := "Gate-" + m[1]
		title := ""
		if len(m) > 2 {
			title = strings.TrimSpace(m[2])
		}

		isModular := strings.HasPrefix(m[1], "M-")

		// Collect predicate text: everything until next gate, ---, or blank+blank
		var predLines []string
		var rawLines []string
		rawLines = append(rawLines, line)
		endLine := i + 1

		for j := i + 1; j < len(lines); j++ {
			jTrimmed := strings.TrimSpace(lines[j])
			// Stop at next gate, section boundary, or horizontal rule
			if GateRe.MatchString(jTrimmed) || jTrimmed == "---" {
				endLine = j + 1
				break
			}
			if HeadingRe.MatchString(jTrimmed) {
				endLine = j + 1
				break
			}
			rawLines = append(rawLines, lines[j])
			if jTrimmed != "" {
				predLines = append(predLines, jTrimmed)
			}
			endLine = j + 1
		}

		sec := FindSectionForLine(sections, i)
		var sectionID int64
		if sec != nil {
			sectionID = sec.DBID
		}

		predicate := strings.Join(predLines, " ")

		g := &storage.QualityGate{
			SpecID:    specID,
			SectionID: sectionID,
			GateID:    gateID,
			Title:     title,
			Predicate: predicate,
			IsModular: isModular,
			LineStart: i + 1,
			LineEnd:   endLine,
			RawText:   strings.Join(rawLines, "\n"),
		}

		if _, err := storage.InsertQualityGate(db, g); err != nil {
			return err
		}
	}
	return nil
}
