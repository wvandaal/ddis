package storage

import (
	"testing"
)

// helper opens an in-memory database with the full schema applied.
func openTestDB(t *testing.T) *testDB {
	t.Helper()
	db, err := Open(":memory:")
	if err != nil {
		t.Fatalf("Open(:memory:) failed: %v", err)
	}
	t.Cleanup(func() { db.Close() })
	return &testDB{DB: db, t: t}
}

// testDB wraps *sql.DB with helpers that insert prerequisite rows.
type testDB struct {
	DB
	t *testing.T
}

// insertSpec inserts a minimal spec_index row and returns its ID.
func (d *testDB) insertSpec() int64 {
	d.t.Helper()
	id, err := InsertSpecIndex(d.DB, &SpecIndex{
		SpecPath:    "/tmp/test-spec.md",
		SpecName:    "Test Spec",
		DDISVersion: "3.0",
		TotalLines:  100,
		ContentHash: "abc123",
		ParsedAt:    "2026-01-01T00:00:00Z",
		SourceType:  "monolith",
	})
	if err != nil {
		d.t.Fatalf("insertSpec: %v", err)
	}
	return id
}

// insertSourceFile inserts a minimal source_files row and returns its ID.
func (d *testDB) insertSourceFile(specID int64) int64 {
	d.t.Helper()
	id, err := InsertSourceFile(d.DB, &SourceFile{
		SpecID:      specID,
		FilePath:    "/tmp/test-spec.md",
		FileRole:    "monolith",
		ContentHash: "filehash123",
		LineCount:   100,
		RawText:     "# Test Spec\nSome content",
	})
	if err != nil {
		d.t.Fatalf("insertSourceFile: %v", err)
	}
	return id
}

// insertSection inserts a minimal sections row and returns its ID.
func (d *testDB) insertSection(specID, sourceFileID int64) int64 {
	d.t.Helper()
	id, err := InsertSection(d.DB, &Section{
		SpecID:       specID,
		SourceFileID: sourceFileID,
		SectionPath:  "Chapter-1",
		Title:        "Introduction",
		HeadingLevel: 1,
		LineStart:    1,
		LineEnd:      50,
		RawText:      "# Introduction\nSome text",
		ContentHash:  "sechash123",
	})
	if err != nil {
		d.t.Fatalf("insertSection: %v", err)
	}
	return id
}

// --- Tests ---

func TestOpen_CreatesExpectedTables(t *testing.T) {
	td := openTestDB(t)

	// Query sqlite_master for all table names (excluding internal sqlite_ tables).
	rows, err := td.DB.Query(
		`SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name`)
	if err != nil {
		t.Fatalf("query sqlite_master: %v", err)
	}
	defer rows.Close()

	var tables []string
	for rows.Next() {
		var name string
		if err := rows.Scan(&name); err != nil {
			t.Fatalf("scan table name: %v", err)
		}
		tables = append(tables, name)
	}
	if err := rows.Err(); err != nil {
		t.Fatalf("rows.Err: %v", err)
	}

	// These are the tables we expect from SchemaSQL.
	expected := map[string]bool{
		"spec_index":           true,
		"source_files":         true,
		"sections":             true,
		"invariants":           true,
		"adrs":                 true,
		"adr_options":          true,
		"quality_gates":        true,
		"negative_specs":       true,
		"verification_prompts": true,
		"verification_checks":  true,
		"meta_instructions":    true,
		"worked_examples":      true,
		"why_not_annotations":  true,
		"comparison_blocks":    true,
		"performance_budgets":  true,
		"budget_entries":       true,
		"state_machines":       true,
		"state_machine_cells":  true,
		"glossary_entries":     true,
		"cross_references":     true,
		"modules":              true,
		"module_relationships": true,
		"module_negative_specs": true,
		"manifest":             true,
		"invariant_registry":   true,
		"transactions":         true,
		"tx_operations":        true,
		"formatting_hints":     true,
		"fts_index":            true,
		"search_vectors":       true,
		"search_model":         true,
		"search_authority":     true,
		"session_state":        true,
		"code_annotations":     true,
	}

	found := make(map[string]bool)
	for _, name := range tables {
		found[name] = true
	}

	for table := range expected {
		if !found[table] {
			t.Errorf("expected table %q not found in database", table)
		}
	}
}

