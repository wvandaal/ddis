package parser

import (
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ExtractStateMachines finds state machine tables.
func ExtractStateMachines(lines []string, sections []*SectionNode, specID int64, db storage.DB) error {
	for i, line := range lines {
		trimmed := strings.TrimSpace(line)

		// Look for tables with state/event headers
		if !TableRowRe.MatchString(trimmed) {
			continue
		}
		if !StateMachineHeaderRe.MatchString(trimmed) {
			continue
		}

		// Verify next line is a table separator
		if i+1 >= len(lines) || !TableSepRe.MatchString(strings.TrimSpace(lines[i+1])) {
			continue
		}

		startLine := i
		headers := splitTableRow(trimmed)

		// Find a title: look backwards for a heading or bold text
		var title string
		for j := i - 1; j >= 0 && j >= i-5; j-- {
			jTrimmed := strings.TrimSpace(lines[j])
			if jTrimmed == "" {
				continue
			}
			if HeadingRe.MatchString(jTrimmed) {
				hm := HeadingRe.FindStringSubmatch(jTrimmed)
				title = hm[2]
				break
			}
			if strings.HasPrefix(jTrimmed, "**") {
				title = strings.Trim(jTrimmed, "*: ")
				break
			}
			break
		}

		var rawLines []string
		rawLines = append(rawLines, line)

		// Collect table rows
		var rows [][]string
		endLine := i + 1

		for j := i + 1; j < len(lines); j++ {
			jTrimmed := strings.TrimSpace(lines[j])
			rawLines = append(rawLines, lines[j])
			endLine = j + 1

			if TableSepRe.MatchString(jTrimmed) {
				continue
			}
			if TableRowRe.MatchString(jTrimmed) {
				rows = append(rows, splitTableRow(jTrimmed))
			} else {
				break
			}
		}

		sec := FindSectionForLine(sections, startLine)
		var sectionID int64
		if sec != nil {
			sectionID = sec.DBID
		}

		sm := &storage.StateMachine{
			SpecID:    specID,
			SectionID: sectionID,
			Title:     title,
			LineStart: startLine + 1,
			LineEnd:   endLine,
			RawText:   strings.Join(rawLines, "\n"),
		}

		smID, err := storage.InsertStateMachine(db, sm)
		if err != nil {
			return err
		}

		// Parse cells - figure out which column is state, event, transition, etc.
		stateCol, eventCol, transCol, guardCol := -1, -1, -1, -1
		for ci, h := range headers {
			hLower := strings.ToLower(h)
			switch {
			case strings.Contains(hLower, "state") && stateCol < 0:
				stateCol = ci
			case strings.Contains(hLower, "event"):
				eventCol = ci
			case strings.Contains(hLower, "transition") || strings.Contains(hLower, "next") || strings.Contains(hLower, "action"):
				transCol = ci
			case strings.Contains(hLower, "guard"):
				guardCol = ci
			}
		}

		// If it's a state x event matrix (no explicit event column)
		if eventCol < 0 && stateCol >= 0 && len(headers) > 2 {
			// Matrix form: first column is state, other columns are events
			for _, row := range rows {
				if len(row) <= stateCol {
					continue
				}
				stateName := strings.TrimSpace(row[stateCol])
				for ci := 0; ci < len(row); ci++ {
					if ci == stateCol || ci >= len(headers) {
						continue
					}
					eventName := strings.TrimSpace(headers[ci])
					transition := strings.TrimSpace(row[ci])
					if transition == "" || transition == "-" {
						continue
					}

					isInvalid := strings.Contains(strings.ToLower(transition), "invalid") ||
						strings.Contains(transition, "✗") ||
						strings.Contains(transition, "×")

					cell := &storage.StateMachineCell{
						MachineID:  smID,
						StateName:  stateName,
						EventName:  eventName,
						Transition: transition,
						IsInvalid:  isInvalid,
					}
					if _, err := storage.InsertStateMachineCell(db, cell); err != nil {
						return err
					}
				}
			}
		} else {
			// List form: state, event, transition columns
			for _, row := range rows {
				var stateName, eventName, transition, guard string
				if stateCol >= 0 && stateCol < len(row) {
					stateName = strings.TrimSpace(row[stateCol])
				}
				if eventCol >= 0 && eventCol < len(row) {
					eventName = strings.TrimSpace(row[eventCol])
				}
				if transCol >= 0 && transCol < len(row) {
					transition = strings.TrimSpace(row[transCol])
				}
				if guardCol >= 0 && guardCol < len(row) {
					guard = strings.TrimSpace(row[guardCol])
				}

				if stateName == "" && len(row) > 0 {
					stateName = strings.TrimSpace(row[0])
				}
				if transition == "" && len(row) > 1 {
					transition = strings.TrimSpace(row[len(row)-1])
				}

				isInvalid := strings.Contains(strings.ToLower(transition), "invalid") ||
					strings.Contains(transition, "✗")

				cell := &storage.StateMachineCell{
					MachineID:  smID,
					StateName:  stateName,
					EventName:  eventName,
					Transition: transition,
					Guard:      guard,
					IsInvalid:  isInvalid,
				}
				if _, err := storage.InsertStateMachineCell(db, cell); err != nil {
					return err
				}
			}
		}
	}
	return nil
}
