package storage

import (
	"database/sql"
	"fmt"
)

// GetFirstSpecID returns the first spec ID in the database.
func GetFirstSpecID(db *sql.DB) (int64, error) {
	var id int64
	err := db.QueryRow("SELECT id FROM spec_index LIMIT 1").Scan(&id)
	if err != nil {
		return 0, fmt.Errorf("get first spec_id: %w", err)
	}
	return id, nil
}

// GetSpecIndex retrieves a spec_index row by ID.
func GetSpecIndex(db *sql.DB, specID int64) (*SpecIndex, error) {
	s := &SpecIndex{}
	var specName, ddisVersion sql.NullString
	err := db.QueryRow(
		`SELECT id, spec_path, spec_name, ddis_version, total_lines, content_hash, parsed_at, source_type
		 FROM spec_index WHERE id = ?`, specID,
	).Scan(&s.ID, &s.SpecPath, &specName, &ddisVersion,
		&s.TotalLines, &s.ContentHash, &s.ParsedAt, &s.SourceType)
	if err != nil {
		return nil, fmt.Errorf("get spec_index %d: %w", specID, err)
	}
	s.SpecName = specName.String
	s.DDISVersion = ddisVersion.String
	return s, nil
}

// GetSection retrieves a section by spec ID and section path.
func GetSection(db *sql.DB, specID int64, sectionPath string) (*Section, error) {
	s := &Section{}
	var parentID sql.NullInt64
	err := db.QueryRow(
		`SELECT id, spec_id, source_file_id, section_path, title, heading_level, parent_id,
		        line_start, line_end, raw_text, content_hash
		 FROM sections WHERE spec_id = ? AND section_path = ?`, specID, sectionPath,
	).Scan(&s.ID, &s.SpecID, &s.SourceFileID, &s.SectionPath, &s.Title, &s.HeadingLevel,
		&parentID, &s.LineStart, &s.LineEnd, &s.RawText, &s.ContentHash)
	if err != nil {
		return nil, fmt.Errorf("get section %s: %w", sectionPath, err)
	}
	if parentID.Valid {
		s.ParentID = &parentID.Int64
	}
	return s, nil
}

// GetInvariant retrieves an invariant by spec ID and invariant ID (e.g. "INV-006").
func GetInvariant(db *sql.DB, specID int64, invariantID string) (*Invariant, error) {
	inv := &Invariant{}
	var semiFormal, violation, validation, whyMatters, conditional sql.NullString
	err := db.QueryRow(
		`SELECT id, spec_id, source_file_id, section_id, invariant_id, title, statement,
		        semi_formal, violation_scenario, validation_method, why_this_matters,
		        conditional_tag, line_start, line_end, raw_text, content_hash
		 FROM invariants WHERE spec_id = ? AND invariant_id = ?`, specID, invariantID,
	).Scan(&inv.ID, &inv.SpecID, &inv.SourceFileID, &inv.SectionID,
		&inv.InvariantID, &inv.Title, &inv.Statement,
		&semiFormal, &violation, &validation, &whyMatters,
		&conditional, &inv.LineStart, &inv.LineEnd, &inv.RawText, &inv.ContentHash)
	if err != nil {
		return nil, fmt.Errorf("get invariant %s: %w", invariantID, err)
	}
	inv.SemiFormal = semiFormal.String
	inv.ViolationScenario = violation.String
	inv.ValidationMethod = validation.String
	inv.WhyThisMatters = whyMatters.String
	inv.ConditionalTag = conditional.String
	return inv, nil
}

// GetADR retrieves an ADR by spec ID and ADR ID (e.g. "ADR-003").
func GetADR(db *sql.DB, specID int64, adrID string) (*ADR, error) {
	a := &ADR{}
	var chosen, consequences, tests, confidence, status, superseded sql.NullString
	err := db.QueryRow(
		`SELECT id, spec_id, source_file_id, section_id, adr_id, title, problem, decision_text,
		        chosen_option, consequences, tests, confidence, status, superseded_by,
		        line_start, line_end, raw_text, content_hash
		 FROM adrs WHERE spec_id = ? AND adr_id = ?`, specID, adrID,
	).Scan(&a.ID, &a.SpecID, &a.SourceFileID, &a.SectionID, &a.ADRID, &a.Title, &a.Problem,
		&a.DecisionText, &chosen, &consequences, &tests, &confidence, &status, &superseded,
		&a.LineStart, &a.LineEnd, &a.RawText, &a.ContentHash)
	if err != nil {
		return nil, fmt.Errorf("get ADR %s: %w", adrID, err)
	}
	a.ChosenOption = chosen.String
	a.Consequences = consequences.String
	a.Tests = tests.String
	a.Confidence = confidence.String
	a.Status = status.String
	a.SupersededBy = superseded.String
	return a, nil
}

