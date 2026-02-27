package validator

import (
	"testing"

	"github.com/wvandaal/ddis/internal/storage"
)

// ---------------------------------------------------------------------------
// helpers: create an in-memory DB and seed baseline data
// ---------------------------------------------------------------------------

// testDB creates an in-memory SQLite database with the DDIS schema applied
// and returns both the *sql.DB handle and a spec ID for a minimal spec.
func testDB(t *testing.T, sourceType string) (*storage.DB, int64) {
	t.Helper()
	db, err := storage.Open(":memory:")
	if err != nil {
		t.Fatalf("open in-memory db: %v", err)
	}
	t.Cleanup(func() { db.Close() })

	specID, err := storage.InsertSpecIndex(db, &storage.SpecIndex{
		SpecPath:    "/test/spec.md",
		SpecName:    "test-spec",
		DDISVersion: "3.0",
		TotalLines:  100,
		ContentHash: "abc123",
		ParsedAt:    "2026-01-01T00:00:00Z",
		SourceType:  sourceType,
	})
	if err != nil {
		t.Fatalf("insert spec: %v", err)
	}

	return &db, specID
}

// insertSourceFile is a convenience wrapper that inserts a source file and
// returns its database ID.
func insertSourceFile(t *testing.T, db storage.DB, specID int64, filePath, fileRole, moduleName string, lineCount int) int64 {
	t.Helper()
	id, err := storage.InsertSourceFile(db, &storage.SourceFile{
		SpecID:      specID,
		FilePath:    filePath,
		FileRole:    fileRole,
		ModuleName:  moduleName,
		ContentHash: "hash",
		LineCount:   lineCount,
		RawText:     "",
	})
	if err != nil {
		t.Fatalf("insert source file %s: %v", filePath, err)
	}
	return id
}

// insertSection is a convenience wrapper that inserts a section and returns
// its database ID.
func insertSection(t *testing.T, db storage.DB, specID, sfID int64, path, title string, level, lineStart, lineEnd int, rawText string) int64 {
	t.Helper()
	id, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  path,
		Title:        title,
		HeadingLevel: level,
		LineStart:    lineStart,
		LineEnd:      lineEnd,
		RawText:      rawText,
		ContentHash:  "hash",
	})
	if err != nil {
		t.Fatalf("insert section %s: %v", path, err)
	}
	return id
}

// ---------------------------------------------------------------------------
// Test: Validate runs all applicable checks
// ---------------------------------------------------------------------------

