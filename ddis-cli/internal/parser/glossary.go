package parser

import (
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ExtractGlossaryEntries finds glossary table rows: | **Term** | Definition |
func ExtractGlossaryEntries(lines []string, sections []*SectionNode, specID int64, db storage.DB) error {
	// First, find the glossary section(s)
	inGlossary := false

	for i, line := range lines {
		trimmed := strings.TrimSpace(line)

		// Detect glossary section by heading
		if HeadingRe.MatchString(trimmed) {
			lower := strings.ToLower(trimmed)
			if strings.Contains(lower, "glossary") {
				inGlossary = true
			} else if inGlossary {
				// Left the glossary section
				inGlossary = false
			}
			continue
		}

		if !inGlossary {
			continue
		}

		m := GlossaryRowRe.FindStringSubmatch(trimmed)
		if m == nil {
			continue
		}

		term := strings.TrimSpace(m[1])
		definition := strings.TrimSpace(m[2])

		// Skip table headers like "Term | Definition"
		if strings.ToLower(term) == "term" {
			continue
		}

		// Extract section references from definition
		var sectionRef string
		if refs := XRefSectionRe.FindAllString(definition, -1); len(refs) > 0 {
			sectionRef = strings.Join(refs, ", ")
		}

		sec := FindSectionForLine(sections, i)
		var sectionID int64
		if sec != nil {
			sectionID = sec.DBID
		}

		ge := &storage.GlossaryEntry{
			SpecID:     specID,
			SectionID:  sectionID,
			Term:       term,
			Definition: definition,
			SectionRef: sectionRef,
			LineNumber: i + 1,
		}

		if _, err := storage.InsertGlossaryEntry(db, ge); err != nil {
			return err
		}
	}
	return nil
}
