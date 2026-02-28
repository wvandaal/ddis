package materialize

// ddis:tests APP-INV-093 (snapshot creation determinism — StateHash is deterministic)
// ddis:tests APP-INV-096 (pipeline round-trip preservation — StructuralDiff detects differences)
// ddis:tests APP-ADR-067 (structural equivalence definition)

import (
	"database/sql"
	"testing"

	_ "modernc.org/sqlite"
)

func createTestDB(t *testing.T) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("open memory db: %v", err)
	}

	// Minimal schema for content tables
	stmts := []string{
		`CREATE TABLE spec_index (id INTEGER PRIMARY KEY, spec_name TEXT, spec_type TEXT, source_path TEXT, parsed_at TEXT)`,
		`CREATE TABLE source_files (id INTEGER PRIMARY KEY, spec_id INTEGER, file_path TEXT, line_count INTEGER, content_hash TEXT)`,
		`CREATE TABLE invariants (id INTEGER PRIMARY KEY, spec_id INTEGER, source_file_id INTEGER, section_id INTEGER, invariant_id TEXT, title TEXT, statement TEXT, semi_formal TEXT, violation_scenario TEXT, validation_method TEXT, why_this_matters TEXT, line_start INTEGER, line_end INTEGER, raw_text TEXT, content_hash TEXT, UNIQUE(spec_id, invariant_id))`,
		`CREATE TABLE adrs (id INTEGER PRIMARY KEY, spec_id INTEGER, source_file_id INTEGER, section_id INTEGER, adr_id TEXT, title TEXT, problem TEXT, decision_text TEXT, chosen_option TEXT, consequences TEXT, tests TEXT, confidence TEXT, status TEXT DEFAULT 'active', superseded_by TEXT, line_start INTEGER, line_end INTEGER, raw_text TEXT, content_hash TEXT, UNIQUE(spec_id, adr_id))`,
		`CREATE TABLE sections (id INTEGER PRIMARY KEY, spec_id INTEGER, source_file_id INTEGER, section_path TEXT, title TEXT, heading_level INTEGER, line_start INTEGER, line_end INTEGER, raw_text TEXT, content_hash TEXT, UNIQUE(spec_id, source_file_id, section_path))`,
		`CREATE TABLE modules (id INTEGER PRIMARY KEY, spec_id INTEGER, source_file_id INTEGER, module_name TEXT, domain TEXT, line_count INTEGER, UNIQUE(spec_id, module_name))`,
		`CREATE TABLE glossary_entries (id INTEGER PRIMARY KEY, spec_id INTEGER, section_id INTEGER, term TEXT, definition TEXT, line_number INTEGER, UNIQUE(spec_id, term))`,
		`CREATE TABLE quality_gates (id INTEGER PRIMARY KEY, spec_id INTEGER, section_id INTEGER, gate_id TEXT, title TEXT, predicate TEXT, is_modular INTEGER, line_start INTEGER, line_end INTEGER, raw_text TEXT, UNIQUE(spec_id, gate_id))`,
		`CREATE TABLE negative_specs (id INTEGER PRIMARY KEY, spec_id INTEGER, source_file_id INTEGER, section_id INTEGER, constraint_text TEXT, reason TEXT, line_number INTEGER, raw_text TEXT)`,
		`CREATE TABLE cross_references (id INTEGER PRIMARY KEY, spec_id INTEGER, source_file_id INTEGER, source_line INTEGER, ref_type TEXT, ref_target TEXT, ref_text TEXT, resolved INTEGER)`,
	}
	for _, s := range stmts {
		if _, err := db.Exec(s); err != nil {
			t.Fatalf("create table: %v\n%s", err, s)
		}
	}
	return db
}

