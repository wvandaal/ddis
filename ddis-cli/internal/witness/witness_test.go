package witness_test

import (
	"database/sql"
	"fmt"
	"testing"

	"github.com/wvandaal/ddis/internal/progress"
	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/witness"
)

// setupWitnessDB creates an in-memory SQLite DB populated with a spec, source file,
// section, 5 invariants (APP-INV-T01..T05), 5 registry entries, a module, and
// module_relationships (maintains) for each invariant. Returns the raw *sql.DB
// and the spec ID.
func setupWitnessDB(t *testing.T) (*sql.DB, int64) {
	t.Helper()

	db, err := storage.Open(":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	t.Cleanup(func() { db.Close() })

	// 1. Insert spec_index
	specID, err := storage.InsertSpecIndex(db, &storage.SpecIndex{
		SpecPath:    "/test/witness-spec.md",
		SpecName:    "witness-test-spec",
		DDISVersion: "3.0",
		TotalLines:  500,
		ContentHash: "spechash000",
		ParsedAt:    "2026-01-01T00:00:00Z",
		SourceType:  "modular",
	})
	if err != nil {
		t.Fatalf("insert spec: %v", err)
	}

	// 2. Insert source_file (role: module)
	sfID, err := storage.InsertSourceFile(db, &storage.SourceFile{
		SpecID:      specID,
		FilePath:    "/test/modules/witness-module.md",
		FileRole:    "module",
		ModuleName:  "witness-mod",
		ContentHash: "sfhash000",
		LineCount:   500,
		RawText:     "# Witness Module\nTest content for witness tests.",
	})
	if err != nil {
		t.Fatalf("insert source file: %v", err)
	}

	// 3. Insert section
	secID, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "§W.1",
		Title:        "Witness Test Section",
		HeadingLevel: 2,
		LineStart:    1,
		LineEnd:      500,
		RawText:      "## Witness Test Section\nContent.",
		ContentHash:  "sechash000",
	})
	if err != nil {
		t.Fatalf("insert section: %v", err)
	}

	// 4. Insert 5 invariants with distinct content hashes
	invIDs := []string{"APP-INV-T01", "APP-INV-T02", "APP-INV-T03", "APP-INV-T04", "APP-INV-T05"}
	for i, invID := range invIDs {
		_, err := storage.InsertInvariant(db, &storage.Invariant{
			SpecID:            specID,
			SourceFileID:      sfID,
			SectionID:         secID,
			InvariantID:       invID,
			Title:             fmt.Sprintf("Test Invariant %d", i+1),
			Statement:         fmt.Sprintf("Statement for invariant %d.", i+1),
			SemiFormal:        fmt.Sprintf("forall x: prop_%d(x)", i+1),
			ViolationScenario: fmt.Sprintf("Violation scenario %d.", i+1),
			ValidationMethod:  fmt.Sprintf("Validation method %d.", i+1),
			LineStart:         i*20 + 1,
			LineEnd:           i*20 + 20,
			RawText:           fmt.Sprintf("Raw text for %s", invID),
			ContentHash:       fmt.Sprintf("invhash%03d", i+1),
		})
		if err != nil {
			t.Fatalf("insert %s: %v", invID, err)
		}
	}

	// 5. Insert 5 invariant_registry entries
	for i, invID := range invIDs {
		_, err := storage.InsertInvariantRegistryEntry(db, &storage.InvariantRegistryEntry{
			SpecID:      specID,
			InvariantID: invID,
			Owner:       "witness-mod",
			Domain:      "testing",
			Description: fmt.Sprintf("Registry entry for %s (test %d)", invID, i+1),
		})
		if err != nil {
			t.Fatalf("insert registry %s: %v", invID, err)
		}
	}

	// 6. Insert module
	modID, err := storage.InsertModule(db, &storage.Module{
		SpecID:       specID,
		SourceFileID: sfID,
		ModuleName:   "witness-mod",
		Domain:       "testing",
		LineCount:    500,
	})
	if err != nil {
		t.Fatalf("insert module: %v", err)
	}

	// 7. Insert module_relationships (maintains for each invariant)
	for _, invID := range invIDs {
		_, err := storage.InsertModuleRelationship(db, &storage.ModuleRelationship{
			ModuleID: modID,
			RelType:  "maintains",
			Target:   invID,
		})
		if err != nil {
			t.Fatalf("insert module_relationship for %s: %v", invID, err)
		}
	}

	return db, specID
}