func TestInsertSpec_GetFirstAndLatestSpecID_RoundTrip(t *testing.T) {
	td := openTestDB(t)

	spec := &SpecIndex{
		SpecPath:    "/tmp/my-spec.md",
		SpecName:    "My Spec",
		DDISVersion: "3.0",
		TotalLines:  200,
		ContentHash: "hash_abc",
		ParsedAt:    "2026-02-25T12:00:00Z",
		SourceType:  "monolith",
	}

	insertedID, err := InsertSpecIndex(td.DB, spec)
	if err != nil {
		t.Fatalf("InsertSpecIndex: %v", err)
	}
	if insertedID <= 0 {
		t.Fatalf("expected positive ID, got %d", insertedID)
	}

	// GetFirstSpecID should return the same ID.
	gotID, err := GetFirstSpecID(td.DB)
	if err != nil {
		t.Fatalf("GetFirstSpecID: %v", err)
	}
	if gotID != insertedID {
		t.Errorf("GetFirstSpecID = %d, want %d", gotID, insertedID)
	}

	// GetLatestSpecID should also return the same ID (only one spec).
	latestID, err := GetLatestSpecID(td.DB)
	if err != nil {
		t.Fatalf("GetLatestSpecID: %v", err)
	}
	if latestID != insertedID {
		t.Errorf("GetLatestSpecID = %d, want %d", latestID, insertedID)
	}

	// Insert a second spec — GetFirstSpecID returns first, GetLatestSpecID returns second.
	spec2 := &SpecIndex{
		SpecPath:    "/tmp/my-spec.md",
		SpecName:    "My Spec v2",
		DDISVersion: "3.0",
		TotalLines:  300,
		ContentHash: "hash_def",
		ParsedAt:    "2026-02-25T13:00:00Z",
		SourceType:  "monolith",
	}
	secondID, err := InsertSpecIndex(td.DB, spec2)
	if err != nil {
		t.Fatalf("InsertSpecIndex (second): %v", err)
	}

	firstID, err := GetFirstSpecID(td.DB)
	if err != nil {
		t.Fatalf("GetFirstSpecID after second insert: %v", err)
	}
	if firstID != insertedID {
		t.Errorf("GetFirstSpecID = %d, want %d (original)", firstID, insertedID)
	}

	latestID2, err := GetLatestSpecID(td.DB)
	if err != nil {
		t.Fatalf("GetLatestSpecID after second insert: %v", err)
	}
	if latestID2 != secondID {
		t.Errorf("GetLatestSpecID = %d, want %d (second)", latestID2, secondID)
	}

	// GetSpecIndex should return matching data.
	got, err := GetSpecIndex(td.DB, insertedID)
	if err != nil {
		t.Fatalf("GetSpecIndex: %v", err)
	}
	if got.SpecPath != spec.SpecPath {
		t.Errorf("SpecPath = %q, want %q", got.SpecPath, spec.SpecPath)
	}
	if got.SpecName != spec.SpecName {
		t.Errorf("SpecName = %q, want %q", got.SpecName, spec.SpecName)
	}
	if got.DDISVersion != spec.DDISVersion {
		t.Errorf("DDISVersion = %q, want %q", got.DDISVersion, spec.DDISVersion)
	}
	if got.TotalLines != spec.TotalLines {
		t.Errorf("TotalLines = %d, want %d", got.TotalLines, spec.TotalLines)
	}
	if got.ContentHash != spec.ContentHash {
		t.Errorf("ContentHash = %q, want %q", got.ContentHash, spec.ContentHash)
	}
	if got.SourceType != spec.SourceType {
		t.Errorf("SourceType = %q, want %q", got.SourceType, spec.SourceType)
	}
}

