package parser

import (
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ExtractWhyNots finds // WHY NOT ... annotations.
func ExtractWhyNots(lines []string, sections []*SectionNode, specID int64, db storage.DB) error {
	for i, line := range lines {
		trimmed := strings.TrimSpace(line)
		m := WhyNotRe.FindStringSubmatch(trimmed)
		if m == nil {
			continue
		}

		alternative := strings.TrimSpace(m[1])
		explanation := strings.TrimSpace(m[2])

		// Extract ADR reference from explanation
		var adrRef string
		if refs := XRefADRRe.FindAllString(explanation, -1); len(refs) > 0 {
			adrRef = strings.Join(refs, ", ")
		}

		sec := FindSectionForLine(sections, i)
		var sectionID int64
		if sec != nil {
			sectionID = sec.DBID
		}

		wn := &storage.WhyNotAnnotation{
			SpecID:      specID,
			SectionID:   sectionID,
			Alternative: alternative,
			Explanation: explanation,
			ADRRef:      adrRef,
			LineNumber:  i + 1,
			RawText:     line,
		}

		if _, err := storage.InsertWhyNotAnnotation(db, wn); err != nil {
			return err
		}
	}
	return nil
}

// ExtractComparisonBlocks finds ❌/✅ comparison blocks.
func ExtractComparisonBlocks(lines []string, sections []*SectionNode, specID int64, db storage.DB) error {
	i := 0
	for i < len(lines) {
		trimmed := strings.TrimSpace(lines[i])
		if !ComparisonBadRe.MatchString(trimmed) {
			i++
			continue
		}

		startLine := i
		suboptimal := strings.TrimSpace(strings.TrimPrefix(trimmed, "❌"))
		var suboptimalReasons []string
		var rawLines []string
		rawLines = append(rawLines, lines[i])
		i++

		// Collect suboptimal reasons until ✅
		for i < len(lines) {
			t := strings.TrimSpace(lines[i])
			rawLines = append(rawLines, lines[i])
			if ComparisonGoodRe.MatchString(t) {
				break
			}
			if t != "" && !ComparisonBadRe.MatchString(t) {
				suboptimalReasons = append(suboptimalReasons, t)
			}
			i++
		}

		if i >= len(lines) {
			break
		}

		chosenLine := strings.TrimSpace(lines[i])
		chosen := strings.TrimSpace(strings.TrimPrefix(chosenLine, "✅"))
		var chosenReasons []string
		i++

		// Collect chosen reasons until next element or blank line gap
		blankCount := 0
		for i < len(lines) {
			t := strings.TrimSpace(lines[i])
			if t == "" {
				blankCount++
				if blankCount >= 2 {
					break
				}
				rawLines = append(rawLines, lines[i])
				i++
				continue
			}
			blankCount = 0
			if HeadingRe.MatchString(t) || ComparisonBadRe.MatchString(t) || t == "---" {
				break
			}
			rawLines = append(rawLines, lines[i])
			chosenReasons = append(chosenReasons, t)
			i++
		}

		sec := FindSectionForLine(sections, startLine)
		var sectionID int64
		if sec != nil {
			sectionID = sec.DBID
		}

		// Look for ADR reference
		var adrRef string
		rawText := strings.Join(rawLines, "\n")
		if refs := XRefADRRe.FindAllString(rawText, -1); len(refs) > 0 {
			adrRef = strings.Join(refs, ", ")
		}

		cb := &storage.ComparisonBlock{
			SpecID:             specID,
			SectionID:          sectionID,
			SuboptimalApproach: suboptimal,
			ChosenApproach:     chosen,
			SuboptimalReasons:  strings.Join(suboptimalReasons, "\n"),
			ChosenReasons:      strings.Join(chosenReasons, "\n"),
			ADRRef:             adrRef,
			LineStart:          startLine + 1,
			LineEnd:            i,
			RawText:            rawText,
		}

		if _, err := storage.InsertComparisonBlock(db, cb); err != nil {
			return err
		}
	}
	return nil
}