// ---------------------------------------------------------------------------
// TestRecord_NewWitness
// ---------------------------------------------------------------------------

func TestRecord_NewWitness(t *testing.T) {
	db, specID := setupWitnessDB(t)

	w, err := witness.Record(db, specID, witness.Options{
		InvariantID:  "APP-INV-T01",
		EvidenceType: "test",
		Evidence:     "unit test passed",
		ProvenBy:     "test-agent",
		Model:        "claude-opus-4-6",
		CodeHash:     "code123",
		Notes:        "initial witness",
	})
	if err != nil {
		t.Fatalf("Record: %v", err)
	}

	if w.Status != "valid" {
		t.Errorf("expected status 'valid', got %q", w.Status)
	}
	if w.InvariantID != "APP-INV-T01" {
		t.Errorf("expected invariant_id 'APP-INV-T01', got %q", w.InvariantID)
	}
	// SpecHash should match the invariant's ContentHash
	if w.SpecHash != "invhash001" {
		t.Errorf("expected spec_hash 'invhash001' (invariant content_hash), got %q", w.SpecHash)
	}
	if w.EvidenceType != "test" {
		t.Errorf("expected evidence_type 'test', got %q", w.EvidenceType)
	}
	if w.ProvenBy != "test-agent" {
		t.Errorf("expected proven_by 'test-agent', got %q", w.ProvenBy)
	}
	if w.Model != "claude-opus-4-6" {
		t.Errorf("expected model 'claude-opus-4-6', got %q", w.Model)
	}
	if w.ProvenAt == "" {
		t.Error("expected proven_at to be set (non-empty)")
	}
}

// ---------------------------------------------------------------------------
// TestRecord_InvalidInvariant
// ---------------------------------------------------------------------------

func TestRecord_InvalidInvariant(t *testing.T) {
	db, specID := setupWitnessDB(t)

	_, err := witness.Record(db, specID, witness.Options{
		InvariantID:  "APP-INV-NONEXISTENT",
		EvidenceType: "attestation",
		Evidence:     "should fail",
		ProvenBy:     "test-agent",
	})
	if err == nil {
		t.Fatal("expected error for nonexistent invariant, got nil")
	}
}

// ---------------------------------------------------------------------------
// TestRecord_ReplaceExisting
// ---------------------------------------------------------------------------

func TestRecord_ReplaceExisting(t *testing.T) {
	db, specID := setupWitnessDB(t)

	// First witness
	w1, err := witness.Record(db, specID, witness.Options{
		InvariantID:  "APP-INV-T01",
		EvidenceType: "attestation",
		Evidence:     "first witness",
		ProvenBy:     "agent-1",
	})
	if err != nil {
		t.Fatalf("first Record: %v", err)
	}

	// Second witness (replaces due to UNIQUE constraint on spec_id, invariant_id)
	w2, err := witness.Record(db, specID, witness.Options{
		InvariantID:  "APP-INV-T01",
		EvidenceType: "test",
		Evidence:     "second witness (replacement)",
		ProvenBy:     "agent-2",
	})
	if err != nil {
		t.Fatalf("second Record: %v", err)
	}

	// The replacement should have updated fields
	if w2.ProvenBy != "agent-2" {
		t.Errorf("expected proven_by 'agent-2' after replacement, got %q", w2.ProvenBy)
	}
	if w2.EvidenceType != "test" {
		t.Errorf("expected evidence_type 'test' after replacement, got %q", w2.EvidenceType)
	}

	// Verify only one witness exists for this invariant
	witnesses, err := storage.ListWitnesses(db, specID)
	if err != nil {
		t.Fatalf("ListWitnesses: %v", err)
	}
	count := 0
	for _, w := range witnesses {
		if w.InvariantID == "APP-INV-T01" {
			count++
		}
	}
	if count != 1 {
		t.Errorf("expected exactly 1 witness for APP-INV-T01 after replacement, got %d", count)
	}

	// Ensure timestamps differ (second is at least as recent)
	_ = w1 // w1 used only for initial creation
}

