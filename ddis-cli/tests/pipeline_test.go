package tests

// ddis:tests APP-INV-040 (progressive validation monotonicity — full pipeline coherence)

import (
	"database/sql"
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/coverage"
	"github.com/wvandaal/ddis/internal/drift"
	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/validator"
)

// TestEndToEndPipeline exercises the full parse → validate → coverage → drift pipeline
// on a synthetic spec fixture. This verifies pipeline coherence: each stage's output
// is consistent with the previous stage's state.
func TestEndToEndPipeline(t *testing.T) {
	dir := t.TempDir()
	specPath := filepath.Join(dir, "pipeline_spec.md")

	// Write a minimal but valid DDIS spec
	spec := `# Pipeline Test Spec

## §1 Core Requirements

### §1.1 Data Integrity

**INV-001: Data Persistence**
*All parsed elements are persisted in the database with no data loss.*

Violation scenario: An element present in the source markdown is absent from the database after parsing.

Validation: Parse a document, then query each element type (sections, invariants, ADRs) and verify counts match expectations.

// WHY THIS MATTERS: Data loss during parsing renders the tool unreliable.

**INV-002: Deterministic Output**
*Given the same input, the parser produces byte-identical output every time.*

Violation scenario: Running the parser twice on identical input produces different database content.

Validation: Parse the same document twice, compare content hashes.

// WHY THIS MATTERS: Non-determinism breaks reproducibility.

### ADR-001: Embedded Database

#### Problem
Need persistent storage for parsed spec elements.

#### Options
A) **SQLite** — embedded, zero-config, single binary
- Pros: No external dependencies, ACID transactions
- Cons: Single-writer limitation

B) **PostgreSQL** — full RDBMS
- Pros: Concurrent writes
- Cons: External dependency, complex deployment

#### Decision
**Option A.** SQLite is sufficient for single-user specification management.

#### Consequences
Single binary distribution. No concurrent write access needed for spec tooling.

#### Tests
Verify database creation and table population after parse.

## §2 Validation

**Gate 1: Structural Conformance**

All sections, invariants, and ADRs must follow the prescribed format.

**DO NOT** skip validation checks in any code path.
**DO NOT** modify validation results after computation.
**DO NOT** allow parse without subsequent validation.
`
	if err := os.WriteFile(specPath, []byte(spec), 0644); err != nil {
		t.Fatalf("write spec: %v", err)
	}

	dbPath := filepath.Join(dir, "pipeline.ddis.db")

	// ── Phase 1: Parse ──────────────────────────────────────────
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	// Verify parse produced elements
	assertCountGt(t, db, specID, "sections", 0)
	assertCountGt(t, db, specID, "invariants", 0)
	assertCountGt(t, db, specID, "adrs", 0)
	assertCountGt(t, db, specID, "negative_specs", 0)

	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		t.Fatalf("list invariants: %v", err)
	}
	if len(invs) != 2 {
		t.Fatalf("expected 2 invariants, got %d", len(invs))
	}

	// ── Phase 2: Build Search Index ─────────────────────────────
	if err := search.BuildIndex(db, specID); err != nil {
		t.Fatalf("build search index: %v", err)
	}

	// Verify search index populated
	var ftsCount int
	if err := db.QueryRow("SELECT COUNT(*) FROM fts_index").Scan(&ftsCount); err != nil {
		t.Fatalf("count FTS: %v", err)
	}
	if ftsCount == 0 {
		t.Error("FTS index empty after build")
	}

	// ── Phase 3: Validate ───────────────────────────────────────
	report, err := validator.Validate(db, specID, validator.ValidateOptions{})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if report.TotalChecks == 0 {
		t.Error("validation ran 0 checks")
	}

	// Log check results for visibility
	for _, r := range report.Results {
		status := "PASS"
		if !r.Passed {
			status = "FAIL"
		}
		t.Logf("  Check %d (%s): %s — %s", r.CheckID, r.CheckName, status, r.Summary)
	}

	// ── Phase 4: Coverage ───────────────────────────────────────
	covResult, err := coverage.Analyze(db, specID, coverage.Options{})
	if err != nil {
		t.Fatalf("coverage: %v", err)
	}

	if covResult.Summary.InvariantsTotal != 2 {
		t.Errorf("coverage total invariants = %d, want 2", covResult.Summary.InvariantsTotal)
	}
	if covResult.Summary.ADRsTotal != 1 {
		t.Errorf("coverage total ADRs = %d, want 1", covResult.Summary.ADRsTotal)
	}

	// ── Phase 5: Drift ──────────────────────────────────────────
	driftResult, err := drift.Analyze(db, specID, drift.Options{})
	if err != nil {
		t.Fatalf("drift: %v", err)
	}

	// A freshly-parsed spec should have 0 effective drift
	if driftResult.EffectiveDrift != 0 {
		t.Errorf("effective drift = %d, want 0 for freshly parsed spec", driftResult.EffectiveDrift)
	}

	// ── Phase 6: Progressive Validation Levels (APP-INV-040) ──────
	// Level 1 subset ⊂ Level 2 subset ⊂ Level 3 (all)
	l1Checks := []int{10, 12}
	l2Checks := []int{1, 2, 4, 5, 7, 8, 10, 12}

	r1, err := validator.Validate(db, specID, validator.ValidateOptions{CheckIDs: l1Checks})
	if err != nil {
		t.Fatalf("validate L1: %v", err)
	}
	r2, err := validator.Validate(db, specID, validator.ValidateOptions{CheckIDs: l2Checks})
	if err != nil {
		t.Fatalf("validate L2: %v", err)
	}

	if len(r1.Results) > len(r2.Results) {
		t.Errorf("L1 checks (%d) > L2 checks (%d) — violates monotonicity",
			len(r1.Results), len(r2.Results))
	}

	// Verify L1 checks are a subset of L2 checks
	l2Set := make(map[int]bool)
	for _, r := range r2.Results {
		l2Set[r.CheckID] = true
	}
	for _, r := range r1.Results {
		if !l2Set[r.CheckID] {
			t.Errorf("L1 check %d not in L2 — violates subset property", r.CheckID)
		}
	}

	// ── Phase 7: Idempotent Re-parse ────────────────────────────
	// Re-parsing should produce equivalent results (APP-INV-009 spirit)
	specID2, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("re-parse: %v", err)
	}

	invs2, err := storage.ListInvariants(db, specID2)
	if err != nil {
		t.Fatalf("list invariants after re-parse: %v", err)
	}
	if len(invs2) != len(invs) {
		t.Errorf("re-parse invariant count = %d, want %d", len(invs2), len(invs))
	}
}

// assertCountGt checks that the count of rows for a given table and spec_id is > min.
func assertCountGt(t *testing.T, db *sql.DB, specID int64, table string, min int) {
	t.Helper()
	var count int
	err := db.QueryRow("SELECT COUNT(*) FROM "+table+" WHERE spec_id = ?", specID).Scan(&count)
	if err != nil {
		t.Fatalf("count %s: %v", table, err)
	}
	if count <= min {
		t.Errorf("%s count = %d, want > %d", table, count, min)
	}
}