func seedContent(t *testing.T, db *sql.DB, specID int64) {
	t.Helper()
	db.Exec(`INSERT INTO invariants (spec_id, source_file_id, section_id, invariant_id, title, statement, semi_formal, violation_scenario, validation_method, why_this_matters, line_start, line_end, raw_text, content_hash)
		VALUES (?, 1, 0, 'APP-INV-071', 'Log Canonicality', 'JSONL is the canonical record', 'forall s: canonical(s) => JSONL', 'Direct SQL write bypasses log', 'Verify no direct SQL writes', 'Ensures single source of truth', 0, 0, '', '')`, specID)
	db.Exec(`INSERT INTO invariants (spec_id, source_file_id, section_id, invariant_id, title, statement, semi_formal, violation_scenario, validation_method, why_this_matters, line_start, line_end, raw_text, content_hash)
		VALUES (?, 1, 0, 'APP-INV-073', 'Fold Determinism', 'Same events produce same state', 'f(e1..en) = f(e1..en)', 'Non-deterministic fold', 'Run fold twice, compare', 'Replay reliability', 0, 0, '', '')`, specID)
	db.Exec(`INSERT INTO adrs (spec_id, source_file_id, section_id, adr_id, title, problem, decision_text, chosen_option, consequences, tests, status, line_start, line_end, raw_text, content_hash)
		VALUES (?, 1, 0, 'APP-ADR-058', 'JSONL as Canonical', 'Need a canonical record format', 'Use JSONL', 'JSONL', 'Append-only, human-readable', 'Verify round-trip', 'active', 0, 0, '', '')`, specID)
	db.Exec(`INSERT INTO sections (spec_id, source_file_id, section_path, title, heading_level, line_start, line_end, raw_text, content_hash)
		VALUES (?, 1, '1', 'Introduction', 1, 0, 0, '', '')`, specID)
	db.Exec(`INSERT INTO sections (spec_id, source_file_id, section_path, title, heading_level, line_start, line_end, raw_text, content_hash)
		VALUES (?, 1, '1.1', 'Overview', 2, 0, 0, '', '')`, specID)
	db.Exec(`INSERT INTO modules (spec_id, source_file_id, module_name, domain, line_count)
		VALUES (?, 1, 'event-sourcing', 'eventsourcing', 0)`, specID)
	db.Exec(`INSERT INTO glossary_entries (spec_id, section_id, term, definition, line_number)
		VALUES (?, 0, 'Fold', 'Deterministic replay of event sequence into state', 0)`, specID)
	db.Exec(`INSERT INTO quality_gates (spec_id, section_id, gate_id, title, predicate, is_modular, line_start, line_end, raw_text)
		VALUES (?, 0, 'APP-G-1', 'Structural Conformance', 'All sections present', 0, 0, 0, '')`, specID)
	db.Exec(`INSERT INTO negative_specs (spec_id, source_file_id, section_id, constraint_text, reason, line_number, raw_text)
		VALUES (?, 1, 0, 'DO NOT write directly to SQLite', 'Bypasses event log', 0, '')`, specID)
	db.Exec(`INSERT INTO cross_references (spec_id, source_file_id, source_line, ref_type, ref_target, ref_text, resolved)
		VALUES (?, 1, 0, 'invariant', 'APP-INV-071', 'See APP-INV-071', 1)`, specID)
}

func TestStateHash_Determinism(t *testing.T) {
	// APP-INV-093: same state → same hash, always
	db := createTestDB(t)
	defer db.Close()

	seedContent(t, db, 1)

	h1, err := StateHash(db, 1)
	if err != nil {
		t.Fatalf("StateHash run 1: %v", err)
	}
	h2, err := StateHash(db, 1)
	if err != nil {
		t.Fatalf("StateHash run 2: %v", err)
	}

	if h1 != h2 {
		t.Errorf("StateHash not deterministic: %s vs %s", h1, h2)
	}
	if len(h1) != 64 {
		t.Errorf("expected 64-char hex SHA-256, got %d chars: %s", len(h1), h1)
	}
}

func TestStateHash_Sensitivity(t *testing.T) {
	// Changing content must change hash
	db := createTestDB(t)
	defer db.Close()

	seedContent(t, db, 1)

	h1, _ := StateHash(db, 1)

	// Modify an invariant's statement
	db.Exec(`UPDATE invariants SET statement = 'MODIFIED' WHERE spec_id = 1 AND invariant_id = 'APP-INV-071'`)

	h2, _ := StateHash(db, 1)

	if h1 == h2 {
		t.Error("StateHash did not change after content modification")
	}
}

func TestStateHash_MetadataExclusion(t *testing.T) {
	// APP-ADR-067: changing raw_text or content_hash should NOT affect state hash
	db := createTestDB(t)
	defer db.Close()

	seedContent(t, db, 1)

	h1, _ := StateHash(db, 1)

	// Modify metadata-only fields
	db.Exec(`UPDATE invariants SET raw_text = 'changed raw', content_hash = 'abc123' WHERE spec_id = 1`)
	db.Exec(`UPDATE sections SET raw_text = 'changed section raw' WHERE spec_id = 1`)

	h2, _ := StateHash(db, 1)

	if h1 != h2 {
		t.Errorf("StateHash changed on metadata-only modification: %s vs %s", h1, h2)
	}
}

func TestStateHash_EmptyDB(t *testing.T) {
	db := createTestDB(t)
	defer db.Close()

	h, err := StateHash(db, 999)
	if err != nil {
		t.Fatalf("StateHash empty: %v", err)
	}
	if len(h) != 64 {
		t.Errorf("expected 64-char hex, got %d: %s", len(h), h)
	}
}