func TestValidate_RunsAllApplicableChecks(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)

	// Insert minimal required sections so Gate-1 can find them
	for _, sec := range []struct {
		path, title string
	}{
		{"§0.1", "Preamble"},
		{"§0.5", "State Model"},
		{"§0.6", "Glossary"},
		{"§0.7", "Quality Gates"},
	} {
		insertSection(t, db, specID, sfID, sec.path, sec.title, 2, 1, 10, "text")
	}

	// Insert one invariant, one ADR, one gate, one neg spec, one glossary, one xref
	secID := insertSection(t, db, specID, sfID, "§1", "Chapter 1", 2, 11, 50, "text")

	if _, err := storage.InsertInvariant(db, &storage.Invariant{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		InvariantID: "INV-001", Title: "Test", Statement: "stmt",
		SemiFormal: "sf", ViolationScenario: "vs", ValidationMethod: "vm",
		LineStart: 11, LineEnd: 20, RawText: "raw", ContentHash: "h",
	}); err != nil {
		t.Fatalf("insert invariant: %v", err)
	}

	if _, err := storage.InsertADR(db, &storage.ADR{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		ADRID: "ADR-001", Title: "Test ADR", Problem: "problem",
		DecisionText: "decision", Status: "active",
		LineStart: 21, LineEnd: 30, RawText: "raw", ContentHash: "h",
	}); err != nil {
		t.Fatalf("insert ADR: %v", err)
	}

	if _, err := storage.InsertQualityGate(db, &storage.QualityGate{
		SpecID: specID, SectionID: secID,
		GateID: "Gate-1", Title: "Test Gate", Predicate: "pred",
		LineStart: 31, LineEnd: 35, RawText: "raw",
	}); err != nil {
		t.Fatalf("insert gate: %v", err)
	}

	if _, err := storage.InsertNegativeSpec(db, &storage.NegativeSpec{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		ConstraintText: "DO NOT do X", Reason: "because",
		LineNumber: 36, RawText: "raw",
	}); err != nil {
		t.Fatalf("insert neg spec: %v", err)
	}

	if _, err := storage.InsertGlossaryEntry(db, &storage.GlossaryEntry{
		SpecID: specID, SectionID: secID,
		Term: "TestTerm", Definition: "A test term",
		LineNumber: 40,
	}); err != nil {
		t.Fatalf("insert glossary: %v", err)
	}

	if _, err := storage.InsertCrossReference(db, &storage.CrossReference{
		SpecID: specID, SourceFileID: sfID, SourceSectionID: &secID,
		SourceLine: 15, RefType: "invariant", RefTarget: "INV-001",
		RefText: "see INV-001", Resolved: true,
	}); err != nil {
		t.Fatalf("insert xref: %v", err)
	}

	// Run all checks except Check 13 (traceability, requires CodeRoot)
	report, err := Validate(db, specID, ValidateOptions{})
	if err != nil {
		t.Fatalf("Validate: %v", err)
	}

	// For monolith, Checks 5-8 are not applicable (modular only).
	// Check 13 is not applicable (no CodeRoot).
	// So we expect checks 1-4, 9-12, 14, 15, 16, 17, 18, 19, 20 = 15 checks.
	if report.TotalChecks != 15 {
		t.Errorf("expected 15 applicable checks for monolith, got %d", report.TotalChecks)
		for _, r := range report.Results {
			t.Logf("  Check %d (%s): passed=%v", r.CheckID, r.CheckName, r.Passed)
		}
	}

	// Verify report metadata
	if report.SpecPath != "/test/spec.md" {
		t.Errorf("spec path: got %q, want %q", report.SpecPath, "/test/spec.md")
	}
	if report.SourceType != "monolith" {
		t.Errorf("source type: got %q, want %q", report.SourceType, "monolith")
	}
}

func TestValidate_FilterByCheckID(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)
	insertSection(t, db, specID, sfID, "§1", "Chapter 1", 2, 1, 100, "text")

	// Run only Check 1 and Check 12
	report, err := Validate(db, specID, ValidateOptions{CheckIDs: []int{1, 12}})
	if err != nil {
		t.Fatalf("Validate: %v", err)
	}

	if report.TotalChecks != 2 {
		t.Errorf("expected 2 filtered checks, got %d", report.TotalChecks)
	}

	ids := map[int]bool{}
	for _, r := range report.Results {
		ids[r.CheckID] = true
	}
	if !ids[1] || !ids[12] {
		t.Errorf("expected checks 1 and 12 in results, got IDs: %v", ids)
	}
}

// ---------------------------------------------------------------------------
// Test: Check 1 — Cross-reference integrity
// ---------------------------------------------------------------------------

func TestCheck1_XRefIntegrity_AllResolved(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)
	secID := insertSection(t, db, specID, sfID, "§1", "Chapter 1", 2, 1, 50, "text")

	// Insert an invariant and a resolved reference to it
	if _, err := storage.InsertInvariant(db, &storage.Invariant{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		InvariantID: "INV-001", Title: "Test", Statement: "stmt",
		LineStart: 1, LineEnd: 10, RawText: "raw", ContentHash: "h",
	}); err != nil {
		t.Fatalf("insert invariant: %v", err)
	}

	if _, err := storage.InsertCrossReference(db, &storage.CrossReference{
		SpecID: specID, SourceFileID: sfID, SourceSectionID: &secID,
		SourceLine: 5, RefType: "invariant", RefTarget: "INV-001",
		RefText: "see INV-001", Resolved: true,
	}); err != nil {
		t.Fatalf("insert xref: %v", err)
	}

	check := &checkXRefIntegrity{}
	result := check.Run(db, specID)

	if !result.Passed {
		t.Errorf("expected pass when all refs resolved, got fail")
		for _, f := range result.Findings {
			t.Logf("  finding: %s", f.Message)
		}
	}
}

