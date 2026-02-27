package process

import (
	"database/sql"
	"math"
	"os"
	"path/filepath"
	"testing"

	_ "modernc.org/sqlite"
)

// setupTestDB creates a minimal in-memory DB with schema for testing.
func setupTestDB(t *testing.T) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}

	// Create minimal schema needed for compliance computation
	stmts := []string{
		`CREATE TABLE spec_index (id INTEGER PRIMARY KEY, spec_path TEXT, source_type TEXT, content_hash TEXT)`,
		`INSERT INTO spec_index (id, spec_path, source_type, content_hash) VALUES (1, 'test.md', 'monolith', 'abc123')`,
		`CREATE TABLE invariants (
			id INTEGER PRIMARY KEY, spec_id INTEGER,
			source_file_id INTEGER DEFAULT 0, section_id INTEGER DEFAULT 0,
			invariant_id TEXT, title TEXT, statement TEXT, semi_formal TEXT,
			violation_scenario TEXT, validation_method TEXT,
			why_this_matters TEXT, conditional_tag TEXT,
			line_start INTEGER DEFAULT 0, line_end INTEGER DEFAULT 0,
			raw_text TEXT DEFAULT '', content_hash TEXT)`,
		`CREATE TABLE invariant_witnesses (
			id INTEGER PRIMARY KEY, spec_id INTEGER, invariant_id TEXT,
			spec_hash TEXT, code_hash TEXT, evidence_type TEXT,
			evidence TEXT, proven_by TEXT, model TEXT,
			proven_at TEXT DEFAULT (datetime('now')),
			status TEXT DEFAULT 'valid', notes TEXT,
			UNIQUE(spec_id, invariant_id))`,
	}

	for _, stmt := range stmts {
		if _, err := db.Exec(stmt); err != nil {
			t.Fatalf("setup: %s: %v", stmt[:40], err)
		}
	}

	return db
}

func insertInvariant(t *testing.T, db *sql.DB, invID, title string) {
	t.Helper()
	_, err := db.Exec(
		`INSERT INTO invariants (spec_id, source_file_id, section_id, invariant_id, title, statement, semi_formal, violation_scenario, validation_method, why_this_matters, content_hash, raw_text)
		 VALUES (1, 0, 0, ?, ?, 'test stmt', 'test sf', 'test vs', 'test vm', 'test why', 'hash-' || ?, '')`,
		invID, title, invID)
	if err != nil {
		t.Fatal(err)
	}
}

func insertWitness(t *testing.T, db *sql.DB, invID, status string) {
	t.Helper()
	_, err := db.Exec(
		`INSERT OR REPLACE INTO invariant_witnesses (spec_id, invariant_id, spec_hash, evidence_type, evidence, proven_by, status)
		 VALUES (1, ?, 'hash-' || ?, 'attestation', 'test evidence', 'test-agent', ?)`,
		invID, invID, status)
	if err != nil {
		t.Fatal(err)
	}
}

func TestCompute_DBOnly(t *testing.T) {
	db := setupTestDB(t)
	defer db.Close()

	// Add 4 invariants, no witnesses
	for _, id := range []string{"INV-001", "INV-002", "INV-003", "INV-004"} {
		insertInvariant(t, db, id, "Test "+id)
	}

	info := Compute(db, 1, Options{})

	// DB only: spec_first, tool_usage, validation_gate all degrade to 0.5
	// witness_coverage = 0 (no witnesses)
	if info.SpecFirstRatio != 0.5 {
		t.Errorf("expected spec_first_ratio=0.5, got %f", info.SpecFirstRatio)
	}
	if info.ToolUsage != 0.5 {
		t.Errorf("expected tool_usage=0.5, got %f", info.ToolUsage)
	}
	if info.WitnessCoverage != 0.0 {
		t.Errorf("expected witness_coverage=0.0, got %f", info.WitnessCoverage)
	}
	if info.ValidationGate != 0.5 {
		t.Errorf("expected validation_gate=0.5, got %f", info.ValidationGate)
	}

	// Score = 0.35*0.5 + 0.20*0.5 + 0.25*0.0 + 0.20*0.5 = 0.375
	expected := 0.375
	if math.Abs(info.Score-expected) > 0.001 {
		t.Errorf("expected score=%f, got %f", expected, info.Score)
	}

	// Should have 3 degraded signals
	if len(info.Degraded) != 3 {
		t.Errorf("expected 3 degraded signals, got %d: %v", len(info.Degraded), info.Degraded)
	}
}