// GetQualityGate retrieves a quality gate by spec ID and gate ID (e.g. "Gate-1").
func GetQualityGate(db *sql.DB, specID int64, gateID string) (*QualityGate, error) {
	g := &QualityGate{}
	var isModular int
	err := db.QueryRow(
		`SELECT id, spec_id, section_id, gate_id, title, predicate, is_modular,
		        line_start, line_end, raw_text
		 FROM quality_gates WHERE spec_id = ? AND gate_id = ?`, specID, gateID,
	).Scan(&g.ID, &g.SpecID, &g.SectionID, &g.GateID, &g.Title, &g.Predicate,
		&isModular, &g.LineStart, &g.LineEnd, &g.RawText)
	if err != nil {
		return nil, fmt.Errorf("get gate %s: %w", gateID, err)
	}
	g.IsModular = isModular != 0
	return g, nil
}

// GetADROptions retrieves all options for a given ADR database ID.
func GetADROptions(db *sql.DB, adrDBID int64) ([]ADROption, error) {
	rows, err := db.Query(
		`SELECT id, adr_id, option_label, option_name, pros, cons, is_chosen, why_not
		 FROM adr_options WHERE adr_id = ? ORDER BY option_label`, adrDBID,
	)
	if err != nil {
		return nil, fmt.Errorf("get adr_options: %w", err)
	}
	defer rows.Close()

	var opts []ADROption
	for rows.Next() {
		var o ADROption
		var pros, cons, whyNot sql.NullString
		var isChosen int
		if err := rows.Scan(&o.ID, &o.ADRID, &o.OptionLabel, &o.OptionName,
			&pros, &cons, &isChosen, &whyNot); err != nil {
			return nil, fmt.Errorf("scan adr_option: %w", err)
		}
		o.Pros = pros.String
		o.Cons = cons.String
		o.IsChosen = isChosen != 0
		o.WhyNot = whyNot.String
		opts = append(opts, o)
	}
	return opts, rows.Err()
}

// ListInvariants returns all invariants for a spec.
func ListInvariants(db *sql.DB, specID int64) ([]Invariant, error) {
	rows, err := db.Query(
		`SELECT id, spec_id, source_file_id, section_id, invariant_id, title, statement,
		        semi_formal, violation_scenario, validation_method, why_this_matters,
		        conditional_tag, line_start, line_end, raw_text, content_hash
		 FROM invariants WHERE spec_id = ? ORDER BY invariant_id`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("list invariants: %w", err)
	}
	defer rows.Close()

	var result []Invariant
	for rows.Next() {
		var inv Invariant
		var semiFormal, violation, validation, whyMatters, conditional sql.NullString
		if err := rows.Scan(&inv.ID, &inv.SpecID, &inv.SourceFileID, &inv.SectionID,
			&inv.InvariantID, &inv.Title, &inv.Statement,
			&semiFormal, &violation, &validation, &whyMatters,
			&conditional, &inv.LineStart, &inv.LineEnd, &inv.RawText, &inv.ContentHash); err != nil {
			return nil, fmt.Errorf("scan invariant: %w", err)
		}
		inv.SemiFormal = semiFormal.String
		inv.ViolationScenario = violation.String
		inv.ValidationMethod = validation.String
		inv.WhyThisMatters = whyMatters.String
		inv.ConditionalTag = conditional.String
		result = append(result, inv)
	}
	return result, rows.Err()
}