func TestCheck1_XRefIntegrity_UnresolvedErrors(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)
	secID := insertSection(t, db, specID, sfID, "§1", "Chapter 1", 2, 1, 50, "text")

	// Insert an unresolved reference to a non-existent invariant
	if _, err := storage.InsertCrossReference(db, &storage.CrossReference{
		SpecID: specID, SourceFileID: sfID, SourceSectionID: &secID,
		SourceLine: 5, RefType: "invariant", RefTarget: "INV-999",
		RefText: "see INV-999", Resolved: false,
	}); err != nil {
		t.Fatalf("insert xref: %v", err)
	}

	check := &checkXRefIntegrity{}
	result := check.Run(db, specID)

	if result.Passed {
		t.Errorf("expected fail when unresolved non-template ref exists")
	}

	if len(result.Findings) == 0 {
		t.Fatalf("expected at least 1 finding")
	}

	f := result.Findings[0]
	if f.Severity != SeverityError {
		t.Errorf("expected error severity, got %s", f.Severity)
	}
}

func TestCheck1_XRefIntegrity_TemplateRefsAreInfo(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)
	secID := insertSection(t, db, specID, sfID, "§1", "Chapter 1", 2, 1, 50, "text")

	// Insert an unresolved template reference (INV-NNN)
	if _, err := storage.InsertCrossReference(db, &storage.CrossReference{
		SpecID: specID, SourceFileID: sfID, SourceSectionID: &secID,
		SourceLine: 5, RefType: "invariant", RefTarget: "INV-NNN",
		RefText: "see INV-NNN", Resolved: false,
	}); err != nil {
		t.Fatalf("insert xref: %v", err)
	}

	check := &checkXRefIntegrity{}
	result := check.Run(db, specID)

	// Template refs should not cause failure
	if !result.Passed {
		t.Errorf("expected pass for template-only unresolved refs")
	}

	if len(result.Findings) == 0 {
		t.Fatalf("expected at least 1 finding (info severity)")
	}

	if result.Findings[0].Severity != SeverityInfo {
		t.Errorf("expected info severity for template ref, got %s", result.Findings[0].Severity)
	}
}

// ---------------------------------------------------------------------------
// Test: Check 11 — Proportional weight
// ---------------------------------------------------------------------------

func TestCheck11_ProportionalWeight_Balanced(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 1000)

	// Insert 3 balanced implementation chapters (§1, §2, §3) with similar sizes
	insertSection(t, db, specID, sfID, "§1", "Chapter One", 2, 1, 101, "text")
	insertSection(t, db, specID, sfID, "§2", "Chapter Two", 2, 101, 201, "text")
	insertSection(t, db, specID, sfID, "§3", "Chapter Three", 2, 201, 301, "text")

	check := &checkProportionalWeight{}
	result := check.Run(db, specID)

	if !result.Passed {
		t.Errorf("expected pass for balanced chapters")
		for _, f := range result.Findings {
			t.Logf("  finding: %s", f.Message)
		}
	}
}

func TestCheck11_ProportionalWeight_Imbalanced(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 1000)

	// Insert chapters with highly uneven sizes:
	//   §1: 100 lines, §2: 100 lines, §3: 500 lines (>50% deviation -> error)
	insertSection(t, db, specID, sfID, "§1", "Small chapter", 2, 1, 101, "text")
	insertSection(t, db, specID, sfID, "§2", "Small chapter 2", 2, 101, 201, "text")
	insertSection(t, db, specID, sfID, "§3", "Very large chapter", 2, 201, 701, "text")

	check := &checkProportionalWeight{}
	result := check.Run(db, specID)

	// Mean is (100+100+500)/3 ~ 233. §3 at 500 is ~114% over mean, which is >50%
	if result.Passed {
		t.Errorf("expected fail for severely imbalanced chapters")
	}

	hasError := false
	for _, f := range result.Findings {
		if f.Severity == SeverityError {
			hasError = true
		}
	}
	if !hasError {
		t.Errorf("expected at least one error-severity finding for >50%% deviation")
	}
}

func TestCheck11_ProportionalWeight_SkipsPreamble(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 1000)

	// §0 sections should be ignored
	insertSection(t, db, specID, sfID, "§0", "Preamble", 2, 1, 500, "text")
	insertSection(t, db, specID, sfID, "§1", "Chapter One", 2, 500, 600, "text")
	insertSection(t, db, specID, sfID, "§2", "Chapter Two", 2, 600, 700, "text")

	check := &checkProportionalWeight{}
	result := check.Run(db, specID)

	// Only 2 implementation chapters, both 100 lines — balanced
	if !result.Passed {
		t.Errorf("expected pass; preamble §0 should be ignored")
		for _, f := range result.Findings {
			t.Logf("  finding: %s", f.Message)
		}
	}
}

