package parser

import (
	"database/sql"
	"os"
	"strings"
	"testing"

	"github.com/wvandaal/ddis/internal/storage"
)

// ---------------------------------------------------------------------------
// InvHeaderRe regex matching
// ---------------------------------------------------------------------------

func TestInvHeaderRe_SimpleMatch(t *testing.T) {
	input := "**INV-001: Round-Trip Fidelity**"
	m := InvHeaderRe.FindStringSubmatch(input)
	if m == nil {
		t.Fatalf("expected InvHeaderRe to match %q", input)
	}
	if m[1] != "INV-001" {
		t.Errorf("expected id 'INV-001', got %q", m[1])
	}
	if strings.TrimSpace(m[2]) != "Round-Trip Fidelity" {
		t.Errorf("expected title 'Round-Trip Fidelity', got %q", m[2])
	}
}

func TestInvHeaderRe_AppPrefixed(t *testing.T) {
	input := "**APP-INV-017: Schema Migration Safety**"
	m := InvHeaderRe.FindStringSubmatch(input)
	if m == nil {
		t.Fatalf("expected InvHeaderRe to match %q", input)
	}
	if m[1] != "APP-INV-017" {
		t.Errorf("expected id 'APP-INV-017', got %q", m[1])
	}
	if strings.TrimSpace(m[2]) != "Schema Migration Safety" {
		t.Errorf("expected title 'Schema Migration Safety', got %q", m[2])
	}
}

func TestInvHeaderRe_ConditionalTag(t *testing.T) {
	input := "**APP-INV-001: Conditional Rule** [Conditional]"
	m := InvHeaderRe.FindStringSubmatch(input)
	if m == nil {
		t.Fatalf("expected InvHeaderRe to match %q", input)
	}
	if m[1] != "APP-INV-001" {
		t.Errorf("expected id 'APP-INV-001', got %q", m[1])
	}
	if strings.TrimSpace(m[2]) != "Conditional Rule" {
		t.Errorf("expected title 'Conditional Rule', got %q", m[2])
	}
	if m[3] != "Conditional" {
		t.Errorf("expected conditional tag 'Conditional', got %q", m[3])
	}
}

func TestInvHeaderRe_ConditionalWithDash(t *testing.T) {
	input := "**INV-005: Feature Guard** [Conditional -- only when enabled]"
	m := InvHeaderRe.FindStringSubmatch(input)
	if m == nil {
		t.Fatalf("expected InvHeaderRe to match %q", input)
	}
	if m[3] != "Conditional -- only when enabled" {
		t.Errorf("expected full conditional tag, got %q", m[3])
	}
}

func TestInvHeaderRe_NoMatch(t *testing.T) {
	tests := []string{
		"INV-001: No stars",
		"**Not an invariant**",
		"**INV-ABC: Bad ID**",
		"some random text",
		"",
	}
	for _, input := range tests {
		if m := InvHeaderRe.FindStringSubmatch(input); m != nil {
			t.Errorf("expected no match for %q, got %v", input, m)
		}
	}
}

// ---------------------------------------------------------------------------
// ADRHeaderRe regex matching
// ---------------------------------------------------------------------------

func TestADRHeaderRe_SimpleMatch(t *testing.T) {
	input := "### ADR-001: Use SQLite for Storage"
	m := ADRHeaderRe.FindStringSubmatch(input)
	if m == nil {
		t.Fatalf("expected ADRHeaderRe to match %q", input)
	}
	if m[1] != "ADR-001" {
		t.Errorf("expected id 'ADR-001', got %q", m[1])
	}
	if strings.TrimSpace(m[2]) != "Use SQLite for Storage" {
		t.Errorf("expected title 'Use SQLite for Storage', got %q", m[2])
	}
}

func TestADRHeaderRe_AppPrefixed(t *testing.T) {
	input := "### APP-ADR-005: Normalized Schema"
	m := ADRHeaderRe.FindStringSubmatch(input)
	if m == nil {
		t.Fatalf("expected ADRHeaderRe to match %q", input)
	}
	if m[1] != "APP-ADR-005" {
		t.Errorf("expected id 'APP-ADR-005', got %q", m[1])
	}
	if strings.TrimSpace(m[2]) != "Normalized Schema" {
		t.Errorf("expected title 'Normalized Schema', got %q", m[2])
	}
}