// ListADRs returns all ADRs for a spec.
func ListADRs(db *sql.DB, specID int64) ([]ADR, error) {
	rows, err := db.Query(
		`SELECT id, spec_id, source_file_id, section_id, adr_id, title, problem, decision_text,
		        chosen_option, consequences, tests, confidence, status, superseded_by,
		        line_start, line_end, raw_text, content_hash
		 FROM adrs WHERE spec_id = ? ORDER BY adr_id`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("list adrs: %w", err)
	}
	defer rows.Close()

	var result []ADR
	for rows.Next() {
		var a ADR
		var chosen, consequences, tests, confidence, status, superseded sql.NullString
		if err := rows.Scan(&a.ID, &a.SpecID, &a.SourceFileID, &a.SectionID,
			&a.ADRID, &a.Title, &a.Problem, &a.DecisionText,
			&chosen, &consequences, &tests, &confidence, &status, &superseded,
			&a.LineStart, &a.LineEnd, &a.RawText, &a.ContentHash); err != nil {
			return nil, fmt.Errorf("scan adr: %w", err)
		}
		a.ChosenOption = chosen.String
		a.Consequences = consequences.String
		a.Tests = tests.String
		a.Confidence = confidence.String
		a.Status = status.String
		a.SupersededBy = superseded.String
		result = append(result, a)
	}
	return result, rows.Err()
}

// ListQualityGates returns all quality gates for a spec.
func ListQualityGates(db *sql.DB, specID int64) ([]QualityGate, error) {
	rows, err := db.Query(
		`SELECT id, spec_id, section_id, gate_id, title, predicate, is_modular,
		        line_start, line_end, raw_text
		 FROM quality_gates WHERE spec_id = ? ORDER BY gate_id`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("list quality_gates: %w", err)
	}
	defer rows.Close()

	var result []QualityGate
	for rows.Next() {
		var g QualityGate
		var isModular int
		if err := rows.Scan(&g.ID, &g.SpecID, &g.SectionID, &g.GateID, &g.Title,
			&g.Predicate, &isModular, &g.LineStart, &g.LineEnd, &g.RawText); err != nil {
			return nil, fmt.Errorf("scan quality_gate: %w", err)
		}
		g.IsModular = isModular != 0
		result = append(result, g)
	}
	return result, rows.Err()
}

// ListSections returns all sections for a spec.
func ListSections(db *sql.DB, specID int64) ([]Section, error) {
	rows, err := db.Query(
		`SELECT id, spec_id, source_file_id, section_path, title, heading_level, parent_id,
		        line_start, line_end, raw_text, content_hash
		 FROM sections WHERE spec_id = ? ORDER BY line_start`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("list sections: %w", err)
	}
	defer rows.Close()

	var result []Section
	for rows.Next() {
		var s Section
		var parentID sql.NullInt64
		if err := rows.Scan(&s.ID, &s.SpecID, &s.SourceFileID, &s.SectionPath, &s.Title,
			&s.HeadingLevel, &parentID, &s.LineStart, &s.LineEnd, &s.RawText, &s.ContentHash); err != nil {
			return nil, fmt.Errorf("scan section: %w", err)
		}
		if parentID.Valid {
			s.ParentID = &parentID.Int64
		}
		result = append(result, s)
	}
	return result, rows.Err()
}

// ListGlossaryEntries returns all glossary entries for a spec.
func ListGlossaryEntries(db *sql.DB, specID int64) ([]GlossaryEntry, error) {
	rows, err := db.Query(
		`SELECT id, spec_id, section_id, term, definition, section_ref, line_number
		 FROM glossary_entries WHERE spec_id = ? ORDER BY term`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("list glossary_entries: %w", err)
	}
	defer rows.Close()

	var result []GlossaryEntry
	for rows.Next() {
		var ge GlossaryEntry
		var sectionRef sql.NullString
		if err := rows.Scan(&ge.ID, &ge.SpecID, &ge.SectionID, &ge.Term, &ge.Definition,
			&sectionRef, &ge.LineNumber); err != nil {
			return nil, fmt.Errorf("scan glossary_entry: %w", err)
		}
		ge.SectionRef = sectionRef.String
		result = append(result, ge)
	}
	return result, rows.Err()
}