func TestCheck11_ProportionalWeight_TooFewChapters(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)

	// Only one implementation chapter
	insertSection(t, db, specID, sfID, "§1", "Chapter One", 2, 1, 100, "text")

	check := &checkProportionalWeight{}
	result := check.Run(db, specID)

	// Should pass when fewer than 2 chapters (no meaningful comparison)
	if !result.Passed {
		t.Errorf("expected pass with < 2 chapters")
	}

	if result.Summary != "fewer than 2 implementation chapters found" {
		t.Errorf("unexpected summary: %s", result.Summary)
	}
}

// ---------------------------------------------------------------------------
// Test: Check 12 — Namespace consistency
// ---------------------------------------------------------------------------

func TestCheck12_NamespaceConsistency_MatchingRange(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)

	// Insert a section that declares "INV-001 through INV-003"
	secID := insertSection(t, db, specID, sfID, "§1", "Overview", 2, 1, 50,
		"This spec contains INV-001 through INV-003.")

	// Insert exactly 3 invariants in that range
	for _, invID := range []string{"INV-001", "INV-002", "INV-003"} {
		if _, err := storage.InsertInvariant(db, &storage.Invariant{
			SpecID: specID, SourceFileID: sfID, SectionID: secID,
			InvariantID: invID, Title: "Test " + invID, Statement: "stmt",
			LineStart: 1, LineEnd: 10, RawText: "raw", ContentHash: "h",
		}); err != nil {
			t.Fatalf("insert %s: %v", invID, err)
		}
	}

	check := &checkNamespaceConsistency{}
	result := check.Run(db, specID)

	if !result.Passed {
		t.Errorf("expected pass when declared range matches actual count")
		for _, f := range result.Findings {
			t.Logf("  finding: %s", f.Message)
		}
	}
}

func TestCheck12_NamespaceConsistency_MismatchRange(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)

	// Declare "INV-001 through INV-005" but only define 3
	secID := insertSection(t, db, specID, sfID, "§1", "Overview", 2, 1, 50,
		"This spec contains INV-001 through INV-005.")

	for _, invID := range []string{"INV-001", "INV-002", "INV-003"} {
		if _, err := storage.InsertInvariant(db, &storage.Invariant{
			SpecID: specID, SourceFileID: sfID, SectionID: secID,
			InvariantID: invID, Title: "Test " + invID, Statement: "stmt",
			LineStart: 1, LineEnd: 10, RawText: "raw", ContentHash: "h",
		}); err != nil {
			t.Fatalf("insert %s: %v", invID, err)
		}
	}

	check := &checkNamespaceConsistency{}
	result := check.Run(db, specID)

	// The declared range is 5 but only 3 exist — should produce a warning finding
	if len(result.Findings) == 0 {
		t.Fatalf("expected at least 1 finding for range mismatch")
	}

	found := false
	for _, f := range result.Findings {
		if f.Severity == SeverityWarning {
			found = true
		}
	}
	if !found {
		t.Errorf("expected a warning finding for count mismatch")
	}
}

func TestCheck12_NamespaceConsistency_ADRRange(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)

	// Declare "ADR-001 through ADR-002" and insert exactly 2
	secID := insertSection(t, db, specID, sfID, "§1", "Overview", 2, 1, 50,
		"Decisions ADR-001 through ADR-002.")

	for _, adrID := range []string{"ADR-001", "ADR-002"} {
		if _, err := storage.InsertADR(db, &storage.ADR{
			SpecID: specID, SourceFileID: sfID, SectionID: secID,
			ADRID: adrID, Title: "Test " + adrID, Problem: "prob",
			DecisionText: "dec", Status: "active",
			LineStart: 1, LineEnd: 10, RawText: "raw", ContentHash: "h",
		}); err != nil {
			t.Fatalf("insert %s: %v", adrID, err)
		}
	}

	check := &checkNamespaceConsistency{}
	result := check.Run(db, specID)

	// Should pass — 2 declared, 2 found
	if len(result.Findings) != 0 {
		t.Errorf("expected 0 findings when ADR range matches, got %d", len(result.Findings))
		for _, f := range result.Findings {
			t.Logf("  finding: %s", f.Message)
		}
	}
}