// ---------------------------------------------------------------------------
// TestCheck_AllValid
// ---------------------------------------------------------------------------

func TestCheck_AllValid(t *testing.T) {
	db, specID := setupWitnessDB(t)

	// Witness all 5 invariants
	invIDs := []string{"APP-INV-T01", "APP-INV-T02", "APP-INV-T03", "APP-INV-T04", "APP-INV-T05"}
	for _, invID := range invIDs {
		_, err := witness.Record(db, specID, witness.Options{
			InvariantID:  invID,
			EvidenceType: "test",
			Evidence:     "all tests pass",
			ProvenBy:     "test-agent",
		})
		if err != nil {
			t.Fatalf("Record %s: %v", invID, err)
		}
	}

	summary, err := witness.Check(db, specID, witness.CheckOptions{})
	if err != nil {
		t.Fatalf("Check: %v", err)
	}

	if summary.Total != 5 {
		t.Errorf("expected total 5, got %d", summary.Total)
	}
	if summary.Valid != 5 {
		t.Errorf("expected valid 5, got %d", summary.Valid)
	}
	if summary.Stale != 0 {
		t.Errorf("expected stale 0, got %d", summary.Stale)
	}
	if summary.Missing != 0 {
		t.Errorf("expected missing 0, got %d", summary.Missing)
	}
	if summary.Coverage != "100%" {
		t.Errorf("expected coverage '100%%', got %q", summary.Coverage)
	}
}

// ---------------------------------------------------------------------------
// TestCheck_StalenessDetection
// ---------------------------------------------------------------------------

func TestCheck_StalenessDetection(t *testing.T) {
	db, specID := setupWitnessDB(t)

	// Witness APP-INV-T01
	_, err := witness.Record(db, specID, witness.Options{
		InvariantID:  "APP-INV-T01",
		EvidenceType: "test",
		Evidence:     "passed",
		ProvenBy:     "test-agent",
	})
	if err != nil {
		t.Fatalf("Record: %v", err)
	}

	// Now change the invariant's content_hash to simulate a spec edit
	_, err = db.Exec(
		`UPDATE invariants SET content_hash = 'CHANGED_HASH' WHERE spec_id = ? AND invariant_id = ?`,
		specID, "APP-INV-T01",
	)
	if err != nil {
		t.Fatalf("update content_hash: %v", err)
	}

	// Refresh should detect the stale witness
	invalidated, err := witness.Refresh(db, specID)
	if err != nil {
		t.Fatalf("Refresh: %v", err)
	}
	if invalidated != 1 {
		t.Errorf("expected 1 invalidated witness, got %d", invalidated)
	}

	// Verify the witness is now stale_spec
	w, err := storage.GetWitness(db, specID, "APP-INV-T01")
	if err != nil {
		t.Fatalf("GetWitness: %v", err)
	}
	if w.Status != "stale_spec" {
		t.Errorf("expected status 'stale_spec', got %q", w.Status)
	}
}

// ---------------------------------------------------------------------------
// TestCheck_MissingInvariants
// ---------------------------------------------------------------------------

