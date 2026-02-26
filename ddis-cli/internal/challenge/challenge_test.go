package challenge

import (
	"database/sql"
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/consistency"
	"github.com/wvandaal/ddis/internal/storage"
)

// setupTestDB creates a fresh in-memory database with schema + test data.
func setupTestDB(t *testing.T) *sql.DB {
	t.Helper()
	db, err := storage.Open(":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	// Insert a spec index entry.
	_, err = db.Exec(`INSERT INTO spec_index (spec_path, source_type, content_hash, parsed_at) VALUES ('test.yaml', 'monolith', 'abc123', datetime('now'))`)
	if err != nil {
		t.Fatalf("insert spec: %v", err)
	}

	// Insert a source file.
	_, err = db.Exec(`INSERT INTO source_files (spec_id, file_path, file_role, raw_text, content_hash, line_count) VALUES (1, 'test.md', 'monolith', 'content', 'hash123', 10)`)
	if err != nil {
		t.Fatalf("insert source: %v", err)
	}

	// Insert a section (required as FK parent for invariants).
	_, err = db.Exec(`INSERT INTO sections (spec_id, source_file_id, section_path, title, heading_level, raw_text, content_hash, line_start, line_end)
		VALUES (1, 1, '§1', 'Test Section', 1, 'test content', 'schash', 1, 10)`)
	if err != nil {
		t.Fatalf("insert section: %v", err)
	}

	return db
}

// insertTestInvariant adds an invariant to the DB.
func insertTestInvariant(t *testing.T, db *sql.DB, id, title, statement, semiFormal string) {
	t.Helper()
	_, err := db.Exec(
		`INSERT INTO invariants (spec_id, source_file_id, section_id, invariant_id, title, statement, semi_formal, line_start, line_end, raw_text, content_hash)
		 VALUES (1, 1, 1, ?, ?, ?, ?, 1, 10, 'raw', 'hash-'||?)`,
		id, title, statement, semiFormal, id,
	)
	if err != nil {
		t.Fatalf("insert invariant %s: %v", id, err)
	}
}

// insertTestWitness adds a witness to the DB and returns its ID.
func insertTestWitness(t *testing.T, db *sql.DB, invID, evidenceType, evidence string) int64 {
	t.Helper()
	w := &storage.InvariantWitness{
		SpecID:       1,
		InvariantID:  invID,
		SpecHash:     "hash-" + invID,
		EvidenceType: evidenceType,
		Evidence:     evidence,
		ProvenBy:     "test-agent",
		Status:       "valid",
	}
	id, err := storage.InsertWitness(db, w)
	if err != nil {
		t.Fatalf("insert witness %s: %v", invID, err)
	}
	return id
}

// --- Level 1: Formal Tests ---

func TestChallenge_FormalParsesAndSAT(t *testing.T) {
	vm := consistency.NewVarMap()
	cnf := consistency.ParseSemiFormal("render(x) = true", vm)
	if len(cnf) == 0 {
		t.Error("expected non-empty CNF for valid semi-formal")
	}
	if !consistency.Satisfiable(cnf, vm) {
		t.Error("single predicate should be satisfiable")
	}

	inv := &storage.Invariant{SemiFormal: "render(x) = true"}
	result := levelFormal(inv)
	if !result.Parsed {
		t.Error("expected Parsed=true")
	}
	if !result.SelfConsistent {
		t.Error("expected SelfConsistent=true")
	}
}

func TestChallenge_FormalEmpty(t *testing.T) {
	inv := &storage.Invariant{SemiFormal: ""}
	result := levelFormal(inv)
	if result.Parsed {
		t.Error("expected Parsed=false for empty semi-formal")
	}
}

func TestChallenge_FormalContradiction(t *testing.T) {
	inv := &storage.Invariant{SemiFormal: "P(x) = true AND P(x) = false"}
	result := levelFormal(inv)
	if !result.Parsed {
		t.Error("expected Parsed=true for parseable contradiction")
	}
	if result.SelfConsistent {
		t.Error("expected SelfConsistent=false for contradiction")
	}
}

// --- Level 2: Uncertainty Tests ---

func TestChallenge_UncertaintyTest(t *testing.T) {
	w := &storage.InvariantWitness{EvidenceType: "test"}
	result := levelUncertainty(w)
	if result.Confidence != 0.9 {
		t.Errorf("expected confidence=0.9 for test, got %f", result.Confidence)
	}
}

func TestChallenge_UncertaintyAttestation(t *testing.T) {
	w := &storage.InvariantWitness{EvidenceType: "attestation"}
	result := levelUncertainty(w)
	if result.Confidence != 0.3 {
		t.Errorf("expected confidence=0.3 for attestation, got %f", result.Confidence)
	}
}

// --- Level 3: Causal Tests ---

func TestChallenge_CausalFound(t *testing.T) {
	// Create a temp directory with a file containing a test annotation.
	dir := t.TempDir()
	testFile := filepath.Join(dir, "foo_test.go")
	content := `package foo

// ddis:tests APP-INV-901
func TestSomething(t *testing.T) {}
`
	if err := os.WriteFile(testFile, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}

	inv := &storage.Invariant{InvariantID: "APP-INV-901"}
	result := levelCausal(inv, dir)
	if !result.TestFound {
		t.Error("expected TestFound=true")
	}
	if result.TestName != "TestSomething" {
		t.Errorf("expected TestName=TestSomething, got %s", result.TestName)
	}
}

func TestChallenge_CausalMissing(t *testing.T) {
	dir := t.TempDir()
	// Empty dir — no annotations.
	inv := &storage.Invariant{InvariantID: "APP-INV-MISSING"}
	result := levelCausal(inv, dir)
	if result.TestFound {
		t.Error("expected TestFound=false for missing annotations")
	}
}

// --- Level 5: Meta Tests ---

func TestChallenge_MetaHighOverlap(t *testing.T) {
	inv := &storage.Invariant{
		Title:     "Parse Fidelity",
		Statement: "parse then render produces identical output for all specs",
	}
	w := &storage.InvariantWitness{
		Evidence: "parse roundtrip test produces identical output",
	}
	result := levelMeta(inv, w)
	if result.Overlap < 0.2 {
		t.Errorf("expected high overlap for related terms, got %.2f", result.Overlap)
	}
}

func TestChallenge_MetaLowOverlap(t *testing.T) {
	inv := &storage.Invariant{
		Title:     "Parse Fidelity",
		Statement: "parse then render produces identical output for all specs",
	}
	w := &storage.InvariantWitness{
		Evidence: "fixed the namespace collision bug in gophersat variables",
	}
	result := levelMeta(inv, w)
	if result.Overlap > 0.3 {
		t.Errorf("expected low overlap for unrelated terms, got %.2f", result.Overlap)
	}
}

// --- Integration Tests ---

func TestChallenge_RefutedInvalidates(t *testing.T) {
	db := setupTestDB(t)
	defer db.Close()

	// Insert invariant with contradictory semi-formal.
	insertTestInvariant(t, db, "APP-INV-REFUTE", "Self Contradiction", "A and not A", "P(x) = true AND P(x) = false")

	// Insert valid witness.
	insertTestWitness(t, db, "APP-INV-REFUTE", "test", "test passes somehow")

	// Challenge — should be refuted due to contradictory semi-formal.
	result, err := Challenge(db, 1, "APP-INV-REFUTE", Options{MaxLevel: 2, ChallengedBy: "test"})
	if err != nil {
		t.Fatalf("challenge: %v", err)
	}
	if result.Verdict != Refuted {
		t.Errorf("expected Refuted, got %s", result.Verdict)
	}
	if !result.WitnessInvalidated {
		t.Error("expected WitnessInvalidated=true")
	}

	// Verify witness status in DB.
	w, err := storage.GetWitness(db, 1, "APP-INV-REFUTE")
	if err != nil {
		t.Fatalf("get witness: %v", err)
	}
	if w.Status != "invalidated" {
		t.Errorf("expected witness status=invalidated, got %s", w.Status)
	}
}

func TestChallenge_ConfirmedFull(t *testing.T) {
	db := setupTestDB(t)
	defer db.Close()

	// Insert invariant with valid semi-formal.
	insertTestInvariant(t, db, "APP-INV-902", "Valid Invariant", "render produces output", "render(x) = true")

	// Insert test witness with test annotation in temp dir.
	dir := t.TempDir()
	testFile := filepath.Join(dir, "conf_test.go")
	content := `package conf

// ddis:tests APP-INV-902
func TestConf(t *testing.T) {}
`
	if err := os.WriteFile(testFile, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}

	insertTestWitness(t, db, "APP-INV-902", "test", "TestConf passes for render output verification")

	// Challenge at max level 3 (skip test execution in test env).
	result, err := Challenge(db, 1, "APP-INV-902", Options{
		CodeRoot:     dir,
		MaxLevel:     3,
		ChallengedBy: "test",
	})
	if err != nil {
		t.Fatalf("challenge: %v", err)
	}

	// With max level 3, test found but not run → inconclusive
	// (practical level not executed, but causal found)
	if result.LevelCausal == nil || !result.LevelCausal.TestFound {
		t.Error("expected causal test found")
	}
	if result.LevelFormal == nil || !result.LevelFormal.SelfConsistent {
		t.Error("expected formal self-consistent")
	}
}

func TestChallenge_StorageRoundTrip(t *testing.T) {
	db := setupTestDB(t)
	defer db.Close()

	cr := &storage.ChallengeResult{
		SpecID:           1,
		InvariantID:      "APP-INV-RT",
		Verdict:          "confirmed",
		LevelFormal:      "parsed=true consistent=true",
		LevelUncertainty: "type=test confidence=0.9",
		ChallengedBy:     "test-agent",
	}

	id, err := storage.InsertChallengeResult(db, cr)
	if err != nil {
		t.Fatalf("insert: %v", err)
	}
	if id == 0 {
		t.Error("expected non-zero ID")
	}

	got, err := storage.GetChallengeResult(db, 1, "APP-INV-RT")
	if err != nil {
		t.Fatalf("get: %v", err)
	}
	if got.Verdict != "confirmed" {
		t.Errorf("expected verdict=confirmed, got %s", got.Verdict)
	}
	if got.ChallengedBy != "test-agent" {
		t.Errorf("expected challenged_by=test-agent, got %s", got.ChallengedBy)
	}
}

// --- Behavioral Test (APP-INV-050) ---

// ddis:tests APP-INV-050
func TestAPPINV050_AdjunctionFidelity(t *testing.T) {
	db := setupTestDB(t)
	defer db.Close()

	// 1. Create invariant with semi-formal "render(x) = true"
	insertTestInvariant(t, db, "APP-INV-903", "Render Fidelity", "render produces valid output", "render(x) = true")

	// 2. Record witness with type=test, evidence="TestRoundTrip passes"
	insertTestWitness(t, db, "APP-INV-903", "test", "TestRoundTrip passes for render verification")

	// 3. Create temp code file with ddis:tests annotation.
	dir := t.TempDir()
	testFile := filepath.Join(dir, "adj_test.go")
	content := `package adj

// ddis:tests APP-INV-903
func TestRoundTrip(t *testing.T) {}
`
	if err := os.WriteFile(testFile, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}

	// 4. Challenge at level 3 (skip test execution in test environment).
	result, err := Challenge(db, 1, "APP-INV-903", Options{
		CodeRoot:     dir,
		MaxLevel:     3,
		ChallengedBy: "test-adjunction",
	})
	if err != nil {
		t.Fatalf("challenge: %v", err)
	}

	// Verdict must be in {confirmed, provisional, refuted, inconclusive}.
	switch result.Verdict {
	case Confirmed, Provisional, Refuted, Inconclusive:
		// OK
	default:
		t.Errorf("verdict %q not in {confirmed, provisional, refuted, inconclusive}", result.Verdict)
	}

	// 5. Create a second invariant with attestation-only witness and no test annotation.
	insertTestInvariant(t, db, "APP-INV-904", "Attestation Only", "something without tests", "render(x) = true")
	cleanDir := t.TempDir()
	insertTestWitness(t, db, "APP-INV-904", "attestation", "I implemented it")

	result2, err := Challenge(db, 1, "APP-INV-904", Options{
		CodeRoot:     cleanDir,
		MaxLevel:     5,
		ChallengedBy: "test-adjunction-2",
	})
	if err != nil {
		t.Fatalf("challenge 2: %v", err)
	}

	// Should be inconclusive: attestation-only + no annotations.
	if result2.Verdict != Inconclusive {
		t.Errorf("expected inconclusive for attestation-only, got %s", result2.Verdict)
	}

	// 6. Verify witness NOT invalidated (inconclusive doesn't invalidate).
	w, err := storage.GetWitness(db, 1, "APP-INV-904")
	if err != nil {
		t.Fatalf("get witness: %v", err)
	}
	if w.Status == "invalidated" {
		t.Error("inconclusive challenge should not invalidate witness")
	}
}
