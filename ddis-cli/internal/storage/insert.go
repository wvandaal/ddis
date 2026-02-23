package storage

import (
	"database/sql"
	"fmt"
)

// InsertSpecIndex inserts a new spec_index row and returns its ID.
func InsertSpecIndex(db *sql.DB, s *SpecIndex) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO spec_index (spec_path, spec_name, ddis_version, total_lines, content_hash, parsed_at, source_type)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		s.SpecPath, nullStr(s.SpecName), nullStr(s.DDISVersion),
		s.TotalLines, s.ContentHash, s.ParsedAt, s.SourceType,
	)
	if err != nil {
		return 0, fmt.Errorf("insert spec_index: %w", err)
	}
	return res.LastInsertId()
}

// InsertSourceFile inserts a source_files row and returns its ID.
func InsertSourceFile(db *sql.DB, sf *SourceFile) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO source_files (spec_id, file_path, file_role, module_name, content_hash, line_count, raw_text)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		sf.SpecID, sf.FilePath, sf.FileRole, nullStr(sf.ModuleName),
		sf.ContentHash, sf.LineCount, sf.RawText,
	)
	if err != nil {
		return 0, fmt.Errorf("insert source_files: %w", err)
	}
	return res.LastInsertId()
}

// InsertSection inserts a sections row and returns its ID.
func InsertSection(db *sql.DB, s *Section) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO sections (spec_id, source_file_id, section_path, title, heading_level, parent_id, line_start, line_end, raw_text, content_hash)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		s.SpecID, s.SourceFileID, s.SectionPath, s.Title, s.HeadingLevel,
		s.ParentID, s.LineStart, s.LineEnd, s.RawText, s.ContentHash,
	)
	if err != nil {
		return 0, fmt.Errorf("insert sections: %w", err)
	}
	return res.LastInsertId()
}

// InsertInvariant inserts an invariants row and returns its ID.
func InsertInvariant(db *sql.DB, inv *Invariant) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO invariants (spec_id, source_file_id, section_id, invariant_id, title, statement,
		  semi_formal, violation_scenario, validation_method, why_this_matters, conditional_tag,
		  line_start, line_end, raw_text, content_hash)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		inv.SpecID, inv.SourceFileID, inv.SectionID, inv.InvariantID, inv.Title, inv.Statement,
		nullStr(inv.SemiFormal), nullStr(inv.ViolationScenario), nullStr(inv.ValidationMethod),
		nullStr(inv.WhyThisMatters), nullStr(inv.ConditionalTag),
		inv.LineStart, inv.LineEnd, inv.RawText, inv.ContentHash,
	)
	if err != nil {
		return 0, fmt.Errorf("insert invariant %s: %w", inv.InvariantID, err)
	}
	return res.LastInsertId()
}

// InsertADR inserts an adrs row and returns its ID.
func InsertADR(db *sql.DB, a *ADR) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO adrs (spec_id, source_file_id, section_id, adr_id, title, problem, decision_text,
		  chosen_option, consequences, tests, confidence, status, superseded_by,
		  line_start, line_end, raw_text, content_hash)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		a.SpecID, a.SourceFileID, a.SectionID, a.ADRID, a.Title, a.Problem, a.DecisionText,
		nullStr(a.ChosenOption), nullStr(a.Consequences), nullStr(a.Tests),
		nullStr(a.Confidence), a.Status, nullStr(a.SupersededBy),
		a.LineStart, a.LineEnd, a.RawText, a.ContentHash,
	)
	if err != nil {
		return 0, fmt.Errorf("insert ADR %s: %w", a.ADRID, err)
	}
	return res.LastInsertId()
}