func TestInsertInvariant_ListInvariants_RoundTrip(t *testing.T) {
	td := openTestDB(t)
	specID := td.insertSpec()
	sfID := td.insertSourceFile(specID)
	secID := td.insertSection(specID, sfID)

	inv := &Invariant{
		SpecID:            specID,
		SourceFileID:      sfID,
		SectionID:         secID,
		InvariantID:       "INV-001",
		Title:             "No null references",
		Statement:         "All references must resolve to existing elements.",
		SemiFormal:        "forall r in refs: exists(r.target)",
		ViolationScenario: "Reference INV-999 appears but no such invariant exists.",
		ValidationMethod:  "Cross-reference resolution pass",
		WhyThisMatters:    "Broken references confuse implementers.",
		ConditionalTag:    "",
		LineStart:         10,
		LineEnd:           25,
		RawText:           "**INV-001:** No null references\n...",
		ContentHash:       "invhash001",
	}

	insertedID, err := InsertInvariant(td.DB, inv)
	if err != nil {
		t.Fatalf("InsertInvariant: %v", err)
	}
	if insertedID <= 0 {
		t.Fatalf("expected positive ID, got %d", insertedID)
	}

	// Insert a second invariant to verify list returns both.
	inv2 := &Invariant{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionID:    secID,
		InvariantID:  "INV-002",
		Title:        "Deterministic parsing",
		Statement:    "Same input always produces same output.",
		LineStart:    30,
		LineEnd:      40,
		RawText:      "**INV-002:** Deterministic parsing\n...",
		ContentHash:  "invhash002",
	}
	if _, err := InsertInvariant(td.DB, inv2); err != nil {
		t.Fatalf("InsertInvariant (INV-002): %v", err)
	}

	// ListInvariants should return both, ordered by invariant_id.
	invs, err := ListInvariants(td.DB, specID)
	if err != nil {
		t.Fatalf("ListInvariants: %v", err)
	}
	if len(invs) != 2 {
		t.Fatalf("ListInvariants returned %d invariants, want 2", len(invs))
	}

	// Verify first invariant fields.
	got := invs[0]
	if got.InvariantID != "INV-001" {
		t.Errorf("InvariantID = %q, want %q", got.InvariantID, "INV-001")
	}
	if got.Title != inv.Title {
		t.Errorf("Title = %q, want %q", got.Title, inv.Title)
	}
	if got.Statement != inv.Statement {
		t.Errorf("Statement = %q, want %q", got.Statement, inv.Statement)
	}
	if got.SemiFormal != inv.SemiFormal {
		t.Errorf("SemiFormal = %q, want %q", got.SemiFormal, inv.SemiFormal)
	}
	if got.ViolationScenario != inv.ViolationScenario {
		t.Errorf("ViolationScenario = %q, want %q", got.ViolationScenario, inv.ViolationScenario)
	}
	if got.ValidationMethod != inv.ValidationMethod {
		t.Errorf("ValidationMethod = %q, want %q", got.ValidationMethod, inv.ValidationMethod)
	}
	if got.WhyThisMatters != inv.WhyThisMatters {
		t.Errorf("WhyThisMatters = %q, want %q", got.WhyThisMatters, inv.WhyThisMatters)
	}
	if got.LineStart != inv.LineStart {
		t.Errorf("LineStart = %d, want %d", got.LineStart, inv.LineStart)
	}
	if got.LineEnd != inv.LineEnd {
		t.Errorf("LineEnd = %d, want %d", got.LineEnd, inv.LineEnd)
	}
	if got.ContentHash != inv.ContentHash {
		t.Errorf("ContentHash = %q, want %q", got.ContentHash, inv.ContentHash)
	}

	// Second invariant should be INV-002.
	if invs[1].InvariantID != "INV-002" {
		t.Errorf("second InvariantID = %q, want %q", invs[1].InvariantID, "INV-002")
	}

	// GetInvariant should also work for individual retrieval.
	single, err := GetInvariant(td.DB, specID, "INV-001")
	if err != nil {
		t.Fatalf("GetInvariant: %v", err)
	}
	if single.Title != inv.Title {
		t.Errorf("GetInvariant Title = %q, want %q", single.Title, inv.Title)
	}
}

