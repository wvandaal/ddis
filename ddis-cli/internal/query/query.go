package query

// ddis:implements APP-ADR-011 (structured intent over formal derivation)

import (
	"database/sql"
	"fmt"
	"regexp"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// QueryOptions controls which enrichments to include.
type QueryOptions struct {
	ResolveRefs     bool
	IncludeGlossary bool
	Backlinks       bool
}

var (
	sectionRe = regexp.MustCompile(`^§(\d+(?:\.\d+)*)$`)
	invRe     = regexp.MustCompile(`^((?:APP-)?INV-\d{3})$`)
	adrRe     = regexp.MustCompile(`^((?:APP-)?ADR-\d{3})$`)
	gateRe    = regexp.MustCompile(`^Gate-?((?:M-)?[1-9]\d*)$`)
)

// parseTarget identifies the fragment type and normalizes the ID from user input.
func parseTarget(target string) (FragmentType, string, error) {
	target = strings.TrimSpace(target)

	if m := sectionRe.FindStringSubmatch(target); m != nil {
		return FragmentSection, "§" + m[1], nil
	}
	if m := invRe.FindStringSubmatch(target); m != nil {
		return FragmentInvariant, strings.ToUpper(m[1]), nil
	}
	if m := adrRe.FindStringSubmatch(target); m != nil {
		return FragmentADR, strings.ToUpper(m[1]), nil
	}
	if m := gateRe.FindStringSubmatch(target); m != nil {
		return FragmentGate, "Gate-" + m[1], nil
	}

	// Try as a raw section path (e.g. "PART-0", "Chapter-3", "Appendix-A")
	if strings.HasPrefix(target, "PART-") || strings.HasPrefix(target, "Chapter-") ||
		strings.HasPrefix(target, "Appendix-") {
		return FragmentSection, target, nil
	}

	return "", "", fmt.Errorf("cannot parse target %q: expected §N.M, INV-NNN, ADR-NNN, Gate-N, PART-N, Chapter-N, or Appendix-X", target)
}

// QueryTarget retrieves and assembles a fragment for the given target string.
func QueryTarget(db *sql.DB, specID int64, target string, opts QueryOptions) (*Fragment, error) {
	fragType, normalizedID, err := parseTarget(target)
	if err != nil {
		return nil, err
	}

	f, sectionID, err := assembleFragment(db, specID, fragType, normalizedID)
	if err != nil {
		return nil, err
	}

	if opts.ResolveRefs && sectionID > 0 {
		refs, err := storage.GetOutgoingRefs(db, specID, sectionID)
		if err != nil {
			return nil, fmt.Errorf("resolve refs: %w", err)
		}
		for _, xr := range refs {
			rr := ResolvedRef{
				RefType:  xr.RefType,
				Target:   xr.RefTarget,
				RefText:  xr.RefText,
				Resolved: xr.Resolved,
			}
			// Fetch target raw text for resolved refs
			if xr.Resolved {
				rr.RawText = fetchRefTargetText(db, specID, xr.RefType, xr.RefTarget)
			}
			f.ResolvedRefs = append(f.ResolvedRefs, rr)
		}
	}

	if opts.IncludeGlossary {
		glossaryEntries, err := storage.ListGlossaryEntries(db, specID)
		if err != nil {
			return nil, fmt.Errorf("get glossary: %w", err)
		}
		textLower := strings.ToLower(f.RawText)
		for _, ge := range glossaryEntries {
			if strings.Contains(textLower, strings.ToLower(ge.Term)) {
				f.GlossaryDefs = append(f.GlossaryDefs, GlossaryDef{
					Term:       ge.Term,
					Definition: ge.Definition,
				})
			}
		}
	}

	if opts.Backlinks {
		backlinks, err := storage.GetBacklinks(db, specID, normalizedID)
		if err != nil {
			return nil, fmt.Errorf("get backlinks: %w", err)
		}
		for _, bl := range backlinks {
			b := Backlink{
				SourceLine: bl.SourceLine,
				RefType:    bl.RefType,
				RefText:    bl.RefText,
			}
			if bl.SourceSectionID != nil {
				sec, err := getSectionByID(db, *bl.SourceSectionID)
				if err == nil {
					b.SourceSection = sec.SectionPath
				}
			}
			f.Backlinks = append(f.Backlinks, b)
		}
	}

	return f, nil
}

// assembleFragment builds a Fragment from the database. Returns the fragment and its owning section ID.
func assembleFragment(db *sql.DB, specID int64, fragType FragmentType, id string) (*Fragment, int64, error) {
	switch fragType {
	case FragmentSection:
		sec, err := storage.GetSection(db, specID, id)
		if err != nil {
			return nil, 0, err
		}
		return &Fragment{
			Type:        FragmentSection,
			ID:          sec.SectionPath,
			Title:       sec.Title,
			RawText:     sec.RawText,
			LineStart:   sec.LineStart,
			LineEnd:     sec.LineEnd,
			SectionPath: sec.SectionPath,
		}, sec.ID, nil

	case FragmentInvariant:
		inv, err := storage.GetInvariant(db, specID, id)
		if err != nil {
			return nil, 0, err
		}
		// Look up parent section path
		secPath := ""
		sec, err := getSectionByID(db, inv.SectionID)
		if err == nil {
			secPath = sec.SectionPath
		}
		return &Fragment{
			Type:        FragmentInvariant,
			ID:          inv.InvariantID,
			Title:       inv.Title,
			RawText:     inv.RawText,
			LineStart:   inv.LineStart,
			LineEnd:     inv.LineEnd,
			SectionPath: secPath,
		}, inv.SectionID, nil

	case FragmentADR:
		adr, err := storage.GetADR(db, specID, id)
		if err != nil {
			return nil, 0, err
		}
		secPath := ""
		sec, err := getSectionByID(db, adr.SectionID)
		if err == nil {
			secPath = sec.SectionPath
		}
		return &Fragment{
			Type:        FragmentADR,
			ID:          adr.ADRID,
			Title:       adr.Title,
			RawText:     adr.RawText,
			LineStart:   adr.LineStart,
			LineEnd:     adr.LineEnd,
			SectionPath: secPath,
		}, adr.SectionID, nil

	case FragmentGate:
		gate, err := storage.GetQualityGate(db, specID, id)
		if err != nil {
			return nil, 0, err
		}
		secPath := ""
		sec, err := getSectionByID(db, gate.SectionID)
		if err == nil {
			secPath = sec.SectionPath
		}
		return &Fragment{
			Type:        FragmentGate,
			ID:          gate.GateID,
			Title:       gate.Title,
			RawText:     gate.RawText,
			LineStart:   gate.LineStart,
			LineEnd:     gate.LineEnd,
			SectionPath: secPath,
		}, gate.SectionID, nil

	default:
		return nil, 0, fmt.Errorf("unknown fragment type: %s", fragType)
	}
}

// getSectionByID retrieves a section by its database row ID.
func getSectionByID(db *sql.DB, sectionID int64) (*storage.Section, error) {
	s := &storage.Section{}
	var parentID sql.NullInt64
	err := db.QueryRow(
		`SELECT id, spec_id, source_file_id, section_path, title, heading_level, parent_id,
		        line_start, line_end, raw_text, content_hash
		 FROM sections WHERE id = ?`, sectionID,
	).Scan(&s.ID, &s.SpecID, &s.SourceFileID, &s.SectionPath, &s.Title, &s.HeadingLevel,
		&parentID, &s.LineStart, &s.LineEnd, &s.RawText, &s.ContentHash)
	if err != nil {
		return nil, err
	}
	if parentID.Valid {
		s.ParentID = &parentID.Int64
	}
	return s, nil
}

// fetchRefTargetText retrieves raw_text for a cross-reference target.
func fetchRefTargetText(db *sql.DB, specID int64, refType, refTarget string) string {
	var rawText string
	var err error

	switch refType {
	case "section":
		err = db.QueryRow(
			"SELECT raw_text FROM sections WHERE spec_id = ? AND section_path = ?",
			specID, refTarget).Scan(&rawText)
	case "invariant", "app_invariant":
		err = db.QueryRow(
			"SELECT raw_text FROM invariants WHERE spec_id = ? AND invariant_id = ?",
			specID, refTarget).Scan(&rawText)
	case "adr", "app_adr":
		err = db.QueryRow(
			"SELECT raw_text FROM adrs WHERE spec_id = ? AND adr_id = ?",
			specID, refTarget).Scan(&rawText)
	case "gate":
		err = db.QueryRow(
			"SELECT raw_text FROM quality_gates WHERE spec_id = ? AND gate_id = ?",
			specID, refTarget).Scan(&rawText)
	}

	if err != nil {
		return ""
	}
	return rawText
}