func TestCompute_FullWitnesses(t *testing.T) {
	db := setupTestDB(t)
	defer db.Close()

	// Add 4 invariants with valid witnesses
	for _, id := range []string{"INV-001", "INV-002", "INV-003", "INV-004"} {
		insertInvariant(t, db, id, "Test "+id)
		insertWitness(t, db, id, "valid")
	}

	info := Compute(db, 1, Options{})

	// witness_coverage should be 1.0
	if info.WitnessCoverage != 1.0 {
		t.Errorf("expected witness_coverage=1.0, got %f", info.WitnessCoverage)
	}

	// Score = 0.35*0.5 + 0.20*0.5 + 0.25*1.0 + 0.20*0.5 = 0.625
	expected := 0.625
	if math.Abs(info.Score-expected) > 0.001 {
		t.Errorf("expected score=%f, got %f", expected, info.Score)
	}
}

func TestCompute_PartialWitnesses(t *testing.T) {
	db := setupTestDB(t)
	defer db.Close()

	// 4 invariants: 2 valid witnesses, 1 stale, 1 missing
	for _, id := range []string{"INV-001", "INV-002", "INV-003", "INV-004"} {
		insertInvariant(t, db, id, "Test "+id)
	}
	insertWitness(t, db, "INV-001", "valid")
	insertWitness(t, db, "INV-002", "valid")
	insertWitness(t, db, "INV-003", "stale_spec")
	// INV-004: missing

	info := Compute(db, 1, Options{})

	// witness_coverage = (1.0 + 1.0 + 0.25 + 0.0) / 4 = 0.5625
	expected := 0.5625
	if math.Abs(info.WitnessCoverage-expected) > 0.001 {
		t.Errorf("expected witness_coverage=%f, got %f", expected, info.WitnessCoverage)
	}
}

func TestCompute_NoInvariants(t *testing.T) {
	db := setupTestDB(t)
	defer db.Close()

	// No invariants at all
	info := Compute(db, 1, Options{})

	if info.WitnessCoverage != 0.0 {
		t.Errorf("expected witness_coverage=0.0 with no invariants, got %f", info.WitnessCoverage)
	}
}

func TestCompute_EventStreams(t *testing.T) {
	// Create temp dir with event streams
	dir := t.TempDir()
	eventsDir := filepath.Join(dir, ".ddis", "events")
	if err := os.MkdirAll(eventsDir, 0o755); err != nil {
		t.Fatal(err)
	}

	// Write threads.jsonl with some threads
	threads := `{"id":"t-001","status":"active","summary":"test thread","event_count":3}
{"id":"t-002","status":"active","summary":"another thread","event_count":1}
{"id":"t-003","status":"active","summary":"third thread","event_count":0}
`
	if err := os.WriteFile(filepath.Join(eventsDir, "threads.jsonl"), []byte(threads), 0o644); err != nil {
		t.Fatal(err)
	}

	db := setupTestDB(t)
	defer db.Close()
	insertInvariant(t, db, "INV-001", "Test")

	info := Compute(db, 1, Options{CodeRoot: dir})

	// Event stream usage should be > 0 (3 threads found)
	if info.ToolUsage <= 0.0 {
		t.Errorf("expected tool_usage > 0 with event streams, got %f", info.ToolUsage)
	}
}

func TestGenerateRecommendation_Healthy(t *testing.T) {
	info := &Info{
		Score:           0.9,
		SpecFirstRatio:  0.9,
		ToolUsage:       0.8,
		WitnessCoverage: 1.0,
		ValidationGate:  1.0,
	}
	rec := generateRecommendation(info)
	if rec != "Process compliance is healthy." {
		t.Errorf("expected healthy recommendation, got: %s", rec)
	}
}

func TestGenerateRecommendation_LowWitness(t *testing.T) {
	info := &Info{
		Score:           0.4,
		SpecFirstRatio:  0.5,
		ToolUsage:       0.5,
		WitnessCoverage: 0.1,
		ValidationGate:  0.5,
		Degraded:        []string{"spec_first_ratio (no git)", "tool_usage (no oplog)", "validation_gate (no oplog)"},
	}
	rec := generateRecommendation(info)
	if rec == "Process compliance is healthy." {
		t.Errorf("should not report healthy with 10%% witness coverage")
	}
	// With spec/tool/validate degraded, witness is the only non-degraded sub-score
	if rec == "" {
		t.Error("expected a recommendation string")
	}
}

func TestCompute_Weights(t *testing.T) {
	// Verify weights sum to 1.0
	sum := WeightSpec + WeightTool + WeightWitness + WeightValidate
	if math.Abs(sum-1.0) > 0.001 {
		t.Errorf("weights should sum to 1.0, got %f", sum)
	}
}
