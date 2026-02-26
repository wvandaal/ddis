package storage

// ddis:maintains APP-INV-041 (witness auto-invalidation — InsertWitness, InvalidateWitnesses)

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
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		 ON CONFLICT(spec_id, adr_id) DO UPDATE SET
		  source_file_id = CASE WHEN length(excluded.raw_text) > length(adrs.raw_text) THEN excluded.source_file_id ELSE adrs.source_file_id END,
		  section_id = CASE WHEN length(excluded.raw_text) > length(adrs.raw_text) THEN excluded.section_id ELSE adrs.section_id END,
		  title = CASE WHEN length(excluded.title) > length(adrs.title) THEN excluded.title ELSE adrs.title END,
		  problem = CASE WHEN length(excluded.problem) > length(COALESCE(adrs.problem,'')) THEN excluded.problem ELSE adrs.problem END,
		  decision_text = CASE WHEN length(excluded.decision_text) > length(COALESCE(adrs.decision_text,'')) THEN excluded.decision_text ELSE adrs.decision_text END,
		  chosen_option = CASE WHEN excluded.chosen_option IS NOT NULL AND (adrs.chosen_option IS NULL OR length(excluded.chosen_option) > length(adrs.chosen_option)) THEN excluded.chosen_option ELSE adrs.chosen_option END,
		  consequences = CASE WHEN excluded.consequences IS NOT NULL AND (adrs.consequences IS NULL OR length(excluded.consequences) > length(adrs.consequences)) THEN excluded.consequences ELSE adrs.consequences END,
		  tests = CASE WHEN excluded.tests IS NOT NULL AND (adrs.tests IS NULL OR length(excluded.tests) > length(adrs.tests)) THEN excluded.tests ELSE adrs.tests END,
		  confidence = CASE WHEN excluded.confidence IS NOT NULL THEN excluded.confidence ELSE adrs.confidence END,
		  line_start = CASE WHEN length(excluded.raw_text) > length(adrs.raw_text) THEN excluded.line_start ELSE adrs.line_start END,
		  line_end = CASE WHEN length(excluded.raw_text) > length(adrs.raw_text) THEN excluded.line_end ELSE adrs.line_end END,
		  raw_text = CASE WHEN length(excluded.raw_text) > length(adrs.raw_text) THEN excluded.raw_text ELSE adrs.raw_text END,
		  content_hash = CASE WHEN length(excluded.raw_text) > length(adrs.raw_text) THEN excluded.content_hash ELSE adrs.content_hash END`,
		a.SpecID, a.SourceFileID, a.SectionID, a.ADRID, a.Title, a.Problem, a.DecisionText,
		nullStr(a.ChosenOption), nullStr(a.Consequences), nullStr(a.Tests),
		nullStr(a.Confidence), a.Status, nullStr(a.SupersededBy),
		a.LineStart, a.LineEnd, a.RawText, a.ContentHash,
	)
	if err != nil {
		return 0, fmt.Errorf("insert ADR %s: %w", a.ADRID, err)
	}
	_ = res
	// ON CONFLICT DO UPDATE does not update last_insert_rowid() in SQLite,
	// so LastInsertId() returns a stale value. Always query back the actual
	// row ID for correct FK references in adr_options.
	var id int64
	err = db.QueryRow(`SELECT id FROM adrs WHERE spec_id = ? AND adr_id = ?`, a.SpecID, a.ADRID).Scan(&id)
	if err != nil {
		return 0, fmt.Errorf("get ADR ID after upsert %s: %w", a.ADRID, err)
	}
	return id, nil
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

// InsertFTSEntry inserts a row into the FTS5 index.
func InsertFTSEntry(db *sql.DB, elementType, elementID, title, content string) error {
	_, err := db.Exec(
		`INSERT INTO fts_index (rowid, element_type, element_id, title, content)
		 VALUES (NULL, ?, ?, ?, ?)`,
		elementType, elementID, title, content,
	)
	if err != nil {
		return fmt.Errorf("insert fts_index: %w", err)
	}
	return nil
}

// InsertSearchVector inserts an LSI vector for a document.
func InsertSearchVector(db *sql.DB, specID int64, elementType, elementID string, vector []byte) error {
	_, err := db.Exec(
		`INSERT OR REPLACE INTO search_vectors (spec_id, element_type, element_id, vector)
		 VALUES (?, ?, ?, ?)`,
		specID, elementType, elementID, vector,
	)
	if err != nil {
		return fmt.Errorf("insert search_vector: %w", err)
	}
	return nil
}

// InsertSearchModel inserts a serialized search model.
func InsertSearchModel(db *sql.DB, specID int64, modelType string, k, terms, docs int, data []byte) error {
	_, err := db.Exec(
		`INSERT INTO search_model (spec_id, model_type, k_dimensions, term_count, doc_count, built_at, model_data)
		 VALUES (?, ?, ?, ?, ?, datetime('now'), ?)`,
		specID, modelType, k, terms, docs, data,
	)
	if err != nil {
		return fmt.Errorf("insert search_model: %w", err)
	}
	return nil
}

// InsertAuthority inserts a PageRank authority score.
func InsertAuthority(db *sql.DB, specID int64, elementID string, score float64) error {
	_, err := db.Exec(
		`INSERT OR REPLACE INTO search_authority (spec_id, element_id, score)
		 VALUES (?, ?, ?)`,
		specID, elementID, score,
	)
	if err != nil {
		return fmt.Errorf("insert search_authority: %w", err)
	}
	return nil
}

// ClearFTSIndex removes all rows from the FTS5 index.
// Uses the FTS5 'delete-all' command because fts_index is a contentless table.
func ClearFTSIndex(db *sql.DB) error {
	_, err := db.Exec(`INSERT INTO fts_index(fts_index) VALUES('delete-all')`)
	return err
}

// ClearSearchData removes all search-related data for a spec.
func ClearSearchData(db *sql.DB, specID int64) error {
	if _, err := db.Exec(`DELETE FROM search_vectors WHERE spec_id = ?`, specID); err != nil {
		return err
	}
	if _, err := db.Exec(`DELETE FROM search_model WHERE spec_id = ?`, specID); err != nil {
		return err
	}
	if _, err := db.Exec(`DELETE FROM search_authority WHERE spec_id = ?`, specID); err != nil {
		return err
	}
	return nil
}

// InsertWitness inserts or replaces an invariant witness.
// Clears any challenge_results referencing the old witness to avoid FK violations
// during INSERT OR REPLACE (which DELETEs then INSERTs).
func InsertWitness(db *sql.DB, w *InvariantWitness) (int64, error) {
	// Remove stale challenge results that reference the witness being replaced.
	if _, err := db.Exec(
		`DELETE FROM challenge_results WHERE spec_id = ? AND invariant_id = ?`,
		w.SpecID, w.InvariantID,
	); err != nil {
		return 0, fmt.Errorf("clear challenge for witness %s: %w", w.InvariantID, err)
	}

	res, err := db.Exec(
		`INSERT OR REPLACE INTO invariant_witnesses
		 (spec_id, invariant_id, spec_hash, code_hash, evidence_type, evidence,
		  proven_by, model, proven_at, status, notes)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), ?, ?)`,
		w.SpecID, w.InvariantID, w.SpecHash, nullStr(w.CodeHash),
		w.EvidenceType, w.Evidence, w.ProvenBy, nullStr(w.Model),
		w.Status, nullStr(w.Notes),
	)
	if err != nil {
		return 0, fmt.Errorf("insert witness %s: %w", w.InvariantID, err)
	}
	return res.LastInsertId()
}

// InsertChallengeResult inserts or replaces a challenge result.
func InsertChallengeResult(db DB, cr *ChallengeResult) (int64, error) {
	res, err := db.Exec(
		`INSERT OR REPLACE INTO challenge_results
		 (spec_id, invariant_id, witness_id, verdict,
		  level_formal, level_uncertainty, level_causal, level_practical, level_meta,
		  challenged_at, challenged_by, model)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), ?, ?)`,
		cr.SpecID, cr.InvariantID, cr.WitnessID, cr.Verdict,
		nullStr(cr.LevelFormal), nullStr(cr.LevelUncertainty),
		nullStr(cr.LevelCausal), nullStr(cr.LevelPractical), nullStr(cr.LevelMeta),
		cr.ChallengedBy, nullStr(cr.Model),
	)
	if err != nil {
		return 0, fmt.Errorf("insert challenge result %s: %w", cr.InvariantID, err)
	}
	return res.LastInsertId()
}

// InvalidateWitnessByID sets a specific witness status to 'invalidated'.
func InvalidateWitnessByID(db DB, witnessID int64) error {
	res, err := db.Exec(
		`UPDATE invariant_witnesses SET status = 'invalidated' WHERE id = ?`,
		witnessID,
	)
	if err != nil {
		return fmt.Errorf("invalidate witness %d: %w", witnessID, err)
	}
	n, _ := res.RowsAffected()
	if n == 0 {
		return fmt.Errorf("no witness found with id %d", witnessID)
	}
	return nil
}

// InvalidateWitnesses marks witnesses as stale_spec where the spec hash no longer
// matches the latest parsed invariant content_hash.
//
// witnessSpecID is the spec_id where witnesses are stored (typically GetFirstSpecID).
// currentSpecID is the spec_id of the most recently parsed invariants (the new parse).
// If they are the same, the query joins on the same spec_id. If different (common
// when re-parsing creates a new spec_index row), it compares witness hashes against
// the freshly-parsed invariants.
func InvalidateWitnesses(db DB, witnessSpecID, currentSpecID int64) (int, error) {
	res, err := db.Exec(
		`UPDATE invariant_witnesses SET status = 'stale_spec'
		 WHERE spec_id = ? AND status = 'valid'
		 AND NOT EXISTS (
		     SELECT 1 FROM invariants inv
		     WHERE inv.spec_id = ?
		     AND inv.invariant_id = invariant_witnesses.invariant_id
		     AND inv.content_hash = invariant_witnesses.spec_hash
		 )`, witnessSpecID, currentSpecID,
	)
	if err != nil {
		return 0, fmt.Errorf("invalidate witnesses: %w", err)
	}
	n, _ := res.RowsAffected()
	return int(n), nil
}

// ClearSpecByPath removes all data for existing specs with the given path,
// making re-parsing idempotent. Returns saved witnesses so they can be
// re-attached to the freshly-parsed spec after insertion.
//
// Deletion order respects FK constraints (deepest children first).
func ClearSpecByPath(db DB, specPath string) ([]InvariantWitness, error) {
	// Find all spec_ids for this path
	rows, err := db.Query(`SELECT id FROM spec_index WHERE spec_path = ?`, specPath)
	if err != nil {
		return nil, fmt.Errorf("find specs by path: %w", err)
	}
	var specIDs []int64
	for rows.Next() {
		var id int64
		if err := rows.Scan(&id); err != nil {
			rows.Close()
			return nil, err
		}
		specIDs = append(specIDs, id)
	}
	rows.Close()

	if len(specIDs) == 0 {
		return nil, nil
	}

	// Save witnesses before deletion
	var savedWitnesses []InvariantWitness
	for _, sid := range specIDs {
		ws, err := ListWitnesses(db, sid)
		if err == nil {
			savedWitnesses = append(savedWitnesses, ws...)
		}
	}

	for _, sid := range specIDs {
		if err := deleteSpecData(db, sid); err != nil {
			return savedWitnesses, fmt.Errorf("delete spec %d: %w", sid, err)
		}
	}

	return savedWitnesses, nil
}

// deleteSpecData removes all data for a single specID in FK-safe order.
// Tables are deleted leaf-first respecting all foreign key constraints.
func deleteSpecData(db DB, specID int64) error {
	// Temporarily disable FK checks for bulk deletion, then re-enable.
	// This is safe because we're deleting ALL data for a specID (complete graph removal).
	if _, err := db.Exec(`PRAGMA foreign_keys=OFF`); err != nil {
		return fmt.Errorf("disable FK: %w", err)
	}

	// Delete all tables that reference spec_id
	tables := []string{
		"adr_options",          // FK → adrs
		"verification_checks",  // FK → verification_prompts
		"budget_entries",       // FK → performance_budgets
		"state_machine_cells",  // FK → state_machines
		"module_relationships", // FK → modules
		"module_negative_specs", // FK → modules
		"tx_operations",        // FK → transactions
		"cross_references",
		"negative_specs",
		"formatting_hints",
		"invariants",
		"adrs",
		"quality_gates",
		"verification_prompts",
		"meta_instructions",
		"worked_examples",
		"why_not_annotations",
		"comparison_blocks",
		"performance_budgets",
		"state_machines",
		"glossary_entries",
		"modules",
		"manifest",
		"invariant_registry",
		"transactions",
		"sections",
		"source_files",
		"search_vectors",
		"search_model",
		"search_authority",
		"session_state",
		"code_annotations",
		"challenge_results",
		"invariant_witnesses",
	}

	for _, table := range tables {
		var q string
		switch table {
		case "adr_options":
			q = `DELETE FROM adr_options WHERE adr_id IN (SELECT id FROM adrs WHERE spec_id = ?)`
		case "verification_checks":
			q = `DELETE FROM verification_checks WHERE prompt_id IN (SELECT id FROM verification_prompts WHERE spec_id = ?)`
		case "budget_entries":
			q = `DELETE FROM budget_entries WHERE budget_id IN (SELECT id FROM performance_budgets WHERE spec_id = ?)`
		case "state_machine_cells":
			q = `DELETE FROM state_machine_cells WHERE machine_id IN (SELECT id FROM state_machines WHERE spec_id = ?)`
		case "module_relationships":
			q = `DELETE FROM module_relationships WHERE module_id IN (SELECT id FROM modules WHERE spec_id = ?)`
		case "module_negative_specs":
			q = `DELETE FROM module_negative_specs WHERE module_id IN (SELECT id FROM modules WHERE spec_id = ?)`
		case "tx_operations":
			q = `DELETE FROM tx_operations WHERE tx_id IN (SELECT tx_id FROM transactions WHERE spec_id = ?)`
		default:
			q = fmt.Sprintf("DELETE FROM %s WHERE spec_id = ?", table)
		}
		if _, err := db.Exec(q, specID); err != nil {
			// Re-enable FK before returning error
			db.Exec(`PRAGMA foreign_keys=ON`)
			return fmt.Errorf("delete %s: %w", table, err)
		}
	}

	// Clear parent_spec_id references pointing to this spec
	if _, err := db.Exec(`UPDATE spec_index SET parent_spec_id = NULL WHERE parent_spec_id = ?`, specID); err != nil {
		db.Exec(`PRAGMA foreign_keys=ON`)
		return err
	}

	// Delete spec_index row
	if _, err := db.Exec(`DELETE FROM spec_index WHERE id = ?`, specID); err != nil {
		db.Exec(`PRAGMA foreign_keys=ON`)
		return err
	}

	// Re-enable foreign keys
	if _, err := db.Exec(`PRAGMA foreign_keys=ON`); err != nil {
		return fmt.Errorf("re-enable FK: %w", err)
	}

	return nil
}

// nullStr converts an empty string to a sql.NullString.
func nullStr(s string) interface{} {
	if s == "" {
		return nil
	}
	return s
}