// InsertADROption inserts an adr_options row.
func InsertADROption(db *sql.DB, o *ADROption) (int64, error) {
	chosen := 0
	if o.IsChosen {
		chosen = 1
	}
	res, err := db.Exec(
		`INSERT INTO adr_options (adr_id, option_label, option_name, pros, cons, is_chosen, why_not)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		o.ADRID, o.OptionLabel, o.OptionName, nullStr(o.Pros), nullStr(o.Cons),
		chosen, nullStr(o.WhyNot),
	)
	if err != nil {
		return 0, fmt.Errorf("insert adr_option: %w", err)
	}
	return res.LastInsertId()
}

// InsertQualityGate inserts a quality_gates row.
func InsertQualityGate(db *sql.DB, g *QualityGate) (int64, error) {
	modular := 0
	if g.IsModular {
		modular = 1
	}
	res, err := db.Exec(
		`INSERT INTO quality_gates (spec_id, section_id, gate_id, title, predicate, is_modular, line_start, line_end, raw_text)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		g.SpecID, g.SectionID, g.GateID, g.Title, g.Predicate, modular,
		g.LineStart, g.LineEnd, g.RawText,
	)
	if err != nil {
		return 0, fmt.Errorf("insert gate %s: %w", g.GateID, err)
	}
	return res.LastInsertId()
}

// InsertNegativeSpec inserts a negative_specs row.
func InsertNegativeSpec(db *sql.DB, ns *NegativeSpec) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO negative_specs (spec_id, source_file_id, section_id, constraint_text, reason, invariant_ref, line_number, raw_text)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
		ns.SpecID, ns.SourceFileID, ns.SectionID, ns.ConstraintText,
		nullStr(ns.Reason), nullStr(ns.InvariantRef), ns.LineNumber, ns.RawText,
	)
	if err != nil {
		return 0, fmt.Errorf("insert negative_spec: %w", err)
	}
	return res.LastInsertId()
}

// InsertVerificationPrompt inserts a verification_prompts row.
func InsertVerificationPrompt(db *sql.DB, vp *VerificationPrompt) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO verification_prompts (spec_id, section_id, chapter_name, line_start, line_end, raw_text)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		vp.SpecID, vp.SectionID, vp.ChapterName, vp.LineStart, vp.LineEnd, vp.RawText,
	)
	if err != nil {
		return 0, fmt.Errorf("insert verification_prompt: %w", err)
	}
	return res.LastInsertId()
}