func TestInsertADR_ListADRs_RoundTrip(t *testing.T) {
	td := openTestDB(t)
	specID := td.insertSpec()
	sfID := td.insertSourceFile(specID)
	secID := td.insertSection(specID, sfID)

	adr := &ADR{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionID:    secID,
		ADRID:        "ADR-001",
		Title:        "Use SQLite for storage",
		Problem:      "Need a simple embedded database.",
		DecisionText: "We chose SQLite for its zero-config nature.",
		ChosenOption: "SQLite",
		Consequences: "Single-file database, no server needed.",
		Tests:        "Verify round-trip fidelity.",
		Confidence:   "Committed",
		Status:       "active",
		SupersededBy: "",
		LineStart:    60,
		LineEnd:      80,
		RawText:      "**ADR-001:** Use SQLite for storage\n...",
		ContentHash:  "adrhash001",
	}

	insertedID, err := InsertADR(td.DB, adr)
	if err != nil {
		t.Fatalf("InsertADR: %v", err)
	}
	if insertedID <= 0 {
		t.Fatalf("expected positive ID, got %d", insertedID)
	}

	// Insert a second ADR.
	adr2 := &ADR{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionID:    secID,
		ADRID:        "ADR-002",
		Title:        "Use Go for CLI",
		Problem:      "Need portable binary distribution.",
		DecisionText: "Go compiles to a single static binary.",
		Confidence:   "Committed",
		Status:       "active",
		LineStart:    90,
		LineEnd:      110,
		RawText:      "**ADR-002:** Use Go for CLI\n...",
		ContentHash:  "adrhash002",
	}
	if _, err := InsertADR(td.DB, adr2); err != nil {
		t.Fatalf("InsertADR (ADR-002): %v", err)
	}

	// ListADRs should return both, ordered by adr_id.
	adrs, err := ListADRs(td.DB, specID)
	if err != nil {
		t.Fatalf("ListADRs: %v", err)
	}
	if len(adrs) != 2 {
		t.Fatalf("ListADRs returned %d ADRs, want 2", len(adrs))
	}

	// Verify first ADR fields.
	got := adrs[0]
	if got.ADRID != "ADR-001" {
		t.Errorf("ADRID = %q, want %q", got.ADRID, "ADR-001")
	}
	if got.Title != adr.Title {
		t.Errorf("Title = %q, want %q", got.Title, adr.Title)
	}
	if got.Problem != adr.Problem {
		t.Errorf("Problem = %q, want %q", got.Problem, adr.Problem)
	}
	if got.DecisionText != adr.DecisionText {
		t.Errorf("DecisionText = %q, want %q", got.DecisionText, adr.DecisionText)
	}
	if got.ChosenOption != adr.ChosenOption {
		t.Errorf("ChosenOption = %q, want %q", got.ChosenOption, adr.ChosenOption)
	}
	if got.Consequences != adr.Consequences {
		t.Errorf("Consequences = %q, want %q", got.Consequences, adr.Consequences)
	}
	if got.Confidence != adr.Confidence {
		t.Errorf("Confidence = %q, want %q", got.Confidence, adr.Confidence)
	}
	if got.Status != adr.Status {
		t.Errorf("Status = %q, want %q", got.Status, adr.Status)
	}
	if got.LineStart != adr.LineStart {
		t.Errorf("LineStart = %d, want %d", got.LineStart, adr.LineStart)
	}
	if got.LineEnd != adr.LineEnd {
		t.Errorf("LineEnd = %d, want %d", got.LineEnd, adr.LineEnd)
	}

	// Second should be ADR-002.
	if adrs[1].ADRID != "ADR-002" {
		t.Errorf("second ADRID = %q, want %q", adrs[1].ADRID, "ADR-002")
	}

	// GetADR should also work for individual retrieval.
	single, err := GetADR(td.DB, specID, "ADR-001")
	if err != nil {
		t.Fatalf("GetADR: %v", err)
	}
	if single.Title != adr.Title {
		t.Errorf("GetADR Title = %q, want %q", single.Title, adr.Title)
	}
}