func TestADRHeaderRe_ConditionalTag(t *testing.T) {
	input := "### ADR-003: Caching Strategy [Conditional]"
	m := ADRHeaderRe.FindStringSubmatch(input)
	if m == nil {
		t.Fatalf("expected ADRHeaderRe to match %q", input)
	}
	if m[1] != "ADR-003" {
		t.Errorf("expected id 'ADR-003', got %q", m[1])
	}
	if strings.TrimSpace(m[2]) != "Caching Strategy" {
		t.Errorf("expected title 'Caching Strategy', got %q", m[2])
	}
	if m[3] != "Conditional" {
		t.Errorf("expected conditional tag 'Conditional', got %q", m[3])
	}
}

func TestADRHeaderRe_NoMatch(t *testing.T) {
	tests := []string{
		"ADR-001: No heading prefix",
		"## ADR-001: Wrong heading level",
		"### Not an ADR",
		"### ADR-ABC: Bad ID",
		"",
	}
	for _, input := range tests {
		if m := ADRHeaderRe.FindStringSubmatch(input); m != nil {
			t.Errorf("expected no match for %q, got %v", input, m)
		}
	}
}

// ---------------------------------------------------------------------------
// helpers for DB-backed extraction tests
// ---------------------------------------------------------------------------

// setupTestDB creates an in-memory SQLite database with the DDIS schema,
// inserts a spec and source file, and returns the DB plus their IDs.
func setupTestDB(t *testing.T) (storage.DB, int64, int64) {
	t.Helper()

	db, err := storage.Open(":memory:")
	if err != nil {
		t.Fatalf("create in-memory DB: %v", err)
	}
	t.Cleanup(func() { db.Close() })

	specID, err := storage.InsertSpecIndex(db, &storage.SpecIndex{
		SpecPath:    "test-spec",
		SpecName:    "Test Spec",
		DDISVersion: "3.0",
		TotalLines:  100,
		ContentHash: "abc123",
		ParsedAt:    "2026-01-01T00:00:00Z",
		SourceType:  "monolith",
	})
	if err != nil {
		t.Fatalf("insert spec: %v", err)
	}

	sfID, err := storage.InsertSourceFile(db, &storage.SourceFile{
		SpecID:      specID,
		FilePath:    "test.md",
		FileRole:    "monolith",
		ContentHash: "def456",
		LineCount:   100,
		RawText:     "test content",
	})
	if err != nil {
		t.Fatalf("insert source file: %v", err)
	}

	return db, specID, sfID
}

// setupTestDBRaw returns just the DB (no pre-inserted spec/file).
func setupTestDBRaw(t *testing.T) storage.DB {
	t.Helper()
	db, err := storage.Open(":memory:")
	if err != nil {
		t.Fatalf("create in-memory DB: %v", err)
	}
	t.Cleanup(func() { db.Close() })
	return db
}

// insertTestSection inserts a minimal section row needed as a FK target
// and returns its DB ID.
func insertTestSection(t *testing.T, db storage.DB, specID, sfID int64) int64 {
	t.Helper()
	secID, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "test-section",
		Title:        "Test Section",
		HeadingLevel: 2,
		LineStart:    1,
		LineEnd:      100,
		RawText:      "## Test Section",
		ContentHash:  "sec-hash",
	})
	if err != nil {
		t.Fatalf("insert section: %v", err)
	}
	return secID
}

// ---------------------------------------------------------------------------
// ExtractInvariants
// ---------------------------------------------------------------------------

