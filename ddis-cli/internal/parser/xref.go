package parser

// ddis:implements APP-ADR-027 (peer spec relationships)

import (
	"database/sql"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-003 (cross-reference integrity)

// maskInlineCode replaces every inline-code span (`...`) with spaces of
// equal length so cross-reference regexes don't match references that
// appear inside code spans (e.g. a path like `docs/foo.md §3.2` mentions
// a section in another doc, not in our spec). Length preservation keeps
// column positions stable for callers that care about location.
func maskInlineCode(line string) string {
	return InlineCodeRe.ReplaceAllStringFunc(line, func(match string) string {
		return strings.Repeat(" ", len(match))
	})
}

// ExtractCrossReferences finds all cross-references in the document.
func ExtractCrossReferences(lines []string, sections []*SectionNode, specID, sourceFileID int64, db storage.DB) error {
	inCodeBlock := false

	for i, line := range lines {
		trimmed := strings.TrimSpace(line)

		// Skip fenced code blocks (``` ... ```)
		if CodeFenceRe.MatchString(trimmed) {
			inCodeBlock = !inCodeBlock
			continue
		}
		if inCodeBlock {
			continue
		}

		// Mask out inline code spans (`...`) so refs inside them aren't
		// extracted (e.g. a path like `docs/foo.md §3.2` is prose, not a
		// resolvable section reference). Replace the span with spaces of
		// equal length so column positions and line lengths are preserved.
		scanLine := maskInlineCode(line)

		sec := FindSectionForLine(sections, i)
		var sectionID *int64
		if sec != nil {
			id := sec.DBID
			sectionID = &id
		}

		// Section references: §N.M
		for _, m := range XRefSectionRe.FindAllStringSubmatch(scanLine, -1) {
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

		// Invariant references: INV-NNN, APP-INV-NNN, CMP-INV-NNN, etc.
		// Any 2-5 letter uppercase namespace prefix is treated as a namespaced
		// reference; bare INV-NNN remains the legacy "invariant" class.
		for _, m := range XRefInvRe.FindAllStringSubmatch(scanLine, -1) {
			// Skip if this is the definition line itself (invariant header) —
			// covers both the canonical bold form and the CMP h3+em-dash form.
			if InvHeaderRe.MatchString(trimmed) || InvHeaderH3Re.MatchString(trimmed) {
				continue
			}
			refType := "invariant"
			if !strings.HasPrefix(m[1], "INV-") {
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

		// ADR references: ADR-NNN, APP-ADR-NNN, CMP-ADR-NNN, etc.
		for _, m := range XRefADRRe.FindAllStringSubmatch(scanLine, -1) {
			// Skip ADR definition headers
			if ADRHeaderRe.MatchString(trimmed) {
				continue
			}
			refType := "adr"
			if !strings.HasPrefix(m[1], "ADR-") {
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
		for _, m := range XRefGateRe.FindAllStringSubmatch(scanLine, -1) {
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
// After local resolution, attempts to resolve remaining refs against the parent spec.
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
		exists := resolveRefInSpec(db, specID, r.typ, r.target)
		if exists {
			if _, err := db.Exec(
				`UPDATE cross_references SET resolved = 1 WHERE id = ?`, r.id); err != nil {
				return err
			}
		}
	}

	// Parent fallback for still-unresolved refs
	parentID, err := storage.GetParentSpecID(db, specID)
	if err != nil || parentID == nil {
		return nil
	}

	unresolvedRows, err := db.Query(
		`SELECT id, ref_type, ref_target FROM cross_references WHERE spec_id = ? AND resolved = 0`, specID)
	if err != nil {
		return err
	}
	defer unresolvedRows.Close()

	var unresolved []xref
	for unresolvedRows.Next() {
		var r xref
		if err := unresolvedRows.Scan(&r.id, &r.typ, &r.target); err != nil {
			return err
		}
		unresolved = append(unresolved, r)
	}

	for _, r := range unresolved {
		exists := resolveRefInSpec(db, *parentID, r.typ, r.target)
		if exists {
			if _, err := db.Exec(
				`UPDATE cross_references SET resolved = 1 WHERE id = ?`, r.id); err != nil {
				return err
			}
		}
	}

	return nil
}

// resolveRefInSpec checks if a reference target exists in the given spec.
func resolveRefInSpec(db *sql.DB, specID int64, refType, target string) bool {
	switch refType {
	case "section":
		return queryExists(db,
			`SELECT 1 FROM sections WHERE spec_id = ? AND section_path = ?`,
			specID, target)
	case "invariant", "app_invariant":
		return queryExists(db,
			`SELECT 1 FROM invariants WHERE spec_id = ? AND invariant_id = ?`,
			specID, target)
	case "adr", "app_adr":
		return queryExists(db,
			`SELECT 1 FROM adrs WHERE spec_id = ? AND adr_id = ?`,
			specID, target)
	case "gate":
		return queryExists(db,
			`SELECT 1 FROM quality_gates WHERE spec_id = ? AND gate_id = ?`,
			specID, target)
	}
	return false
}

func queryExists(db *sql.DB, query string, args ...interface{}) bool {
	var x int
	err := db.QueryRow(query, args...).Scan(&x)
	return err == nil
}