func TestCheck_MissingInvariants(t *testing.T) {
	db, specID := setupWitnessDB(t)

	// Witness only 2 of the 5 invariants
	for _, invID := range []string{"APP-INV-T01", "APP-INV-T03"} {
		_, err := witness.Record(db, specID, witness.Options{
			InvariantID:  invID,
			EvidenceType: "attestation",
			Evidence:     "confirmed",
			ProvenBy:     "test-agent",
		})
		if err != nil {
			t.Fatalf("Record %s: %v", invID, err)
		}
	}

	summary, err := witness.Check(db, specID, witness.CheckOptions{})
	if err != nil {
		t.Fatalf("Check: %v", err)
	}

	if summary.Total != 5 {
		t.Errorf("expected total 5, got %d", summary.Total)
	}
	if summary.Valid != 2 {
		t.Errorf("expected valid 2, got %d", summary.Valid)
	}
	if summary.Missing != 3 {
		t.Errorf("expected missing 3, got %d", summary.Missing)
	}
}

// ---------------------------------------------------------------------------
// TestValidDoneSet
// ---------------------------------------------------------------------------

func TestValidDoneSet(t *testing.T) {
	db, specID := setupWitnessDB(t)

	// Create 3 valid witnesses
	for _, invID := range []string{"APP-INV-T01", "APP-INV-T02", "APP-INV-T03"} {
		_, err := witness.Record(db, specID, witness.Options{
			InvariantID:  invID,
			EvidenceType: "test",
			Evidence:     "passed",
			ProvenBy:     "test-agent",
		})
		if err != nil {
			t.Fatalf("Record %s: %v", invID, err)
		}
	}

	// Create 1 witness for T04, then make it stale
	_, err := witness.Record(db, specID, witness.Options{
		InvariantID:  "APP-INV-T04",
		EvidenceType: "test",
		Evidence:     "passed",
		ProvenBy:     "test-agent",
	})
	if err != nil {
		t.Fatalf("Record APP-INV-T04: %v", err)
	}

	// Change T04's content_hash to make the witness stale
	_, err = db.Exec(
		`UPDATE invariants SET content_hash = 'CHANGED' WHERE spec_id = ? AND invariant_id = ?`,
		specID, "APP-INV-T04",
	)
	if err != nil {
		t.Fatalf("update content_hash: %v", err)
	}

	// Refresh to invalidate the stale witness
	_, err = witness.Refresh(db, specID)
	if err != nil {
		t.Fatalf("Refresh: %v", err)
	}

	// ValidDoneSet should return only the 3 valid ones
	doneSet, err := witness.ValidDoneSet(db, specID)
	if err != nil {
		t.Fatalf("ValidDoneSet: %v", err)
	}

	if len(doneSet) != 3 {
		t.Errorf("expected 3 valid witnesses in done set, got %d", len(doneSet))
	}
	for _, invID := range []string{"APP-INV-T01", "APP-INV-T02", "APP-INV-T03"} {
		if !doneSet[invID] {
			t.Errorf("expected %s in done set", invID)
		}
	}
	if doneSet["APP-INV-T04"] {
		t.Error("APP-INV-T04 should NOT be in done set (stale)")
	}
}

// ---------------------------------------------------------------------------
// TestInvalidateWitnesses
// ---------------------------------------------------------------------------

func TestInvalidateWitnesses(t *testing.T) {
	db, specID := setupWitnessDB(t)

	// Create a witness for T02
	_, err := witness.Record(db, specID, witness.Options{
		InvariantID:  "APP-INV-T02",
		EvidenceType: "test",
		Evidence:     "passed",
		ProvenBy:     "test-agent",
	})
	if err != nil {
		t.Fatalf("Record: %v", err)
	}

	// Verify it is valid
	w, err := storage.GetWitness(db, specID, "APP-INV-T02")
	if err != nil {
		t.Fatalf("GetWitness before invalidation: %v", err)
	}
	if w.Status != "valid" {
		t.Fatalf("expected status 'valid' before invalidation, got %q", w.Status)
	}

	// Change the invariant's content_hash so spec_hash no longer matches
	_, err = db.Exec(
		`UPDATE invariants SET content_hash = 'NEW_HASH_T02' WHERE spec_id = ? AND invariant_id = ?`,
		specID, "APP-INV-T02",
	)
	if err != nil {
		t.Fatalf("update content_hash: %v", err)
	}

	// Call InvalidateWitnesses directly through storage
	count, err := storage.InvalidateWitnesses(db, specID, specID)
	if err != nil {
		t.Fatalf("InvalidateWitnesses: %v", err)
	}
	if count != 1 {
		t.Errorf("expected 1 witness invalidated, got %d", count)
	}

	// Verify the witness status changed to stale_spec
	w, err = storage.GetWitness(db, specID, "APP-INV-T02")
	if err != nil {
		t.Fatalf("GetWitness after invalidation: %v", err)
	}
	if w.Status != "stale_spec" {
		t.Errorf("expected status 'stale_spec' after invalidation, got %q", w.Status)
	}
}