func TestInsertSourceFile_GetSourceFiles(t *testing.T) {
	td := openTestDB(t)
	specID := td.insertSpec()

	sf := &SourceFile{
		SpecID:      specID,
		FilePath:    "/tmp/modules/parse-pipeline.md",
		FileRole:    "module",
		ModuleName:  "parse-pipeline",
		ContentHash: "modhash123",
		LineCount:   643,
		RawText:     "# Parse Pipeline\n\nContent here.",
	}

	insertedID, err := InsertSourceFile(td.DB, sf)
	if err != nil {
		t.Fatalf("InsertSourceFile: %v", err)
	}
	if insertedID <= 0 {
		t.Fatalf("expected positive ID, got %d", insertedID)
	}

	// GetSourceFiles should return the inserted file.
	files, err := GetSourceFiles(td.DB, specID)
	if err != nil {
		t.Fatalf("GetSourceFiles: %v", err)
	}
	if len(files) != 1 {
		t.Fatalf("GetSourceFiles returned %d files, want 1", len(files))
	}

	got := files[0]
	if got.FilePath != sf.FilePath {
		t.Errorf("FilePath = %q, want %q", got.FilePath, sf.FilePath)
	}
	if got.FileRole != sf.FileRole {
		t.Errorf("FileRole = %q, want %q", got.FileRole, sf.FileRole)
	}
	if got.ModuleName != sf.ModuleName {
		t.Errorf("ModuleName = %q, want %q", got.ModuleName, sf.ModuleName)
	}
	if got.ContentHash != sf.ContentHash {
		t.Errorf("ContentHash = %q, want %q", got.ContentHash, sf.ContentHash)
	}
	if got.LineCount != sf.LineCount {
		t.Errorf("LineCount = %d, want %d", got.LineCount, sf.LineCount)
	}

	// GetSourceFileContent should return the raw text.
	content, err := GetSourceFileContent(td.DB, insertedID)
	if err != nil {
		t.Fatalf("GetSourceFileContent: %v", err)
	}
	if content != sf.RawText {
		t.Errorf("GetSourceFileContent = %q, want %q", content, sf.RawText)
	}
}

func TestGetFirstSpecID_EmptyDB(t *testing.T) {
	td := openTestDB(t)

	// No specs inserted, GetFirstSpecID should fail.
	_, err := GetFirstSpecID(td.DB)
	if err == nil {
		t.Fatal("expected error from GetFirstSpecID on empty DB, got nil")
	}
}

func TestGetLatestSpecID_EmptyDB(t *testing.T) {
	td := openTestDB(t)

	// No specs inserted, GetLatestSpecID should fail.
	_, err := GetLatestSpecID(td.DB)
	if err == nil {
		t.Fatal("expected error from GetLatestSpecID on empty DB, got nil")
	}
}

