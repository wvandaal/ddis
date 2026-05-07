package parser

import (
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// matchInvHeader tries both the canonical bold form (**NS-INV-NNN: Title**)
// and the CMP-dialect h3 form (### NS-INV-NNN — Title). Returns the captured
// id, title, conditional tag, and a true if either form matched.
func matchInvHeader(trimmed string) (id, title, cond string, ok bool) {
	if m := InvHeaderRe.FindStringSubmatch(trimmed); m != nil {
		return m[1], strings.TrimSpace(m[2]), conditionalOrEmpty(m, 3), true
	}
	if m := InvHeaderH3Re.FindStringSubmatch(trimmed); m != nil {
		return m[1], strings.TrimSpace(m[2]), conditionalOrEmpty(m, 3), true
	}
	return "", "", "", false
}

func conditionalOrEmpty(m []string, idx int) string {
	if len(m) > idx && m[idx] != "" {
		return m[idx]
	}
	return ""
}

// looksLikeMultilineItalicOpen returns true for a line that opens an italic
// block but does not close it on the same line. Excludes markdown bullets
// (`* item`) and bold opens (`**bold**`) so the multi-line tracker only
// triggers on genuine italic statement openings.
func looksLikeMultilineItalicOpen(trimmed string) bool {
	if len(trimmed) < 2 || trimmed[0] != '*' {
		return false
	}
	if trimmed[1] == '*' || trimmed[1] == ' ' {
		return false // bold or bullet
	}
	if strings.HasSuffix(trimmed, "*") {
		return false // single-line italic, handled elsewhere
	}
	return true
}

// ExtractInvariants finds invariant blocks within the given line range.
// When diags is non-nil, incomplete invariant headers are reported as diagnostics
// instead of being silently discarded.
func ExtractInvariants(lines []string, sections []*SectionNode, specID, sourceFileID int64, db storage.DB, diags ...*Diagnostics) error {
	var diagSink *Diagnostics
	if len(diags) > 0 {
		diagSink = diags[0]
	}
	_ = diagSink // used below
	type invState int
	const (
		idle invState = iota
		headerSeen
		inMultilineStatement
		statementSeen
		inCodeBlock
		codeDone
		afterCode
	)

	state := idle
	var current storage.Invariant
	var rawLines []string
	var codeFence string

	for i, line := range lines {
		trimmed := strings.TrimSpace(line)

		switch state {
		case idle:
			if id, title, cond, ok := matchInvHeader(trimmed); ok {
				state = headerSeen
				current = storage.Invariant{
					SpecID:         specID,
					SourceFileID:   sourceFileID,
					InvariantID:    id,
					Title:          title,
					LineStart:      i + 1, // 1-indexed
					ConditionalTag: cond,
				}
				rawLines = []string{line}

				sec := FindSectionForLine(sections, i)
				if sec != nil {
					current.SectionID = sec.DBID
				}
			}

		case headerSeen:
			rawLines = append(rawLines, line)
			if trimmed == "" {
				continue
			}
			if m := InvStatementRe.FindStringSubmatch(trimmed); m != nil {
				current.Statement = m[1]
				state = statementSeen
			} else if looksLikeMultilineItalicOpen(trimmed) {
				current.Statement = strings.TrimPrefix(trimmed, "*")
				state = inMultilineStatement
			} else {
				// Not a valid invariant block — emit diagnostic and reset
				if diagSink != nil {
					diagSink.Add(ParseDiagnostic{
						ElementID:  current.InvariantID,
						Line:       current.LineStart,
						Deficiency: "missing statement (expected italic *...*)",
					})
				}
				state = idle
			}

		case inMultilineStatement:
			rawLines = append(rawLines, line)
			if strings.HasSuffix(trimmed, "*") && !strings.HasSuffix(trimmed, "**") {
				current.Statement += " " + strings.TrimSuffix(trimmed, "*")
				current.Statement = strings.TrimSpace(current.Statement)
				state = statementSeen
			} else if trimmed == "" {
				// Blank line inside a multi-line italic block is unusual but
				// preserve as a paragraph break to avoid losing structure.
				current.Statement += " "
			} else {
				current.Statement += " " + trimmed
			}

		case statementSeen:
			rawLines = append(rawLines, line)
			if trimmed == "" {
				continue
			}
			if m := CodeFenceRe.FindStringSubmatch(trimmed); m != nil {
				codeFence = m[1]
				state = inCodeBlock
				current.SemiFormal = ""
			} else if m := ViolationRe.FindStringSubmatch(trimmed); m != nil {
				current.ViolationScenario = m[1]
				state = afterCode
			}

		case inCodeBlock:
			rawLines = append(rawLines, line)
			if strings.HasPrefix(trimmed, codeFence) && len(trimmed) <= len(codeFence)+1 {
				state = codeDone
			} else {
				if current.SemiFormal != "" {
					current.SemiFormal += "\n"
				}
				current.SemiFormal += line
			}

		case codeDone:
			rawLines = append(rawLines, line)
			if trimmed == "" {
				continue
			}
			if m := ViolationRe.FindStringSubmatch(trimmed); m != nil {
				current.ViolationScenario = m[1]
				state = afterCode
			} else if trimmed == "---" {
				// Terminate invariant without violation/validation
				current.LineEnd = i + 1
				current.RawText = strings.Join(rawLines, "\n")
				current.ContentHash = sha256Hex(current.RawText)
				if _, err := storage.InsertInvariant(db, &current); err != nil {
					return err
				}
				state = idle
				rawLines = nil
			} else if id, title, cond, ok := matchInvHeader(trimmed); ok {
				// Next invariant starts — flush current
				current.LineEnd = i
				current.RawText = strings.Join(rawLines[:len(rawLines)-1], "\n")
				current.ContentHash = sha256Hex(current.RawText)
				if _, err := storage.InsertInvariant(db, &current); err != nil {
					return err
				}
				state = headerSeen
				current = storage.Invariant{
					SpecID:         specID,
					SourceFileID:   sourceFileID,
					InvariantID:    id,
					Title:          title,
					LineStart:      i + 1,
					ConditionalTag: cond,
				}
				rawLines = []string{line}
				sec := FindSectionForLine(sections, i)
				if sec != nil {
					current.SectionID = sec.DBID
				}
			}

		case afterCode:
			rawLines = append(rawLines, line)
			if m := ValidationRe.FindStringSubmatch(trimmed); m != nil {
				current.ValidationMethod = m[1]
			} else if m := WhyMattersRe.FindStringSubmatch(trimmed); m != nil {
				current.WhyThisMatters = m[1]
			} else if trimmed != "" && current.ViolationScenario != "" &&
				!strings.HasPrefix(trimmed, "Violation") &&
				!strings.HasPrefix(trimmed, "Validation") &&
				!strings.HasPrefix(trimmed, "//") {
				// Multi-line continuation of violation or validation
				if current.ValidationMethod != "" {
					current.ValidationMethod += " " + trimmed
				} else {
					current.ViolationScenario += " " + trimmed
				}
			}

			// Terminate on --- or next invariant header
			if trimmed == "---" {
				current.LineEnd = i + 1
				current.RawText = strings.Join(rawLines, "\n")
				current.ContentHash = sha256Hex(current.RawText)

				if _, err := storage.InsertInvariant(db, &current); err != nil {
					return err
				}
				state = idle
				rawLines = nil
			} else if id, title, cond, ok := matchInvHeader(trimmed); ok {
				// Next invariant starts — flush current without consuming this line
				current.LineEnd = i
				current.RawText = strings.Join(rawLines[:len(rawLines)-1], "\n")
				current.ContentHash = sha256Hex(current.RawText)

				if _, err := storage.InsertInvariant(db, &current); err != nil {
					return err
				}

				// Re-process this line as a new invariant header
				state = headerSeen
				current = storage.Invariant{
					SpecID:         specID,
					SourceFileID:   sourceFileID,
					InvariantID:    id,
					Title:          title,
					LineStart:      i + 1,
					ConditionalTag: cond,
				}
				rawLines = []string{line}
				sec := FindSectionForLine(sections, i)
				if sec != nil {
					current.SectionID = sec.DBID
				}
			}
		}
	}

	// Flush any remaining invariant at EOF
	if state >= headerSeen && state != idle {
		current.LineEnd = len(lines)
		current.RawText = strings.Join(rawLines, "\n")
		current.ContentHash = sha256Hex(current.RawText)
		if current.Statement != "" {
			if sec := FindSectionForLine(sections, current.LineStart-1); sec != nil {
				current.SectionID = sec.DBID
			}
			if _, err := storage.InsertInvariant(db, &current); err != nil {
				return err
			}
		}
	}

	return nil
}

func isInvariantComplete(inv *storage.Invariant) bool {
	return inv.Statement != "" &&
		(inv.ViolationScenario != "" || inv.WhyThisMatters != "")
}