func TestExtractInvariants_SingleComplete(t *testing.T) {
	db, specID, sfID := setupTestDB(t)
	secID := insertTestSection(t, db, specID, sfID)

	// Build a SectionNode covering the entire input so FindSectionForLine works.
	sections := []*SectionNode{
		{
			SectionPath:  "test-section",
			Title:        "Test Section",
			HeadingLevel: 2,
			LineStart:    0,
			LineEnd:      20,
			DBID:         secID,
		},
	}

	md := `## Test Section

**INV-001: Round-Trip Fidelity**

*The parser must preserve all data during parse-render cycles.*

` + "```" + `
FOR ALL spec S: render(parse(S)) == S
` + "```" + `

Violation scenario: A field is silently dropped during round-trip.

---
`
	lines := strings.Split(md, "\n")

	err := ExtractInvariants(lines, sections, specID, sfID, db)
	if err != nil {
		t.Fatalf("ExtractInvariants: %v", err)
	}

	// Query the database to verify what was inserted
	rows, err := db.Query(
		`SELECT invariant_id, title, statement, semi_formal, violation_scenario
		 FROM invariants WHERE spec_id = ?`, specID)
	if err != nil {
		t.Fatalf("query invariants: %v", err)
	}
	defer rows.Close()

	type invRow struct {
		id, title, statement string
		semiFormal           sql.NullString
		violation            sql.NullString
	}
	var results []invRow
	for rows.Next() {
		var r invRow
		if err := rows.Scan(&r.id, &r.title, &r.statement, &r.semiFormal, &r.violation); err != nil {
			t.Fatalf("scan: %v", err)
		}
		results = append(results, r)
	}

	if len(results) != 1 {
		t.Fatalf("expected 1 invariant, got %d", len(results))
	}

	inv := results[0]
	if inv.id != "INV-001" {
		t.Errorf("expected invariant_id 'INV-001', got %q", inv.id)
	}
	if inv.title != "Round-Trip Fidelity" {
		t.Errorf("expected title 'Round-Trip Fidelity', got %q", inv.title)
	}
	if inv.statement != "The parser must preserve all data during parse-render cycles." {
		t.Errorf("unexpected statement: %q", inv.statement)
	}
	if !inv.semiFormal.Valid || !strings.Contains(inv.semiFormal.String, "render(parse(S))") {
		t.Errorf("expected semi_formal containing 'render(parse(S))', got %v", inv.semiFormal)
	}
	if !inv.violation.Valid || !strings.Contains(inv.violation.String, "silently dropped") {
		t.Errorf("expected violation containing 'silently dropped', got %v", inv.violation)
	}
}

func TestExtractInvariants_TwoConsecutive(t *testing.T) {
	db, specID, sfID := setupTestDB(t)
	secID := insertTestSection(t, db, specID, sfID)

	sections := []*SectionNode{
		{
			SectionPath:  "test-section",
			Title:        "Test Section",
			HeadingLevel: 2,
			LineStart:    0,
			LineEnd:      30,
			DBID:         secID,
		},
	}

	md := `## Test Section

**INV-001: First Invariant**

*First statement about the system.*

Violation scenario: First violation description.

---

**INV-002: Second Invariant**

*Second statement about another property.*

Violation scenario: Second violation description.

---
`
	lines := strings.Split(md, "\n")

	err := ExtractInvariants(lines, sections, specID, sfID, db)
	if err != nil {
		t.Fatalf("ExtractInvariants: %v", err)
	}

	var count int
	err = db.QueryRow(`SELECT COUNT(*) FROM invariants WHERE spec_id = ?`, specID).Scan(&count)
	if err != nil {
		t.Fatalf("count: %v", err)
	}
	if count != 2 {
		t.Errorf("expected 2 invariants, got %d", count)
	}

	// Verify each was extracted with correct IDs
	for _, invID := range []string{"INV-001", "INV-002"} {
		var title string
		err = db.QueryRow(
			`SELECT title FROM invariants WHERE spec_id = ? AND invariant_id = ?`,
			specID, invID).Scan(&title)
		if err != nil {
			t.Errorf("query for %s: %v", invID, err)
		}
	}
}