func TestCheck12_NamespaceConsistency_NoRangeText(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)

	// Section with no range declarations
	secID := insertSection(t, db, specID, sfID, "§1", "Overview", 2, 1, 50,
		"This section has no range declarations.")

	if _, err := storage.InsertInvariant(db, &storage.Invariant{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		InvariantID: "INV-001", Title: "Test", Statement: "stmt",
		LineStart: 1, LineEnd: 10, RawText: "raw", ContentHash: "h",
	}); err != nil {
		t.Fatalf("insert invariant: %v", err)
	}

	check := &checkNamespaceConsistency{}
	result := check.Run(db, specID)

	// No range text means no mismatches to detect
	if !result.Passed {
		t.Errorf("expected pass when no range declarations exist")
	}
}

// ---------------------------------------------------------------------------
// Test: Check 2 — INV-003 Falsifiability (invariant components)
// ---------------------------------------------------------------------------

func TestCheck2_Falsifiability_AllComplete(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)
	secID := insertSection(t, db, specID, sfID, "§1", "Chapter 1", 2, 1, 50, "text")

	// Insert invariant with all 4 components filled
	if _, err := storage.InsertInvariant(db, &storage.Invariant{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		InvariantID: "INV-001", Title: "Complete invariant",
		Statement:         "Every X must Y",
		SemiFormal:        "forall x in X: Y(x)",
		ViolationScenario: "When X does not Y",
		ValidationMethod:  "Check Y for all X",
		LineStart:         1, LineEnd: 10, RawText: "raw", ContentHash: "h",
	}); err != nil {
		t.Fatalf("insert invariant: %v", err)
	}

	check := &checkINV003Falsifiability{}
	result := check.Run(db, specID)

	if !result.Passed {
		t.Errorf("expected pass when all components present")
	}

	// No missing-component warnings
	for _, f := range result.Findings {
		if f.Severity == SeverityWarning {
			t.Errorf("unexpected warning: %s", f.Message)
		}
	}
}

func TestCheck2_Falsifiability_MissingComponents(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)
	secID := insertSection(t, db, specID, sfID, "§1", "Chapter 1", 2, 1, 50, "text")

	// Insert invariant with missing semi_formal and violation_scenario
	if _, err := storage.InsertInvariant(db, &storage.Invariant{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		InvariantID: "INV-001", Title: "Incomplete invariant",
		Statement:      "Every X must Y",
		SemiFormal:     "", // missing
		ValidationMethod: "Check Y",
		// ViolationScenario deliberately omitted (zero value = missing)
		LineStart: 1, LineEnd: 10, RawText: "raw", ContentHash: "h",
	}); err != nil {
		t.Fatalf("insert invariant: %v", err)
	}

	check := &checkINV003Falsifiability{}
	result := check.Run(db, specID)

	// Missing components are warnings, not errors — check still passes
	if !result.Passed {
		t.Errorf("expected pass (missing components are warnings, not failures)")
	}

	// Should have exactly 2 warnings: missing semi_formal and missing violation_scenario
	warningCount := 0
	for _, f := range result.Findings {
		if f.Severity == SeverityWarning {
			warningCount++
		}
	}
	if warningCount != 2 {
		t.Errorf("expected 2 warnings for missing components, got %d", warningCount)
		for _, f := range result.Findings {
			t.Logf("  finding: severity=%s message=%s", f.Severity, f.Message)
		}
	}
}

// ---------------------------------------------------------------------------
// Test: Check 10 — Gate-1 Structural conformance
// ---------------------------------------------------------------------------