// InsertVerificationCheck inserts a verification_checks row.
func InsertVerificationCheck(db *sql.DB, vc *VerificationCheck) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO verification_checks (prompt_id, check_type, check_text, invariant_ref, ordinal)
		 VALUES (?, ?, ?, ?, ?)`,
		vc.PromptID, vc.CheckType, vc.CheckText, nullStr(vc.InvariantRef), vc.Ordinal,
	)
	if err != nil {
		return 0, fmt.Errorf("insert verification_check: %w", err)
	}
	return res.LastInsertId()
}

// InsertMetaInstruction inserts a meta_instructions row.
func InsertMetaInstruction(db *sql.DB, mi *MetaInstruction) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO meta_instructions (spec_id, section_id, directive, reason, line_start, line_end, raw_text)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		mi.SpecID, mi.SectionID, mi.Directive, nullStr(mi.Reason),
		mi.LineStart, mi.LineEnd, mi.RawText,
	)
	if err != nil {
		return 0, fmt.Errorf("insert meta_instruction: %w", err)
	}
	return res.LastInsertId()
}

// InsertWorkedExample inserts a worked_examples row.
func InsertWorkedExample(db *sql.DB, we *WorkedExample) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO worked_examples (spec_id, section_id, title, line_start, line_end, raw_text)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		we.SpecID, we.SectionID, nullStr(we.Title), we.LineStart, we.LineEnd, we.RawText,
	)
	if err != nil {
		return 0, fmt.Errorf("insert worked_example: %w", err)
	}
	return res.LastInsertId()
}

// InsertWhyNotAnnotation inserts a why_not_annotations row.
func InsertWhyNotAnnotation(db *sql.DB, wn *WhyNotAnnotation) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO why_not_annotations (spec_id, section_id, alternative, explanation, adr_ref, line_number, raw_text)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		wn.SpecID, wn.SectionID, wn.Alternative, wn.Explanation,
		nullStr(wn.ADRRef), wn.LineNumber, wn.RawText,
	)
	if err != nil {
		return 0, fmt.Errorf("insert why_not_annotation: %w", err)
	}
	return res.LastInsertId()
}

// InsertComparisonBlock inserts a comparison_blocks row.
func InsertComparisonBlock(db *sql.DB, cb *ComparisonBlock) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO comparison_blocks (spec_id, section_id, suboptimal_approach, chosen_approach,
		  suboptimal_reasons, chosen_reasons, adr_ref, line_start, line_end, raw_text)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		cb.SpecID, cb.SectionID, cb.SuboptimalApproach, cb.ChosenApproach,
		nullStr(cb.SuboptimalReasons), nullStr(cb.ChosenReasons),
		nullStr(cb.ADRRef), cb.LineStart, cb.LineEnd, cb.RawText,
	)
	if err != nil {
		return 0, fmt.Errorf("insert comparison_block: %w", err)
	}
	return res.LastInsertId()
}

// InsertPerformanceBudget inserts a performance_budgets row.
func InsertPerformanceBudget(db *sql.DB, pb *PerformanceBudget) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO performance_budgets (spec_id, section_id, design_point, line_start, line_end, raw_text)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		pb.SpecID, pb.SectionID, nullStr(pb.DesignPoint), pb.LineStart, pb.LineEnd, pb.RawText,
	)
	if err != nil {
		return 0, fmt.Errorf("insert performance_budget: %w", err)
	}
	return res.LastInsertId()
}

// InsertBudgetEntry inserts a budget_entries row.
func InsertBudgetEntry(db *sql.DB, be *BudgetEntry) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO budget_entries (budget_id, metric_id, operation, target, measurement_method, ordinal)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		be.BudgetID, nullStr(be.MetricID), be.Operation, be.Target,
		nullStr(be.MeasurementMethod), be.Ordinal,
	)
	if err != nil {
		return 0, fmt.Errorf("insert budget_entry: %w", err)
	}
	return res.LastInsertId()
}

// InsertStateMachine inserts a state_machines row.
func InsertStateMachine(db *sql.DB, sm *StateMachine) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO state_machines (spec_id, section_id, title, line_start, line_end, raw_text)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		sm.SpecID, sm.SectionID, nullStr(sm.Title), sm.LineStart, sm.LineEnd, sm.RawText,
	)
	if err != nil {
		return 0, fmt.Errorf("insert state_machine: %w", err)
	}
	return res.LastInsertId()
}

// InsertStateMachineCell inserts a state_machine_cells row.
func InsertStateMachineCell(db *sql.DB, c *StateMachineCell) (int64, error) {
	invalid := 0
	if c.IsInvalid {
		invalid = 1
	}
	res, err := db.Exec(
		`INSERT INTO state_machine_cells (machine_id, state_name, event_name, transition, guard, is_invalid)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		c.MachineID, c.StateName, c.EventName, c.Transition,
		nullStr(c.Guard), invalid,
	)
	if err != nil {
		return 0, fmt.Errorf("insert state_machine_cell: %w", err)
	}
	return res.LastInsertId()
}

