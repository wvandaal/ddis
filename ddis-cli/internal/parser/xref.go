package parser

import (
	"database/sql"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ExtractCrossReferences finds all cross-references in the document.
func ExtractCrossReferences(lines []string, sections []*SectionNode, specID, sourceFileID int64, db storage.DB) error {
	inCodeBlock := false

	for i, line := range lines {
		trimmed := strings.TrimSpace(line)

		// Skip code blocks
		if CodeFenceRe.MatchString(trimmed) {
			inCodeBlock = !inCodeBlock
			continue
		}
		if inCodeBlock {
			continue
		}

		sec := FindSectionForLine(sections, i)
		var sectionID *int64
		if sec != nil {
			id := sec.DBID
			sectionID = &id
		}

		// Section references: §N.M
		for _, m := range XRefSectionRe.FindAllStringSubmatch(line, -1) {
			xr := &storage.CrossReference{
				SpecID:          specID,
				SourceFileID:    sourceFileID,
				SourceSectionID: sectionID,
				SourceLine:      i + 1,
				RefType:         "section",
				RefTarget:       "§" + m[1],
				RefText:         m[0],
			}
			if _, err := storage.InsertCrossReference(db, xr); err != nil {
				return err
			}
		}

		// Invariant references: INV-NNN or APP-INV-NNN
		for _, m := range XRefInvRe.FindAllStringSubmatch(line, -1) {
			// Skip if this is the definition line itself (invariant header)
			if InvHeaderRe.MatchString(trimmed) {
				continue
			}
			refType := "invariant"
			if strings.HasPrefix(m[1], "APP-") {
				refType = "app_invariant"
			}
			xr := &storage.CrossReference{
				SpecID:          specID,
				SourceFileID:    sourceFileID,
				SourceSectionID: sectionID,
				SourceLine:      i + 1,
				RefType:         refType,
				RefTarget:       m[1],
				RefText:         m[0],
			}
			if _, err := storage.InsertCrossReference(db, xr); err != nil {
				return err
			}
		}

		// ADR references: ADR-NNN or APP-ADR-NNN
		for _, m := range XRefADRRe.FindAllStringSubmatch(line, -1) {
			// Skip ADR definition headers
			if ADRHeaderRe.MatchString(trimmed) {
				continue
			}
			refType := "adr"
			if strings.HasPrefix(m[1], "APP-") {
				refType = "app_adr"
			}
			xr := &storage.CrossReference{
				SpecID:          specID,
				SourceFileID:    sourceFileID,
				SourceSectionID: sectionID,
				SourceLine:      i + 1,
				RefType:         refType,
				RefTarget:       m[1],
				RefText:         m[0],
			}
			if _, err := storage.InsertCrossReference(db, xr); err != nil {
				return err
			}
		}

		// Gate references: Gate N or Gate M-N
		for _, m := range XRefGateRe.FindAllStringSubmatch(line, -1) {
			// Skip gate definition lines
			if GateRe.MatchString(trimmed) {
				continue
			}
			xr := &storage.CrossReference{
				SpecID:          specID,
				SourceFileID:    sourceFileID,
				SourceSectionID: sectionID,
				SourceLine:      i + 1,
				RefType:         "gate",
				RefTarget:       "Gate-" + m[1],
				RefText:         m[0],
			}
			if _, err := storage.InsertCrossReference(db, xr); err != nil {
				return err
			}
		}
	}
	return nil
}

// ResolveCrossReferences checks if each cross-reference target exists.
func ResolveCrossReferences(db storage.DB, specID int64) error {
	rows, err := db.Query(
		`SELECT id, ref_type, ref_target FROM cross_references WHERE spec_id = ?`, specID)
	if err != nil {
		return err
	}
	defer rows.Close()

	type xref struct {
		id     int64
		typ    string
		target string
	}
	var refs []xref
	for rows.Next() {
		var r xref
		if err := rows.Scan(&r.id, &r.typ, &r.target); err != nil {
			return err
		}
		refs = append(refs, r)
	}

	for _, r := range refs {
		var exists bool
		switch r.typ {
		case "section":
			exists = queryExists(db,
				`SELECT 1 FROM sections WHERE spec_id = ? AND section_path = ?`,
				specID, r.target)
		case "invariant", "app_invariant":
			exists = queryExists(db,
				`SELECT 1 FROM invariants WHERE spec_id = ? AND invariant_id = ?`,
				specID, r.target)
		case "adr", "app_adr":
			exists = queryExists(db,
				`SELECT 1 FROM adrs WHERE spec_id = ? AND adr_id = ?`,
				specID, r.target)
		case "gate":
			exists = queryExists(db,
				`SELECT 1 FROM quality_gates WHERE spec_id = ? AND gate_id = ?`,
				specID, r.target)
		}

		if exists {
			if _, err := db.Exec(
				`UPDATE cross_references SET resolved = 1 WHERE id = ?`, r.id); err != nil {
				return err
			}
		}
	}
	return nil
}

func queryExists(db *sql.DB, query string, args ...interface{}) bool {
	var x int
	err := db.QueryRow(query, args...).Scan(&x)
	return err == nil
}