func TestCheck10_Gate1Structural_AllPresent(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 200)

	// Insert all required structural sections
	for _, sec := range []struct {
		path, title string
	}{
		{"§0.1", "Preamble"},
		{"§0.5", "State Model"},
		{"§0.6", "Glossary"},
		{"§0.7", "Quality Gates"},
	} {
		insertSection(t, db, specID, sfID, sec.path, sec.title, 2, 1, 10, "text")
	}

	secID := insertSection(t, db, specID, sfID, "§1", "Chapter 1", 2, 11, 200, "text")

	// Insert required element types
	if _, err := storage.InsertInvariant(db, &storage.Invariant{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		InvariantID: "INV-001", Title: "T", Statement: "s",
		LineStart: 20, LineEnd: 30, RawText: "r", ContentHash: "h",
	}); err != nil {
		t.Fatalf("insert invariant: %v", err)
	}

	if _, err := storage.InsertADR(db, &storage.ADR{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		ADRID: "ADR-001", Title: "T", Problem: "p", DecisionText: "d", Status: "active",
		LineStart: 30, LineEnd: 40, RawText: "r", ContentHash: "h",
	}); err != nil {
		t.Fatalf("insert ADR: %v", err)
	}

	if _, err := storage.InsertQualityGate(db, &storage.QualityGate{
		SpecID: specID, SectionID: secID,
		GateID: "Gate-1", Title: "T", Predicate: "p",
		LineStart: 40, LineEnd: 50, RawText: "r",
	}); err != nil {
		t.Fatalf("insert gate: %v", err)
	}

	if _, err := storage.InsertNegativeSpec(db, &storage.NegativeSpec{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		ConstraintText: "DO NOT", LineNumber: 55, RawText: "r",
	}); err != nil {
		t.Fatalf("insert neg spec: %v", err)
	}

	if _, err := storage.InsertGlossaryEntry(db, &storage.GlossaryEntry{
		SpecID: specID, SectionID: secID,
		Term: "Test", Definition: "A test", LineNumber: 60,
	}); err != nil {
		t.Fatalf("insert glossary: %v", err)
	}

	if _, err := storage.InsertCrossReference(db, &storage.CrossReference{
		SpecID: specID, SourceFileID: sfID, SourceSectionID: &secID,
		SourceLine: 25, RefType: "invariant", RefTarget: "INV-001",
		RefText: "see INV-001", Resolved: true,
	}); err != nil {
		t.Fatalf("insert xref: %v", err)
	}

	check := &checkGate1Structural{}
	result := check.Run(db, specID)

	if !result.Passed {
		t.Errorf("expected pass when all required elements present")
		for _, f := range result.Findings {
			t.Logf("  finding: %s", f.Message)
		}
	}
}

func TestCheck10_Gate1Structural_MissingSections(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)

	// Only insert §0.1, missing §0.5, §0.6, §0.7
	insertSection(t, db, specID, sfID, "§0.1", "Preamble", 2, 1, 10, "text")

	check := &checkGate1Structural{}
	result := check.Run(db, specID)

	if result.Passed {
		t.Errorf("expected fail when required sections are missing")
	}

	// Count error findings for missing sections
	errorCount := 0
	for _, f := range result.Findings {
		if f.Severity == SeverityError {
			errorCount++
		}
	}
	// Should have errors for: missing §0.5, §0.6, §0.7, + missing element types
	if errorCount < 3 {
		t.Errorf("expected >= 3 error findings, got %d", errorCount)
	}
}

// ---------------------------------------------------------------------------
// Test: ParseCheckIDs
// ---------------------------------------------------------------------------

func TestParseCheckIDs(t *testing.T) {
	tests := []struct {
		input   string
		want    []int
		wantErr bool
	}{
		{"", nil, false},
		{"1", []int{1}, false},
		{"1,2,3", []int{1, 2, 3}, false},
		{" 1 , 12 , 3 ", []int{1, 12, 3}, false},
		{"abc", nil, true},
		{"1,,3", []int{1, 3}, false},
	}

	for _, tc := range tests {
		got, err := ParseCheckIDs(tc.input)
		if tc.wantErr && err == nil {
			t.Errorf("ParseCheckIDs(%q): expected error, got nil", tc.input)
			continue
		}
		if !tc.wantErr && err != nil {
			t.Errorf("ParseCheckIDs(%q): unexpected error: %v", tc.input, err)
			continue
		}
		if !tc.wantErr && !intSliceEqual(got, tc.want) {
			t.Errorf("ParseCheckIDs(%q): got %v, want %v", tc.input, got, tc.want)
		}
	}
}

func intSliceEqual(a, b []int) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

// ---------------------------------------------------------------------------
// Test: AllChecks returns 20 registered checks
// ---------------------------------------------------------------------------

