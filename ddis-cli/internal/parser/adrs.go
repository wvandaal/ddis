package parser

import (
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ExtractADRs finds ADR blocks within the given lines.
func ExtractADRs(lines []string, sections []*SectionNode, specID, sourceFileID int64, db storage.DB) error {
	type adrState int
	const (
		idle adrState = iota
		headerSeen
		inProblem
		inOptions
		inDecision
		inConsequences
		inTests
	)

	state := idle
	var current storage.ADR
	var rawLines []string
	var currentSection string
	var options []*storage.ADROption
	var currentOpt *storage.ADROption
	var chosenLabel string

	flush := func(endLine int) error {
		if current.ADRID == "" {
			return nil
		}
		current.LineEnd = endLine
		current.RawText = strings.Join(rawLines, "\n")
		current.ContentHash = sha256Hex(current.RawText)
		if current.Status == "" {
			current.Status = "active"
		}

		adrDBID, err := storage.InsertADR(db, &current)
		if err != nil {
			return err
		}

		// Flush current option if pending
		if currentOpt != nil {
			options = append(options, currentOpt)
			currentOpt = nil
		}

		for _, opt := range options {
			opt.ADRID = adrDBID
			if opt.OptionLabel == chosenLabel {
				opt.IsChosen = true
			}
			if _, err := storage.InsertADROption(db, opt); err != nil {
				return err
			}
		}
		return nil
	}

	for i, line := range lines {
		trimmed := strings.TrimSpace(line)

		// Check for a new ADR header (always resets state)
		if m := ADRHeaderRe.FindStringSubmatch(trimmed); m != nil {
			if state != idle {
				if err := flush(i + 1); err != nil {
					return err
				}
			}
			state = headerSeen
			current = storage.ADR{
				SpecID:       specID,
				SourceFileID: sourceFileID,
				ADRID:        m[1],
				Title:        strings.TrimSpace(m[2]),
				LineStart:    i + 1,
			}
			sec := FindSectionForLine(sections, i)
			if sec != nil {
				current.SectionID = sec.DBID
			}
			rawLines = []string{line}
			options = nil
			currentOpt = nil
			chosenLabel = ""
			currentSection = ""
			continue
		}

		if state == idle {
			continue
		}

		rawLines = append(rawLines, line)

		// Check for subheadings
		if m := ADRSubheadingRe.FindStringSubmatch(trimmed); m != nil {
			// Flush current option if we're leaving Options
			if currentOpt != nil && currentSection == "Options" && m[1] != "Options" {
				options = append(options, currentOpt)
				currentOpt = nil
			}

			currentSection = m[1]
			switch currentSection {
			case "Problem":
				state = inProblem
			case "Options":
				state = inOptions
			case "Decision":
				state = inDecision
			case "Consequences":
				state = inConsequences
			case "Tests":
				state = inTests
			}
			continue
		}

		// Check for ADR block end (--- on its own line)
		if trimmed == "---" && state != idle {
			if err := flush(i + 1); err != nil {
				return err
			}
			state = idle
			continue
		}

		// Accumulate content based on state
		switch state {
		case inProblem:
			if trimmed != "" {
				if current.Problem != "" {
					current.Problem += "\n"
				}
				current.Problem += trimmed
			}

		case inOptions:
			if m := ADROptionRe.FindStringSubmatch(trimmed); m != nil {
				// Flush previous option
				if currentOpt != nil {
					options = append(options, currentOpt)
				}
				currentOpt = &storage.ADROption{
					OptionLabel: m[1],
					OptionName:  m[2],
				}
			} else if currentOpt != nil {
				if m := ADRProsConsRe.FindStringSubmatch(trimmed); m != nil {
					if m[1] == "Pros" {
						currentOpt.Pros = m[2]
					} else {
						currentOpt.Cons = m[2]
					}
				}
			}

		case inDecision:
			if trimmed != "" {
				if current.DecisionText != "" {
					current.DecisionText += "\n"
				}
				current.DecisionText += trimmed
			}
			// Check for chosen option
			if m := ADRChosenRe.FindStringSubmatch(trimmed); m != nil {
				chosenLabel = m[1]
				current.ChosenOption = "Option " + m[1]
			}
			// Check for confidence
			if m := ConfidenceRe.FindStringSubmatch(trimmed); m != nil {
				current.Confidence = m[1]
			}
			// Check for WHY NOT
			if m := WhyNotRe.FindStringSubmatch(trimmed); m != nil {
				// This is part of the decision narrative, already captured
			}

		case inConsequences:
			if trimmed != "" {
				if current.Consequences != "" {
					current.Consequences += "\n"
				}
				current.Consequences += trimmed
			}

		case inTests:
			if trimmed != "" {
				if current.Tests != "" {
					current.Tests += "\n"
				}
				current.Tests += trimmed
			}
		}
	}

	// Flush remaining ADR at EOF
	if state != idle {
		if err := flush(len(lines)); err != nil {
			return err
		}
	}

	return nil
}