// ListModules returns all modules for a spec.
func ListModules(db *sql.DB, specID int64) ([]Module, error) {
	rows, err := db.Query(
		`SELECT id, spec_id, source_file_id, module_name, domain, deep_context_path, line_count
		 FROM modules WHERE spec_id = ? ORDER BY module_name`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("list modules: %w", err)
	}
	defer rows.Close()

	var result []Module
	for rows.Next() {
		var m Module
		var deepCtx sql.NullString
		if err := rows.Scan(&m.ID, &m.SpecID, &m.SourceFileID, &m.ModuleName, &m.Domain,
			&deepCtx, &m.LineCount); err != nil {
			return nil, fmt.Errorf("scan module: %w", err)
		}
		m.DeepContextPath = deepCtx.String
		result = append(result, m)
	}
	return result, rows.Err()
}

// GetSourceFiles returns all source files for a spec.
func GetSourceFiles(db *sql.DB, specID int64) ([]SourceFile, error) {
	rows, err := db.Query(
		`SELECT id, spec_id, file_path, file_role, module_name, content_hash, line_count
		 FROM source_files WHERE spec_id = ? ORDER BY id`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("get source_files: %w", err)
	}
	defer rows.Close()

	var result []SourceFile
	for rows.Next() {
		var sf SourceFile
		var moduleName sql.NullString
		if err := rows.Scan(&sf.ID, &sf.SpecID, &sf.FilePath, &sf.FileRole,
			&moduleName, &sf.ContentHash, &sf.LineCount); err != nil {
			return nil, fmt.Errorf("scan source_file: %w", err)
		}
		sf.ModuleName = moduleName.String
		result = append(result, sf)
	}
	return result, rows.Err()
}

// GetOutgoingRefs returns cross-references originating from a given section.
func GetOutgoingRefs(db *sql.DB, specID int64, sectionID int64) ([]CrossReference, error) {
	rows, err := db.Query(
		`SELECT id, spec_id, source_file_id, source_section_id, source_line,
		        ref_type, ref_target, ref_text, resolved
		 FROM cross_references WHERE spec_id = ? AND source_section_id = ?`, specID, sectionID,
	)
	if err != nil {
		return nil, fmt.Errorf("get outgoing refs: %w", err)
	}
	defer rows.Close()
	return scanCrossRefs(rows)
}

// GetBacklinks returns cross-references targeting a given ref_target string.
func GetBacklinks(db *sql.DB, specID int64, refTarget string) ([]CrossReference, error) {
	rows, err := db.Query(
		`SELECT id, spec_id, source_file_id, source_section_id, source_line,
		        ref_type, ref_target, ref_text, resolved
		 FROM cross_references WHERE spec_id = ? AND ref_target = ?`, specID, refTarget,
	)
	if err != nil {
		return nil, fmt.Errorf("get backlinks for %s: %w", refTarget, err)
	}
	defer rows.Close()
	return scanCrossRefs(rows)
}

// GetUnresolvedRefs returns all unresolved cross-references for a spec.
func GetUnresolvedRefs(db *sql.DB, specID int64) ([]CrossReference, error) {
	rows, err := db.Query(
		`SELECT id, spec_id, source_file_id, source_section_id, source_line,
		        ref_type, ref_target, ref_text, resolved
		 FROM cross_references WHERE spec_id = ? AND resolved = 0`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("get unresolved refs: %w", err)
	}
	defer rows.Close()
	return scanCrossRefs(rows)
}

func scanCrossRefs(rows *sql.Rows) ([]CrossReference, error) {
	var result []CrossReference
	for rows.Next() {
		var xr CrossReference
		var sectionID sql.NullInt64
		var resolved int
		if err := rows.Scan(&xr.ID, &xr.SpecID, &xr.SourceFileID, &sectionID,
			&xr.SourceLine, &xr.RefType, &xr.RefTarget, &xr.RefText, &resolved); err != nil {
			return nil, fmt.Errorf("scan cross_reference: %w", err)
		}
		if sectionID.Valid {
			xr.SourceSectionID = &sectionID.Int64
		}
		xr.Resolved = resolved != 0
		result = append(result, xr)
	}
	return result, rows.Err()
}