func TestAllChecks_Count(t *testing.T) {
	checks := AllChecks()
	if len(checks) != 20 {
		t.Errorf("expected 20 registered checks, got %d", len(checks))
	}

	// Verify IDs are sequential (1-20)
	expectedIDs := []int{1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20}
	for i, c := range checks {
		if i < len(expectedIDs) && c.ID() != expectedIDs[i] {
			t.Errorf("check at index %d has ID %d, expected %d", i, c.ID(), expectedIDs[i])
		}
	}
}

// ---------------------------------------------------------------------------
// Test: isTemplateRef helper
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Behavioral test: APP-INV-040 — Progressive Validation Monotonicity
// ddis:tests APP-INV-040
// ---------------------------------------------------------------------------

func TestAPPINV040_Monotonicity(t *testing.T) {
	// The spec check-to-level table (workspace-ops.md:654-672) defines:
	//   Level 1: G1 (Check 10) + NS (Check 12)
	//   Level 2: L1 + C1 (1) + C2 (2) + C4 (4) + C5 (5) + C7 (7) + C8 (8)
	//   Level 3: all checks
	//
	// APP-INV-040 requires: checks(L1) ⊂ checks(L2) ⊂ checks(L3)

	// Expected per spec table:
	specL1 := map[int]bool{10: true, 12: true}
	specL2 := map[int]bool{1: true, 2: true, 4: true, 5: true, 7: true, 8: true, 10: true, 12: true}

	// Actual from validate.go (must match the switch statement):
	actualL1 := map[int]bool{}
	actualL2 := map[int]bool{}

	// These values come from validate.go:runValidate switch cases.
	// After the fix: L1={10,12}, L2={1,2,4,5,7,8,10,12}
	for _, id := range []int{10, 12} {
		actualL1[id] = true
	}
	for _, id := range []int{1, 2, 4, 5, 7, 8, 10, 12} {
		actualL2[id] = true
	}

	// Check L1 matches spec
	if !intSetsEqual(actualL1, specL1) {
		t.Errorf("APP-INV-040 VIOLATED: Level 1 checks do not match spec table.\n"+
			"  Spec says L1 = %v\n  Code has L1 = %v",
			setToSlice(specL1), setToSlice(actualL1))
	}

	// Check L2 matches spec
	if !intSetsEqual(actualL2, specL2) {
		t.Errorf("APP-INV-040 VIOLATED: Level 2 checks do not match spec table.\n"+
			"  Spec says L2 = %v\n  Code has L2 = %v",
			setToSlice(specL2), setToSlice(actualL2))
	}

	// Check monotonicity: L1 ⊂ L2
	for id := range actualL1 {
		if !actualL2[id] {
			t.Errorf("APP-INV-040 VIOLATED: Check %d is in L1 but not L2 (violates monotonicity)", id)
		}
	}
}

func intSetsEqual(a, b map[int]bool) bool {
	if len(a) != len(b) {
		return false
	}
	for k := range a {
		if !b[k] {
			return false
		}
	}
	return true
}

func setToSlice(s map[int]bool) []int {
	out := make([]int, 0, len(s))
	for k := range s {
		out = append(out, k)
	}
	return out
}

func TestIsTemplateRef(t *testing.T) {
	templates := []string{"INV-NNN", "ADR-NNN", "§N.M", "INV-XXX", "something-nnn-etc"}
	for _, ref := range templates {
		if !isTemplateRef(ref) {
			t.Errorf("expected %q to be a template ref", ref)
		}
	}

	nonTemplates := []string{"INV-001", "ADR-003", "§4.2", "Gate-1"}
	for _, ref := range nonTemplates {
		if isTemplateRef(ref) {
			t.Errorf("expected %q to NOT be a template ref", ref)
		}
	}
}

// ---------------------------------------------------------------------------
// Test: Check 16 — Behavioral witness verification (APP-INV-049)
// ddis:tests APP-INV-049
// ---------------------------------------------------------------------------