func TestStateHash_DifferentSpecIDs(t *testing.T) {
	db := createTestDB(t)
	defer db.Close()

	seedContent(t, db, 1)
	seedContent(t, db, 2)

	// Same content under different spec_ids → same hash
	h1, _ := StateHash(db, 1)
	h2, _ := StateHash(db, 2)

	if h1 != h2 {
		t.Errorf("same content under different spec_ids produced different hashes: %s vs %s", h1, h2)
	}
}

func TestStructuralDiff_Identical(t *testing.T) {
	db1 := createTestDB(t)
	defer db1.Close()
	db2 := createTestDB(t)
	defer db2.Close()

	seedContent(t, db1, 1)
	seedContent(t, db2, 1)

	diffs := StructuralDiff(db1, db2, 1, 1)
	if len(diffs) != 0 {
		t.Errorf("expected 0 diffs for identical DBs, got %d:\n%s", len(diffs), FormatDiffs(diffs))
	}
}

func TestStructuralDiff_ModifiedInvariant(t *testing.T) {
	db1 := createTestDB(t)
	defer db1.Close()
	db2 := createTestDB(t)
	defer db2.Close()

	seedContent(t, db1, 1)
	seedContent(t, db2, 1)

	// Modify one invariant in db2
	db2.Exec(`UPDATE invariants SET title = 'Changed Title' WHERE invariant_id = 'APP-INV-071'`)

	diffs := StructuralDiff(db1, db2, 1, 1)
	found := false
	for _, d := range diffs {
		if d.Table == "invariants" && d.Key == "APP-INV-071" && d.Field == "title" {
			found = true
			if d.Left != "Log Canonicality" || d.Right != "Changed Title" {
				t.Errorf("unexpected diff values: %q → %q", d.Left, d.Right)
			}
		}
	}
	if !found {
		t.Errorf("expected invariant title diff, got: %s", FormatDiffs(diffs))
	}
}

func TestStructuralDiff_AddedRemoved(t *testing.T) {
	db1 := createTestDB(t)
	defer db1.Close()
	db2 := createTestDB(t)
	defer db2.Close()

	seedContent(t, db1, 1)
	seedContent(t, db2, 1)

	// Add extra invariant to db2
	db2.Exec(`INSERT INTO invariants (spec_id, source_file_id, section_id, invariant_id, title, statement, line_start, line_end, raw_text, content_hash)
		VALUES (1, 1, 0, 'APP-INV-099', 'New', 'New statement', 0, 0, '', '')`)

	// Remove a section from db2
	db2.Exec(`DELETE FROM sections WHERE section_path = '1.1'`)

	diffs := StructuralDiff(db1, db2, 1, 1)

	var foundAdded, foundRemoved bool
	for _, d := range diffs {
		if d.Table == "invariants" && d.Key == "APP-INV-099" && d.Kind == "added" {
			foundAdded = true
		}
		if d.Table == "sections" && d.Key == "1.1" && d.Kind == "removed" {
			foundRemoved = true
		}
	}
	if !foundAdded {
		t.Error("expected added invariant APP-INV-099")
	}
	if !foundRemoved {
		t.Error("expected removed section 1.1")
	}
}

func TestStructuralDiff_AllTables(t *testing.T) {
	// Verify diff covers all 8 content tables
	db1 := createTestDB(t)
	defer db1.Close()
	db2 := createTestDB(t)
	defer db2.Close()

	seedContent(t, db1, 1)
	// db2 is empty — should detect all as "removed"

	diffs := StructuralDiff(db1, db2, 1, 1)

	tables := make(map[string]bool)
	for _, d := range diffs {
		tables[d.Table] = true
	}

	expected := []string{"invariants", "adrs", "sections", "modules", "glossary_entries", "quality_gates", "negative_specs", "cross_references"}
	for _, tbl := range expected {
		if !tables[tbl] {
			t.Errorf("missing diff for table %s", tbl)
		}
	}
}

func TestFormatDiffs_Empty(t *testing.T) {
	result := FormatDiffs(nil)
	if result != "No structural differences" {
		t.Errorf("unexpected: %s", result)
	}
}

func TestFormatDiffs_NonEmpty(t *testing.T) {
	diffs := []Difference{
		{Table: "invariants", Key: "INV-1", Kind: "modified", Field: "title", Left: "old", Right: "new"},
		{Table: "sections", Key: "1.1", Kind: "removed"},
	}
	result := FormatDiffs(diffs)
	if len(result) == 0 {
		t.Error("expected non-empty format")
	}
}