func TestInsertInvariant_UniqueConstraint(t *testing.T) {
	td := openTestDB(t)
	specID := td.insertSpec()
	sfID := td.insertSourceFile(specID)
	secID := td.insertSection(specID, sfID)

	inv := &Invariant{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionID:    secID,
		InvariantID:  "INV-001",
		Title:        "Original",
		Statement:    "Original statement",
		LineStart:    1,
		LineEnd:      10,
		RawText:      "raw",
		ContentHash:  "hash1",
	}

	if _, err := InsertInvariant(td.DB, inv); err != nil {
		t.Fatalf("first insert: %v", err)
	}

	// Second insert with same (spec_id, invariant_id) should fail.
	inv2 := *inv
	inv2.Title = "Duplicate"
	inv2.ContentHash = "hash2"
	_, err := InsertInvariant(td.DB, &inv2)
	if err == nil {
		t.Fatal("expected UNIQUE constraint error on duplicate invariant_id, got nil")
	}
}

func TestInsertADR_UniqueConstraint(t *testing.T) {
	td := openTestDB(t)
	specID := td.insertSpec()
	sfID := td.insertSourceFile(specID)
	secID := td.insertSection(specID, sfID)

	adr := &ADR{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionID:    secID,
		ADRID:        "ADR-001",
		Title:        "Original",
		Problem:      "Some problem",
		DecisionText: "Some decision",
		Status:       "active",
		LineStart:    1,
		LineEnd:      10,
		RawText:      "raw",
		ContentHash:  "hash1",
	}

	if _, err := InsertADR(td.DB, adr); err != nil {
		t.Fatalf("first insert: %v", err)
	}

	// Second insert with same (spec_id, adr_id) should upsert (richer content wins).
	adr2 := *adr
	adr2.Title = "Duplicate"
	adr2.Problem = "A much more detailed problem description that is longer"
	adr2.RawText = "raw text that is much longer than the original version for upsert testing"
	adr2.ContentHash = "hash2"
	id2, err := InsertADR(td.DB, &adr2)
	if err != nil {
		t.Fatalf("upsert should succeed: %v", err)
	}

	// Verify richer content was kept
	got, err := GetADR(td.DB, specID, "ADR-001")
	if err != nil {
		t.Fatalf("GetADR after upsert: %v", err)
	}
	if got.Problem != "A much more detailed problem description that is longer" {
		t.Fatalf("upsert should keep richer problem, got %q", got.Problem)
	}
	_ = id2
}

func TestNullableFields_EmptyStringsStoredAsNull(t *testing.T) {
	td := openTestDB(t)
	specID := td.insertSpec()
	sfID := td.insertSourceFile(specID)
	secID := td.insertSection(specID, sfID)

	// Insert an invariant with empty optional fields.
	inv := &Invariant{
		SpecID:            specID,
		SourceFileID:      sfID,
		SectionID:         secID,
		InvariantID:       "INV-010",
		Title:             "Minimal invariant",
		Statement:         "A statement.",
		SemiFormal:        "",
		ViolationScenario: "",
		ValidationMethod:  "",
		WhyThisMatters:    "",
		ConditionalTag:    "",
		LineStart:         1,
		LineEnd:           5,
		RawText:           "raw text",
		ContentHash:       "hash10",
	}

	if _, err := InsertInvariant(td.DB, inv); err != nil {
		t.Fatalf("InsertInvariant: %v", err)
	}

	// Retrieve and verify empty strings come back as empty (not panicking).
	got, err := GetInvariant(td.DB, specID, "INV-010")
	if err != nil {
		t.Fatalf("GetInvariant: %v", err)
	}
	if got.SemiFormal != "" {
		t.Errorf("SemiFormal = %q, want empty", got.SemiFormal)
	}
	if got.ViolationScenario != "" {
		t.Errorf("ViolationScenario = %q, want empty", got.ViolationScenario)
	}
	if got.ValidationMethod != "" {
		t.Errorf("ValidationMethod = %q, want empty", got.ValidationMethod)
	}
	if got.WhyThisMatters != "" {
		t.Errorf("WhyThisMatters = %q, want empty", got.WhyThisMatters)
	}
	if got.ConditionalTag != "" {
		t.Errorf("ConditionalTag = %q, want empty", got.ConditionalTag)
	}
}