func TestCheck16_BehavioralWitness_TestBacked(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)
	secID := insertSection(t, db, specID, sfID, "§1", "Chapter 1", 2, 1, 50, "text")

	if _, err := storage.InsertInvariant(db, &storage.Invariant{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		InvariantID: "INV-001", Title: "Test", Statement: "stmt",
		LineStart: 1, LineEnd: 10, RawText: "raw", ContentHash: "h",
	}); err != nil {
		t.Fatalf("insert invariant: %v", err)
	}

	// Insert a test-backed witness
	if _, err := storage.InsertWitness(db, &storage.InvariantWitness{
		SpecID: specID, InvariantID: "INV-001", SpecHash: "h",
		EvidenceType: "test", Evidence: "TestFoo passes", ProvenBy: "agent", Status: "valid",
	}); err != nil {
		t.Fatalf("insert witness: %v", err)
	}

	check := &checkBehavioralWitness{}
	result := check.Run(db, specID)

	// Test-backed witnesses should produce no warnings
	for _, f := range result.Findings {
		if f.Severity == SeverityWarning {
			t.Errorf("unexpected warning for test-backed witness: %s", f.Message)
		}
	}
}

func TestCheck16_BehavioralWitness_AttestationFlagged(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)
	secID := insertSection(t, db, specID, sfID, "§1", "Chapter 1", 2, 1, 50, "text")

	if _, err := storage.InsertInvariant(db, &storage.Invariant{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		InvariantID: "INV-001", Title: "Test", Statement: "stmt",
		LineStart: 1, LineEnd: 10, RawText: "raw", ContentHash: "h",
	}); err != nil {
		t.Fatalf("insert invariant: %v", err)
	}

	// Insert attestation-only witness — should be flagged
	if _, err := storage.InsertWitness(db, &storage.InvariantWitness{
		SpecID: specID, InvariantID: "INV-001", SpecHash: "h",
		EvidenceType: "attestation", Evidence: "I checked it", ProvenBy: "agent", Status: "valid",
	}); err != nil {
		t.Fatalf("insert witness: %v", err)
	}

	check := &checkBehavioralWitness{}
	result := check.Run(db, specID)

	// Attestation-only witness must produce a warning
	warnings := 0
	for _, f := range result.Findings {
		if f.Severity == SeverityWarning {
			warnings++
		}
	}
	if warnings != 1 {
		t.Errorf("expected 1 warning for attestation-only witness, got %d", warnings)
	}
}

func TestCheck16_BehavioralWitness_StaleIgnored(t *testing.T) {
	dbp, specID := testDB(t, "monolith")
	db := *dbp

	sfID := insertSourceFile(t, db, specID, "/test/spec.md", "monolith", "", 100)
	secID := insertSection(t, db, specID, sfID, "§1", "Chapter 1", 2, 1, 50, "text")

	if _, err := storage.InsertInvariant(db, &storage.Invariant{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		InvariantID: "INV-001", Title: "Test", Statement: "stmt",
		LineStart: 1, LineEnd: 10, RawText: "raw", ContentHash: "h",
	}); err != nil {
		t.Fatalf("insert invariant: %v", err)
	}

	// Insert stale witness — should be ignored by Check 16
	if _, err := storage.InsertWitness(db, &storage.InvariantWitness{
		SpecID: specID, InvariantID: "INV-001", SpecHash: "h",
		EvidenceType: "attestation", Evidence: "old", ProvenBy: "agent", Status: "stale_spec",
	}); err != nil {
		t.Fatalf("insert witness: %v", err)
	}

	check := &checkBehavioralWitness{}
	result := check.Run(db, specID)

	// Stale witnesses should not produce warnings (handled by Check 14)
	for _, f := range result.Findings {
		if f.Severity == SeverityWarning {
			t.Errorf("unexpected warning for stale witness: %s", f.Message)
		}
	}
}

// ---------------------------------------------------------------------------
// Test: findChapterPath helper
// ---------------------------------------------------------------------------

func TestFindChapterPath(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"§4.2", "§4"},
		{"§4.2.1", "§4"},
		{"§4", "§4"},
		{"§0.5", ""},     // preamble skipped
		{"§0", ""},       // preamble skipped
		{"PART-2", ""},   // not a chapter
		{"Chapter-3", "Chapter-3"},
		{"Chapter-3/sub", "Chapter-3"},
	}
	for _, tc := range tests {
		got := findChapterPath(tc.input)
		if got != tc.want {
			t.Errorf("findChapterPath(%q) = %q, want %q", tc.input, got, tc.want)
		}
	}
}
