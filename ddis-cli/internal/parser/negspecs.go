package parser

import (
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ExtractNegativeSpecs finds **DO NOT** constraint lines.
func ExtractNegativeSpecs(lines []string, sections []*SectionNode, specID, sourceFileID int64, db storage.DB) error {
	for i, line := range lines {
		trimmed := strings.TrimSpace(line)
		m := NegSpecRe.FindStringSubmatch(trimmed)
		if m == nil {
			continue
		}

		constraintText := m[1]

		// Extract reason: text after the constraint in parens or after dash
		var reason, invRef string

		// Check for (Validates INV-NNN) or (Validates INV-NNN, INV-NNN) pattern
		if idx := strings.LastIndex(constraintText, "(Validates "); idx >= 0 {
			tail := constraintText[idx:]
			reason = tail
			constraintText = strings.TrimSpace(constraintText[:idx])
			// Extract invariant references from the validation note
			if refs := XRefInvRe.FindAllString(tail, -1); len(refs) > 0 {
				invRef = strings.Join(refs, ", ")
			}
		} else if idx := strings.LastIndex(constraintText, "("); idx >= 0 {
			tail := constraintText[idx:]
			reason = strings.Trim(tail, "()")
			constraintText = strings.TrimSpace(constraintText[:idx])
		}

		// Remove trailing period from constraint text
		constraintText = strings.TrimRight(constraintText, ".")

		sec := FindSectionForLine(sections, i)
		var sectionID int64
		if sec != nil {
			sectionID = sec.DBID
		}

		ns := &storage.NegativeSpec{
			SpecID:         specID,
			SourceFileID:   sourceFileID,
			SectionID:      sectionID,
			ConstraintText: constraintText,
			Reason:         reason,
			InvariantRef:   invRef,
			LineNumber:     i + 1,
			RawText:        line,
		}

		if _, err := storage.InsertNegativeSpec(db, ns); err != nil {
			return err
		}
	}
	return nil
}