// GetSectionRefCounts computes incoming and outgoing cross-reference counts per section.
func GetSectionRefCounts(db *sql.DB, specID int64) (map[int64]RefCounts, error) {
	result := make(map[int64]RefCounts)

	// Outgoing: count refs per source_section_id
	outRows, err := db.Query(
		`SELECT source_section_id, COUNT(*) FROM cross_references
		 WHERE spec_id = ? AND source_section_id IS NOT NULL
		 GROUP BY source_section_id`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("get outgoing ref counts: %w", err)
	}
	defer outRows.Close()

	for outRows.Next() {
		var secID int64
		var count int
		if err := outRows.Scan(&secID, &count); err != nil {
			return nil, fmt.Errorf("scan outgoing: %w", err)
		}
		rc := result[secID]
		rc.Outgoing = count
		result[secID] = rc
	}
	if err := outRows.Err(); err != nil {
		return nil, err
	}

	// Incoming: count refs that target each section's section_path.
	// Also count refs targeting invariants/ADRs/gates that belong to sections.
	inRows, err := db.Query(
		`SELECT s.id, COUNT(*)
		 FROM sections s
		 JOIN cross_references xr ON xr.spec_id = s.spec_id AND xr.ref_target = s.section_path
		 WHERE s.spec_id = ?
		 GROUP BY s.id
		 UNION ALL
		 SELECT i.section_id, COUNT(*)
		 FROM invariants i
		 JOIN cross_references xr ON xr.spec_id = i.spec_id AND xr.ref_target = i.invariant_id
		 WHERE i.spec_id = ?
		 GROUP BY i.section_id
		 UNION ALL
		 SELECT a.section_id, COUNT(*)
		 FROM adrs a
		 JOIN cross_references xr ON xr.spec_id = a.spec_id AND xr.ref_target = a.adr_id
		 WHERE a.spec_id = ?
		 GROUP BY a.section_id
		 UNION ALL
		 SELECT g.section_id, COUNT(*)
		 FROM quality_gates g
		 JOIN cross_references xr ON xr.spec_id = g.spec_id AND xr.ref_target = g.gate_id
		 WHERE g.spec_id = ?
		 GROUP BY g.section_id`, specID, specID, specID, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("get incoming ref counts: %w", err)
	}
	defer inRows.Close()

	for inRows.Next() {
		var secID int64
		var count int
		if err := inRows.Scan(&secID, &count); err != nil {
			return nil, fmt.Errorf("scan incoming: %w", err)
		}
		rc := result[secID]
		rc.Incoming += count
		result[secID] = rc
	}
	return result, inRows.Err()
}

// GetNegativeSpecCountBySection returns negative spec count per section.
func GetNegativeSpecCountBySection(db *sql.DB, specID int64) (map[int64]int, error) {
	rows, err := db.Query(
		`SELECT section_id, COUNT(*) FROM negative_specs WHERE spec_id = ? GROUP BY section_id`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("get neg spec counts: %w", err)
	}
	defer rows.Close()

	result := make(map[int64]int)
	for rows.Next() {
		var secID int64
		var count int
		if err := rows.Scan(&secID, &count); err != nil {
			return nil, fmt.Errorf("scan neg spec count: %w", err)
		}
		result[secID] = count
	}
	return result, rows.Err()
}

// GetModuleRelationships returns all module relationships for a spec.
func GetModuleRelationships(db *sql.DB, specID int64) ([]ModuleRelationship, error) {
	rows, err := db.Query(
		`SELECT mr.id, mr.module_id, mr.rel_type, mr.target
		 FROM module_relationships mr
		 JOIN modules m ON m.id = mr.module_id
		 WHERE m.spec_id = ?`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("get module_relationships: %w", err)
	}
	defer rows.Close()

	var result []ModuleRelationship
	for rows.Next() {
		var mr ModuleRelationship
		if err := rows.Scan(&mr.ID, &mr.ModuleID, &mr.RelType, &mr.Target); err != nil {
			return nil, fmt.Errorf("scan module_relationship: %w", err)
		}
		result = append(result, mr)
	}
	return result, rows.Err()
}

