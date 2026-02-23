package parser

import (
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ExtractMetaInstructions finds > **META-INSTRUCTION**: ... blocks.
func ExtractMetaInstructions(lines []string, sections []*SectionNode, specID int64, db storage.DB) error {
	for i, line := range lines {
		trimmed := strings.TrimSpace(line)
		m := MetaInstrRe.FindStringSubmatch(trimmed)
		if m == nil {
			continue
		}

		directive := strings.TrimSpace(m[1])
		startLine := i

		// Collect continuation lines (still in blockquote >)
		var rawLines []string
		rawLines = append(rawLines, line)
		endLine := i + 1

		for j := i + 1; j < len(lines); j++ {
			jTrimmed := strings.TrimSpace(lines[j])
			if strings.HasPrefix(jTrimmed, ">") {
				continuation := strings.TrimSpace(strings.TrimPrefix(jTrimmed, ">"))
				if continuation != "" {
					directive += " " + continuation
				}
				rawLines = append(rawLines, lines[j])
				endLine = j + 1
			} else {
				break
			}
		}

		sec := FindSectionForLine(sections, startLine)
		var sectionID int64
		if sec != nil {
			sectionID = sec.DBID
		}

		mi := &storage.MetaInstruction{
			SpecID:    specID,
			SectionID: sectionID,
			Directive: directive,
			LineStart: startLine + 1,
			LineEnd:   endLine,
			RawText:   strings.Join(rawLines, "\n"),
		}

		if _, err := storage.InsertMetaInstruction(db, mi); err != nil {
			return err
		}
	}
	return nil
}