func TestExtractInvariants_AppPrefixed(t *testing.T) {
	db, specID, sfID := setupTestDB(t)
	secID := insertTestSection(t, db, specID, sfID)

	sections := []*SectionNode{
		{
			SectionPath:  "test-section",
			Title:        "Test Section",
			HeadingLevel: 2,
			LineStart:    0,
			LineEnd:      20,
			DBID:         secID,
		},
	}

	md := `## Test Section

**APP-INV-003: Application-Level Rule** [Conditional]

*The application must enforce this rule when feature is active.*

Violation scenario: The rule is violated when the feature is disabled.

---
`
	lines := strings.Split(md, "\n")

	err := ExtractInvariants(lines, sections, specID, sfID, db)
	if err != nil {
		t.Fatalf("ExtractInvariants: %v", err)
	}

	var invID, conditionalTag string
	err = db.QueryRow(
		`SELECT invariant_id, conditional_tag FROM invariants WHERE spec_id = ?`,
		specID).Scan(&invID, &conditionalTag)
	if err != nil {
		t.Fatalf("query: %v", err)
	}
	if invID != "APP-INV-003" {
		t.Errorf("expected 'APP-INV-003', got %q", invID)
	}
	if conditionalTag != "Conditional" {
		t.Errorf("expected conditional tag 'Conditional', got %q", conditionalTag)
	}
}

func TestExtractInvariants_NoInvariants(t *testing.T) {
	db, specID, sfID := setupTestDB(t)

	lines := strings.Split("## Just a heading\n\nSome paragraph text.\n", "\n")

	err := ExtractInvariants(lines, nil, specID, sfID, db)
	if err != nil {
		t.Fatalf("ExtractInvariants: %v", err)
	}

	var count int
	err = db.QueryRow(`SELECT COUNT(*) FROM invariants WHERE spec_id = ?`, specID).Scan(&count)
	if err != nil {
		t.Fatalf("count: %v", err)
	}
	if count != 0 {
		t.Errorf("expected 0 invariants, got %d", count)
	}
}

func TestExtractInvariants_FlushAtEOF(t *testing.T) {
	// Invariant at end of file with no trailing --- should still be extracted.
	db, specID, sfID := setupTestDB(t)
	secID := insertTestSection(t, db, specID, sfID)

	sections := []*SectionNode{
		{
			SectionPath:  "test-section",
			Title:        "Test Section",
			HeadingLevel: 2,
			LineStart:    0,
			LineEnd:      15,
			DBID:         secID,
		},
	}

	md := `## Test Section

**INV-010: EOF Flush**

*This invariant has no trailing separator.*

Violation scenario: The invariant is lost at end of file.`

	lines := strings.Split(md, "\n")

	err := ExtractInvariants(lines, sections, specID, sfID, db)
	if err != nil {
		t.Fatalf("ExtractInvariants: %v", err)
	}

	var count int
	err = db.QueryRow(`SELECT COUNT(*) FROM invariants WHERE spec_id = ?`, specID).Scan(&count)
	if err != nil {
		t.Fatalf("count: %v", err)
	}
	if count != 1 {
		t.Errorf("expected 1 invariant flushed at EOF, got %d", count)
	}
}

// ---------------------------------------------------------------------------
// ExtractADRs
// ---------------------------------------------------------------------------