// InsertGlossaryEntry inserts a glossary_entries row.
func InsertGlossaryEntry(db *sql.DB, ge *GlossaryEntry) (int64, error) {
	res, err := db.Exec(
		`INSERT OR IGNORE INTO glossary_entries (spec_id, section_id, term, definition, section_ref, line_number)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		ge.SpecID, ge.SectionID, ge.Term, ge.Definition,
		nullStr(ge.SectionRef), ge.LineNumber,
	)
	if err != nil {
		return 0, fmt.Errorf("insert glossary_entry: %w", err)
	}
	return res.LastInsertId()
}

// InsertCrossReference inserts a cross_references row.
func InsertCrossReference(db *sql.DB, xr *CrossReference) (int64, error) {
	resolved := 0
	if xr.Resolved {
		resolved = 1
	}
	res, err := db.Exec(
		`INSERT INTO cross_references (spec_id, source_file_id, source_section_id, source_line, ref_type, ref_target, ref_text, resolved)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
		xr.SpecID, xr.SourceFileID, xr.SourceSectionID, xr.SourceLine,
		xr.RefType, xr.RefTarget, xr.RefText, resolved,
	)
	if err != nil {
		return 0, fmt.Errorf("insert cross_reference: %w", err)
	}
	return res.LastInsertId()
}

// InsertModule inserts a modules row.
func InsertModule(db *sql.DB, m *Module) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO modules (spec_id, source_file_id, module_name, domain, deep_context_path, line_count)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		m.SpecID, m.SourceFileID, m.ModuleName, m.Domain,
		nullStr(m.DeepContextPath), m.LineCount,
	)
	if err != nil {
		return 0, fmt.Errorf("insert module %s: %w", m.ModuleName, err)
	}
	return res.LastInsertId()
}

// InsertModuleRelationship inserts a module_relationships row.
func InsertModuleRelationship(db *sql.DB, mr *ModuleRelationship) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO module_relationships (module_id, rel_type, target) VALUES (?, ?, ?)`,
		mr.ModuleID, mr.RelType, mr.Target,
	)
	if err != nil {
		return 0, fmt.Errorf("insert module_relationship: %w", err)
	}
	return res.LastInsertId()
}

// InsertModuleNegativeSpec inserts a module_negative_specs row.
func InsertModuleNegativeSpec(db *sql.DB, mns *ModuleNegativeSpec) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO module_negative_specs (module_id, constraint_text) VALUES (?, ?)`,
		mns.ModuleID, mns.ConstraintText,
	)
	if err != nil {
		return 0, fmt.Errorf("insert module_negative_spec: %w", err)
	}
	return res.LastInsertId()
}

// InsertManifest inserts a manifest row.
func InsertManifest(db *sql.DB, m *Manifest) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO manifest (spec_id, ddis_version, spec_name, tier_mode, target_lines, hard_ceiling_lines, reasoning_reserve, raw_yaml)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
		m.SpecID, nullStr(m.DDISVersion), nullStr(m.SpecName), nullStr(m.TierMode),
		m.TargetLines, m.HardCeilingLines, m.ReasoningReserve, m.RawYAML,
	)
	if err != nil {
		return 0, fmt.Errorf("insert manifest: %w", err)
	}
	return res.LastInsertId()
}

// InsertInvariantRegistryEntry inserts an invariant_registry row.
func InsertInvariantRegistryEntry(db *sql.DB, e *InvariantRegistryEntry) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO invariant_registry (spec_id, invariant_id, owner, domain, description)
		 VALUES (?, ?, ?, ?, ?)`,
		e.SpecID, e.InvariantID, e.Owner, e.Domain, e.Description,
	)
	if err != nil {
		return 0, fmt.Errorf("insert invariant_registry: %w", err)
	}
	return res.LastInsertId()
}

// InsertFormattingHint inserts a formatting_hints row.
func InsertFormattingHint(db *sql.DB, fh *FormattingHint) (int64, error) {
	res, err := db.Exec(
		`INSERT INTO formatting_hints (spec_id, source_file_id, line_number, hint_type, hint_value)
		 VALUES (?, ?, ?, ?, ?)`,
		fh.SpecID, fh.SourceFileID, fh.LineNumber, fh.HintType, nullStr(fh.HintValue),
	)
	if err != nil {
		return 0, fmt.Errorf("insert formatting_hint: %w", err)
	}
	return res.LastInsertId()
}

// nullStr converts an empty string to a sql.NullString.
func nullStr(s string) interface{} {
	if s == "" {
		return nil
	}
	return s
}
