package parser

import (
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ExtractWorkedExamples finds worked example blocks.
func ExtractWorkedExamples(lines []string, sections []*SectionNode, specID int64, db storage.DB) error {
	for i, line := range lines {
		trimmed := strings.TrimSpace(line)
		if !WorkedExampleRe.MatchString(trimmed) {
			continue
		}

		title := trimmed
		// Clean up the title
		title = strings.TrimPrefix(title, "#### ")
		title = strings.TrimPrefix(title, "### ")
		title = strings.TrimPrefix(title, "## ")
		title = strings.Trim(title, "*")
		title = strings.TrimSuffix(title, ":")
		title = strings.TrimSpace(title)

		startLine := i

		// Collect everything until next heading at same or higher level
		var rawLines []string
		rawLines = append(rawLines, line)
		endLine := i + 1

		// Determine the heading level of this example
		headingLevel := 0
		if hm := HeadingRe.FindStringSubmatch(trimmed); hm != nil {
			headingLevel = len(hm[1])
		}

		for j := i + 1; j < len(lines); j++ {
			jTrimmed := strings.TrimSpace(lines[j])
			if hm := HeadingRe.FindStringSubmatch(jTrimmed); hm != nil {
				thisLevel := len(hm[1])
				if headingLevel > 0 && thisLevel <= headingLevel {
					endLine = j + 1
					break
				}
			}
			if jTrimmed == "---" {
				endLine = j + 1
				break
			}
			rawLines = append(rawLines, lines[j])
			endLine = j + 1
		}

		sec := FindSectionForLine(sections, startLine)
		var sectionID int64
		if sec != nil {
			sectionID = sec.DBID
		}

		we := &storage.WorkedExample{
			SpecID:    specID,
			SectionID: sectionID,
			Title:     title,
			LineStart: startLine + 1,
			LineEnd:   endLine,
			RawText:   strings.Join(rawLines, "\n"),
		}

		if _, err := storage.InsertWorkedExample(db, we); err != nil {
			return err
		}
	}
	return nil
}