func TestListInvariants_EmptyForNonexistentSpec(t *testing.T) {
	td := openTestDB(t)

	invs, err := ListInvariants(td.DB, 9999)
	if err != nil {
		t.Fatalf("ListInvariants: %v", err)
	}
	if len(invs) != 0 {
		t.Errorf("expected 0 invariants for nonexistent spec, got %d", len(invs))
	}
}

func TestListADRs_EmptyForNonexistentSpec(t *testing.T) {
	td := openTestDB(t)

	adrs, err := ListADRs(td.DB, 9999)
	if err != nil {
		t.Fatalf("ListADRs: %v", err)
	}
	if len(adrs) != 0 {
		t.Errorf("expected 0 ADRs for nonexistent spec, got %d", len(adrs))
	}
}

func TestCountElements(t *testing.T) {
	td := openTestDB(t)
	specID := td.insertSpec()
	sfID := td.insertSourceFile(specID)
	secID := td.insertSection(specID, sfID)

	// Insert one invariant and one ADR.
	if _, err := InsertInvariant(td.DB, &Invariant{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		InvariantID: "INV-001", Title: "T", Statement: "S",
		LineStart: 1, LineEnd: 5, RawText: "r", ContentHash: "h",
	}); err != nil {
		t.Fatalf("InsertInvariant: %v", err)
	}

	if _, err := InsertADR(td.DB, &ADR{
		SpecID: specID, SourceFileID: sfID, SectionID: secID,
		ADRID: "ADR-001", Title: "T", Problem: "P", DecisionText: "D",
		Status: "active", LineStart: 10, LineEnd: 20, RawText: "r", ContentHash: "h",
	}); err != nil {
		t.Fatalf("InsertADR: %v", err)
	}

	counts, err := CountElements(td.DB, specID)
	if err != nil {
		t.Fatalf("CountElements: %v", err)
	}

	if counts["invariants"] != 1 {
		t.Errorf("invariants count = %d, want 1", counts["invariants"])
	}
	if counts["adrs"] != 1 {
		t.Errorf("adrs count = %d, want 1", counts["adrs"])
	}
	if counts["sections"] != 1 {
		t.Errorf("sections count = %d, want 1", counts["sections"])
	}
	// Tables with no rows should be 0.
	if counts["quality_gates"] != 0 {
		t.Errorf("quality_gates count = %d, want 0", counts["quality_gates"])
	}
}

func TestSetParentSpecID_GetParentSpecID(t *testing.T) {
	td := openTestDB(t)

	parentID := td.insertSpec()

	// Insert a child spec.
	childID, err := InsertSpecIndex(td.DB, &SpecIndex{
		SpecPath:    "/tmp/child-spec.md",
		SpecName:    "Child Spec",
		DDISVersion: "3.0",
		TotalLines:  50,
		ContentHash: "childhash",
		ParsedAt:    "2026-01-02T00:00:00Z",
		SourceType:  "modular",
	})
	if err != nil {
		t.Fatalf("InsertSpecIndex (child): %v", err)
	}

	// Initially parent should be nil.
	got, err := GetParentSpecID(td.DB, childID)
	if err != nil {
		t.Fatalf("GetParentSpecID: %v", err)
	}
	if got != nil {
		t.Errorf("expected nil parent, got %d", *got)
	}

	// Set parent.
	if err := SetParentSpecID(td.DB, childID, parentID); err != nil {
		t.Fatalf("SetParentSpecID: %v", err)
	}

	// Now parent should be set.
	got, err = GetParentSpecID(td.DB, childID)
	if err != nil {
		t.Fatalf("GetParentSpecID after set: %v", err)
	}
	if got == nil {
		t.Fatal("expected non-nil parent, got nil")
	}
	if *got != parentID {
		t.Errorf("parent_spec_id = %d, want %d", *got, parentID)
	}
}