// ---------------------------------------------------------------------------
// TestProgressWithWitnesses
// ---------------------------------------------------------------------------

func TestProgressWithWitnesses(t *testing.T) {
	db, specID := setupWitnessDB(t)

	// Witness 2 of the 5 invariants
	for _, invID := range []string{"APP-INV-T01", "APP-INV-T03"} {
		_, err := witness.Record(db, specID, witness.Options{
			InvariantID:  invID,
			EvidenceType: "test",
			Evidence:     "passed",
			ProvenBy:     "test-agent",
		})
		if err != nil {
			t.Fatalf("Record %s: %v", invID, err)
		}
	}

	// Analyze progress with UseWitnesses enabled
	result, err := progress.Analyze(db, specID, progress.Options{
		UseWitnesses: true,
	})
	if err != nil {
		t.Fatalf("progress.Analyze: %v", err)
	}

	// Verify 2 items are in the Done set
	if len(result.Done) != 2 {
		t.Errorf("expected 2 done items, got %d", len(result.Done))
	}

	// Build a map of done IDs for easier checking
	doneIDs := make(map[string]bool)
	for _, d := range result.Done {
		doneIDs[d.ID] = true
	}

	if !doneIDs["APP-INV-T01"] {
		t.Error("expected APP-INV-T01 in done set")
	}
	if !doneIDs["APP-INV-T03"] {
		t.Error("expected APP-INV-T03 in done set")
	}

	// The remaining 3 should be in frontier (no dependencies between invariants)
	if len(result.Frontier) != 3 {
		t.Errorf("expected 3 frontier items, got %d", len(result.Frontier))
	}
}

// ---------------------------------------------------------------------------
// TestProgressWitnessAndDoneFlag
// ---------------------------------------------------------------------------

func TestProgressWitnessAndDoneFlag(t *testing.T) {
	db, specID := setupWitnessDB(t)

	// Witness APP-INV-T01 via the witness system
	_, err := witness.Record(db, specID, witness.Options{
		InvariantID:  "APP-INV-T01",
		EvidenceType: "test",
		Evidence:     "passed",
		ProvenBy:     "test-agent",
	})
	if err != nil {
		t.Fatalf("Record: %v", err)
	}

	// Also mark APP-INV-T02 as done via --done flag
	result, err := progress.Analyze(db, specID, progress.Options{
		Done:         "APP-INV-T02",
		UseWitnesses: true,
	})
	if err != nil {
		t.Fatalf("progress.Analyze: %v", err)
	}

	// Both should be in Done (additive: witness + --done flag)
	if len(result.Done) != 2 {
		t.Errorf("expected 2 done items (witness + done flag), got %d", len(result.Done))
	}

	doneIDs := make(map[string]bool)
	for _, d := range result.Done {
		doneIDs[d.ID] = true
	}

	if !doneIDs["APP-INV-T01"] {
		t.Error("expected APP-INV-T01 in done set (from witness)")
	}
	if !doneIDs["APP-INV-T02"] {
		t.Error("expected APP-INV-T02 in done set (from --done flag)")
	}

	// The remaining 3 should be in frontier
	if len(result.Frontier) != 3 {
		t.Errorf("expected 3 frontier items, got %d", len(result.Frontier))
	}
}