// GetInvariantRegistryEntries returns all invariant registry entries for a spec.
func GetInvariantRegistryEntries(db *sql.DB, specID int64) ([]InvariantRegistryEntry, error) {
	rows, err := db.Query(
		`SELECT id, spec_id, invariant_id, owner, domain, description
		 FROM invariant_registry WHERE spec_id = ? ORDER BY invariant_id`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("get invariant_registry: %w", err)
	}
	defer rows.Close()

	var result []InvariantRegistryEntry
	for rows.Next() {
		var e InvariantRegistryEntry
		if err := rows.Scan(&e.ID, &e.SpecID, &e.InvariantID, &e.Owner, &e.Domain, &e.Description); err != nil {
			return nil, fmt.Errorf("scan invariant_registry: %w", err)
		}
		result = append(result, e)
	}
	return result, rows.Err()
}

// GetManifest retrieves the manifest for a spec, or nil if none exists.
func GetManifest(db *sql.DB, specID int64) (*Manifest, error) {
	m := &Manifest{}
	var ddisVersion, specName, tierMode sql.NullString
	err := db.QueryRow(
		`SELECT id, spec_id, ddis_version, spec_name, tier_mode, target_lines,
		        hard_ceiling_lines, reasoning_reserve, raw_yaml
		 FROM manifest WHERE spec_id = ?`, specID,
	).Scan(&m.ID, &m.SpecID, &ddisVersion, &specName, &tierMode,
		&m.TargetLines, &m.HardCeilingLines, &m.ReasoningReserve, &m.RawYAML)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, fmt.Errorf("get manifest: %w", err)
	}
	m.DDISVersion = ddisVersion.String
	m.SpecName = specName.String
	m.TierMode = tierMode.String
	return m, nil
}

// GetGlossaryTerms returns a set of all glossary terms for a spec (lowercased).
func GetGlossaryTerms(db *sql.DB, specID int64) (map[string]bool, error) {
	rows, err := db.Query(
		`SELECT term FROM glossary_entries WHERE spec_id = ?`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("get glossary terms: %w", err)
	}
	defer rows.Close()

	result := make(map[string]bool)
	for rows.Next() {
		var term string
		if err := rows.Scan(&term); err != nil {
			return nil, fmt.Errorf("scan term: %w", err)
		}
		result[term] = true
	}
	return result, rows.Err()
}

// CountElements returns element counts by table name for a spec.
func CountElements(db *sql.DB, specID int64) (map[string]int, error) {
	tables := []struct {
		name  string
		query string
	}{
		{"sections", "SELECT COUNT(*) FROM sections WHERE spec_id = ?"},
		{"invariants", "SELECT COUNT(*) FROM invariants WHERE spec_id = ?"},
		{"adrs", "SELECT COUNT(*) FROM adrs WHERE spec_id = ?"},
		{"quality_gates", "SELECT COUNT(*) FROM quality_gates WHERE spec_id = ?"},
		{"negative_specs", "SELECT COUNT(*) FROM negative_specs WHERE spec_id = ?"},
		{"verification_prompts", "SELECT COUNT(*) FROM verification_prompts WHERE spec_id = ?"},
		{"meta_instructions", "SELECT COUNT(*) FROM meta_instructions WHERE spec_id = ?"},
		{"worked_examples", "SELECT COUNT(*) FROM worked_examples WHERE spec_id = ?"},
		{"why_not_annotations", "SELECT COUNT(*) FROM why_not_annotations WHERE spec_id = ?"},
		{"comparison_blocks", "SELECT COUNT(*) FROM comparison_blocks WHERE spec_id = ?"},
		{"performance_budgets", "SELECT COUNT(*) FROM performance_budgets WHERE spec_id = ?"},
		{"state_machines", "SELECT COUNT(*) FROM state_machines WHERE spec_id = ?"},
		{"glossary_entries", "SELECT COUNT(*) FROM glossary_entries WHERE spec_id = ?"},
		{"cross_references", "SELECT COUNT(*) FROM cross_references WHERE spec_id = ?"},
		{"cross_references_resolved", "SELECT COUNT(*) FROM cross_references WHERE spec_id = ? AND resolved = 1"},
		{"modules", "SELECT COUNT(*) FROM modules WHERE spec_id = ?"},
	}

	result := make(map[string]int)
	for _, t := range tables {
		var count int
		if err := db.QueryRow(t.query, specID).Scan(&count); err != nil {
			return nil, fmt.Errorf("count %s: %w", t.name, err)
		}
		result[t.name] = count
	}
	return result, nil
}