func TestExtractADRs_SingleComplete(t *testing.T) {
	db, specID, sfID := setupTestDB(t)
	secID := insertTestSection(t, db, specID, sfID)

	sections := []*SectionNode{
		{
			SectionPath:  "test-section",
			Title:        "Test Section",
			HeadingLevel: 2,
			LineStart:    0,
			LineEnd:      30,
			DBID:         secID,
		},
	}

	md := `## Test Section

### ADR-001: Use SQLite for Storage

#### Problem

We need a storage backend that works without external services.

#### Options

A) **SQLite**
- Pros: Zero-config, single file
- Cons: No concurrent writers

B) **PostgreSQL**
- Pros: Full ACID, concurrent
- Cons: Requires running server

#### Decision

**Option A: SQLite.** It provides everything we need without deployment complexity.

**Confidence: Committed**

#### Consequences

All data lives in a single file that can be copied or versioned.

#### Tests

Run the full test suite against an in-memory SQLite database.

---
`
	lines := strings.Split(md, "\n")

	err := ExtractADRs(lines, sections, specID, sfID, db)
	if err != nil {
		t.Fatalf("ExtractADRs: %v", err)
	}

	// Verify the ADR row
	var adrID, title, problem, decisionText, confidence string
	var chosenOption sql.NullString
	err = db.QueryRow(
		`SELECT adr_id, title, problem, decision_text, chosen_option, confidence
		 FROM adrs WHERE spec_id = ?`, specID,
	).Scan(&adrID, &title, &problem, &decisionText, &chosenOption, &confidence)
	if err != nil {
		t.Fatalf("query ADR: %v", err)
	}

	if adrID != "ADR-001" {
		t.Errorf("expected adr_id 'ADR-001', got %q", adrID)
	}
	if title != "Use SQLite for Storage" {
		t.Errorf("expected title 'Use SQLite for Storage', got %q", title)
	}
	if !strings.Contains(problem, "storage backend") {
		t.Errorf("expected problem containing 'storage backend', got %q", problem)
	}
	if !strings.Contains(decisionText, "Option A") {
		t.Errorf("expected decision_text containing 'Option A', got %q", decisionText)
	}
	if !chosenOption.Valid || !strings.Contains(chosenOption.String, "everything we need") {
		t.Errorf("expected chosen_option containing rationale, got %v", chosenOption)
	}
	if confidence != "Committed" {
		t.Errorf("expected confidence 'Committed', got %q", confidence)
	}

	// Verify the ADR options
	rows, err := db.Query(
		`SELECT option_label, option_name, is_chosen FROM adr_options
		 WHERE adr_id = (SELECT id FROM adrs WHERE spec_id = ? AND adr_id = 'ADR-001')
		 ORDER BY option_label`, specID)
	if err != nil {
		t.Fatalf("query options: %v", err)
	}
	defer rows.Close()

	type optRow struct {
		label, name string
		isChosen    int
	}
	var opts []optRow
	for rows.Next() {
		var o optRow
		if err := rows.Scan(&o.label, &o.name, &o.isChosen); err != nil {
			t.Fatalf("scan option: %v", err)
		}
		opts = append(opts, o)
	}

	if len(opts) != 2 {
		t.Fatalf("expected 2 options, got %d", len(opts))
	}
	if opts[0].label != "A" || opts[0].name != "SQLite" {
		t.Errorf("option A: got label=%q name=%q", opts[0].label, opts[0].name)
	}
	if opts[0].isChosen != 1 {
		t.Errorf("option A should be chosen")
	}
	if opts[1].label != "B" || opts[1].name != "PostgreSQL" {
		t.Errorf("option B: got label=%q name=%q", opts[1].label, opts[1].name)
	}
	if opts[1].isChosen != 0 {
		t.Errorf("option B should not be chosen")
	}
}

func TestExtractADRs_AppPrefixed(t *testing.T) {
	db, specID, sfID := setupTestDB(t)
	secID := insertTestSection(t, db, specID, sfID)

	sections := []*SectionNode{
		{
			SectionPath:  "test-section",
			Title:        "Test Section",
			HeadingLevel: 2,
			LineStart:    0,
			LineEnd:      20,
			DBID:         secID,
		},
	}

	md := `## Test Section

### APP-ADR-010: Application Decision

#### Problem

Application-level problem statement.

#### Decision

**Option A.** Chosen for simplicity.

---
`
	lines := strings.Split(md, "\n")

	err := ExtractADRs(lines, sections, specID, sfID, db)
	if err != nil {
		t.Fatalf("ExtractADRs: %v", err)
	}

	var adrID string
	err = db.QueryRow(
		`SELECT adr_id FROM adrs WHERE spec_id = ?`, specID,
	).Scan(&adrID)
	if err != nil {
		t.Fatalf("query: %v", err)
	}
	if adrID != "APP-ADR-010" {
		t.Errorf("expected 'APP-ADR-010', got %q", adrID)
	}
}

