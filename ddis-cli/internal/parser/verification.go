package parser

import (
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ExtractVerificationPrompts finds "### Verification Prompt for ..." blocks.
func ExtractVerificationPrompts(lines []string, sections []*SectionNode, specID int64, db storage.DB) error {
	for i, line := range lines {
		trimmed := strings.TrimSpace(line)
		m := VerifPromptRe.FindStringSubmatch(trimmed)
		if m == nil {
			continue
		}

		chapterName := strings.TrimSpace(m[1])
		startLine := i

		// Collect everything until next heading or ---
		var rawLines []string
		rawLines = append(rawLines, line)
		endLine := i + 1

		var checks []verifCheck

		for j := i + 1; j < len(lines); j++ {
			jTrimmed := strings.TrimSpace(lines[j])

			// Stop at next heading or ---
			if HeadingRe.MatchString(jTrimmed) || jTrimmed == "---" {
				endLine = j + 1
				break
			}

			rawLines = append(rawLines, lines[j])
			endLine = j + 1

			// Parse checklist items: N. [ ] text
			if len(jTrimmed) > 4 && (jTrimmed[0] >= '0' && jTrimmed[0] <= '9') {
				// Find the [ ] or [x] part
				bracketIdx := strings.Index(jTrimmed, "[ ]")
				if bracketIdx < 0 {
					bracketIdx = strings.Index(jTrimmed, "[x]")
				}
				if bracketIdx >= 0 {
					checkText := strings.TrimSpace(jTrimmed[bracketIdx+3:])
					checkType := categorizeCheck(checkText)
					var invRefStr string
					if refs := XRefInvRe.FindAllString(checkText, -1); len(refs) > 0 {
						invRefStr = strings.Join(refs, ", ")
					}
					checks = append(checks, verifCheck{
						text:    checkText,
						typ:     checkType,
						invRef:  invRefStr,
						ordinal: len(checks) + 1,
					})
				}
			}
		}

		sec := FindSectionForLine(sections, startLine)
		var sectionID int64
		if sec != nil {
			sectionID = sec.DBID
		}

		vp := &storage.VerificationPrompt{
			SpecID:      specID,
			SectionID:   sectionID,
			ChapterName: chapterName,
			LineStart:   startLine + 1,
			LineEnd:     endLine,
			RawText:     strings.Join(rawLines, "\n"),
		}

		vpID, err := storage.InsertVerificationPrompt(db, vp)
		if err != nil {
			return err
		}

		// Insert individual checks
		for _, c := range checks {
			vc := &storage.VerificationCheck{
				PromptID:     vpID,
				CheckType:    c.typ,
				CheckText:    c.text,
				InvariantRef: c.invRef,
				Ordinal:      c.ordinal,
			}
			if _, err := storage.InsertVerificationCheck(db, vc); err != nil {
				return err
			}
		}
	}
	return nil
}

type verifCheck struct {
	text    string
	typ     string
	invRef  string
	ordinal int
}

func categorizeCheck(text string) string {
	lower := strings.ToLower(text)
	if strings.Contains(lower, "not ") || strings.Contains(lower, "does not") ||
		strings.Contains(lower, "no ") || strings.HasPrefix(lower, "never") {
		return "negative"
	}
	if strings.Contains(lower, "integration") || strings.Contains(lower, "map to") ||
		strings.Contains(lower, "traces") || strings.Contains(lower, "cross-ref") {
		return "integration"
	}
	return "positive"
}