func TestExtractADRs_TwoConsecutive(t *testing.T) {
	db, specID, sfID := setupTestDB(t)
	secID := insertTestSection(t, db, specID, sfID)

	sections := []*SectionNode{
		{
			SectionPath:  "test-section",
			Title:        "Test Section",
			HeadingLevel: 2,
			LineStart:    0,
			LineEnd:      40,
			DBID:         secID,
		},
	}

	md := `## Test Section

### ADR-001: First Decision

#### Problem

First problem.

#### Decision

**Option A.** First choice.

---

### ADR-002: Second Decision

#### Problem

Second problem.

#### Decision

**Option B.** Second choice.

---
`
	lines := strings.Split(md, "\n")

	err := ExtractADRs(lines, sections, specID, sfID, db)
	if err != nil {
		t.Fatalf("ExtractADRs: %v", err)
	}

	var count int
	err = db.QueryRow(`SELECT COUNT(*) FROM adrs WHERE spec_id = ?`, specID).Scan(&count)
	if err != nil {
		t.Fatalf("count: %v", err)
	}
	if count != 2 {
		t.Errorf("expected 2 ADRs, got %d", count)
	}
}

func TestExtractADRs_NoADRs(t *testing.T) {
	db, specID, sfID := setupTestDB(t)

	lines := strings.Split("## Just a heading\n\nSome paragraph.\n", "\n")

	err := ExtractADRs(lines, nil, specID, sfID, db)
	if err != nil {
		t.Fatalf("ExtractADRs: %v", err)
	}

	var count int
	err = db.QueryRow(`SELECT COUNT(*) FROM adrs WHERE spec_id = ?`, specID).Scan(&count)
	if err != nil {
		t.Fatalf("count: %v", err)
	}
	if count != 0 {
		t.Errorf("expected 0 ADRs, got %d", count)
	}
}

func TestExtractADRs_FlushAtEOF(t *testing.T) {
	// ADR at end of file with no trailing --- should still be extracted.
	db, specID, sfID := setupTestDB(t)
	secID := insertTestSection(t, db, specID, sfID)

	sections := []*SectionNode{
		{
			SectionPath:  "test-section",
			Title:        "Test Section",
			HeadingLevel: 2,
			LineStart:    0,
			LineEnd:      20,
			DBID:         secID,
		},
	}

	md := `## Test Section

### ADR-005: EOF ADR

#### Problem

Problem at end of file.

#### Decision

**Option A.** Decision at end of file.`

	lines := strings.Split(md, "\n")

	err := ExtractADRs(lines, sections, specID, sfID, db)
	if err != nil {
		t.Fatalf("ExtractADRs: %v", err)
	}

	var count int
	err = db.QueryRow(`SELECT COUNT(*) FROM adrs WHERE spec_id = ?`, specID).Scan(&count)
	if err != nil {
		t.Fatalf("count: %v", err)
	}
	if count != 1 {
		t.Errorf("expected 1 ADR flushed at EOF, got %d", count)
	}
}

// ---------------------------------------------------------------------------
// Additional regex patterns
// ---------------------------------------------------------------------------

func TestViolationRe(t *testing.T) {
	input := "Violation scenario: Data is silently lost during migration."
	m := ViolationRe.FindStringSubmatch(input)
	if m == nil {
		t.Fatalf("expected ViolationRe to match %q", input)
	}
	if m[1] != "Data is silently lost during migration." {
		t.Errorf("expected violation text, got %q", m[1])
	}
}

func TestViolationRe_NoMatch(t *testing.T) {
	tests := []string{
		"Violation scenario (qualified): text",
		"Some other text",
		"",
	}
	for _, input := range tests {
		if m := ViolationRe.FindStringSubmatch(input); m != nil {
			t.Errorf("expected no match for %q, got %v", input, m)
		}
	}
}

func TestADRSubheadingRe(t *testing.T) {
	tests := []struct {
		input   string
		heading string
	}{
		{"#### Problem", "Problem"},
		{"#### Options", "Options"},
		{"#### Decision", "Decision"},
		{"#### Consequences", "Consequences"},
		{"#### Tests", "Tests"},
	}
	for _, tc := range tests {
		m := ADRSubheadingRe.FindStringSubmatch(tc.input)
		if m == nil {
			t.Errorf("expected ADRSubheadingRe to match %q", tc.input)
			continue
		}
		if m[1] != tc.heading {
			t.Errorf("for %q: expected heading %q, got %q", tc.input, tc.heading, m[1])
		}
	}
}

func TestADROptionRe(t *testing.T) {
	input := `A) **SQLite**`
	m := ADROptionRe.FindStringSubmatch(input)
	if m == nil {
		t.Fatalf("expected ADROptionRe to match %q", input)
	}
	if m[1] != "A" {
		t.Errorf("expected label 'A', got %q", m[1])
	}
	if m[2] != "SQLite" {
		t.Errorf("expected name 'SQLite', got %q", m[2])
	}
}

func TestHeadingRe(t *testing.T) {
	tests := []struct {
		input string
		level int
		title string
	}{
		{"# Top Level", 1, "Top Level"},
		{"## Second Level", 2, "Second Level"},
		{"### Third Level", 3, "Third Level"},
		{"###### Sixth Level", 6, "Sixth Level"},
	}
	for _, tc := range tests {
		m := HeadingRe.FindStringSubmatch(tc.input)
		if m == nil {
			t.Errorf("expected HeadingRe to match %q", tc.input)
			continue
		}
		if len(m[1]) != tc.level {
			t.Errorf("for %q: expected level %d, got %d", tc.input, tc.level, len(m[1]))
		}
		if strings.TrimSpace(m[2]) != tc.title {
			t.Errorf("for %q: expected title %q, got %q", tc.input, tc.title, m[2])
		}
	}
}

func TestGateRe(t *testing.T) {
	tests := []struct {
		input string
		id    string
		title string
	}{
		{"**Gate 1: Structural Validity**", "1", "Structural Validity"},
		{"**Gate M-3: Module Check**", "M-3", "Module Check"},
		{"**Gate 5**", "5", ""},
	}
	for _, tc := range tests {
		m := GateRe.FindStringSubmatch(tc.input)
		if m == nil {
			t.Errorf("expected GateRe to match %q", tc.input)
			continue
		}
		if m[1] != tc.id {
			t.Errorf("for %q: expected id %q, got %q", tc.input, tc.id, m[1])
		}
		if len(m) > 2 && m[2] != tc.title {
			t.Errorf("for %q: expected title %q, got %q", tc.input, tc.title, m[2])
		}
	}
}

// ---------------------------------------------------------------------------
// Circular spec dependency detection
// ---------------------------------------------------------------------------

func TestCircularSpecDependency(t *testing.T) {
	// Create two temporary manifest files that reference each other:
	//   A.yaml parent_spec: B.yaml
	//   B.yaml parent_spec: A.yaml
	// ParseModularSpec should detect the cycle and return an error,
	// not recurse infinitely.
	dir := t.TempDir()

	aPath := dir + "/a/manifest.yaml"
	bPath := dir + "/b/manifest.yaml"

	// Create directories
	if err := os.MkdirAll(dir+"/a", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(dir+"/b", 0o755); err != nil {
		t.Fatal(err)
	}

	// Write a minimal constitution file for each
	constContent := `# System Constitution

## System State

Minimal test constitution.
`
	if err := os.WriteFile(dir+"/a/system.md", []byte(constContent), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(dir+"/b/system.md", []byte(constContent), 0o644); err != nil {
		t.Fatal(err)
	}

	// A references B as parent
	aYAML := `ddis_version: "3.0"
spec_name: "Spec A"
tier_mode: "modular"
parent_spec: "../b/manifest.yaml"
constitution:
  system: "system.md"
modules: {}
`
	if err := os.WriteFile(aPath, []byte(aYAML), 0o644); err != nil {
		t.Fatal(err)
	}

	// B references A as parent
	bYAML := `ddis_version: "3.0"
spec_name: "Spec B"
tier_mode: "modular"
parent_spec: "../a/manifest.yaml"
constitution:
  system: "system.md"
modules: {}
`
	if err := os.WriteFile(bPath, []byte(bYAML), 0o644); err != nil {
		t.Fatal(err)
	}

	db := setupTestDBRaw(t)

	_, err := ParseModularSpec(aPath, db)
	if err == nil {
		t.Fatal("expected error for circular spec dependency, got nil")
	}
	if !strings.Contains(err.Error(), "circular spec dependency") {
		t.Errorf("expected 'circular spec dependency' error, got: %v", err)
	}
}
