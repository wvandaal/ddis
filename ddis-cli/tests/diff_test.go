//go:build integration

package tests

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/diff"
	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/storage"
)

func TestDiffIdentical(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	// Parse the same spec into two databases
	basePath := filepath.Join(t.TempDir(), "base.db")
	headPath := filepath.Join(t.TempDir(), "head.db")

	baseDB, err := storage.Open(basePath)
	if err != nil {
		t.Fatalf("open base: %v", err)
	}
	defer baseDB.Close()

	headDB, err := storage.Open(headPath)
	if err != nil {
		t.Fatalf("open head: %v", err)
	}
	defer headDB.Close()

	baseSpecID, err := parser.ParseDocument(specPath, baseDB)
	if err != nil {
		t.Fatalf("parse base: %v", err)
	}
	headSpecID, err := parser.ParseDocument(specPath, headDB)
	if err != nil {
		t.Fatalf("parse head: %v", err)
	}

	result, err := diff.ComputeDiff(baseDB, headDB, baseSpecID, headSpecID)
	if err != nil {
		t.Fatalf("diff: %v", err)
	}

	// Same spec → 0 changes
	if result.Summary.Added != 0 {
		t.Errorf("added = %d, want 0", result.Summary.Added)
	}
	if result.Summary.Removed != 0 {
		t.Errorf("removed = %d, want 0", result.Summary.Removed)
	}
	if result.Summary.Modified != 0 {
		t.Errorf("modified = %d, want 0", result.Summary.Modified)
	}
	if result.Summary.Unchanged == 0 {
		t.Error("unchanged = 0, want > 0")
	}
	if len(result.Changes) != 0 {
		t.Errorf("got %d changes, want 0", len(result.Changes))
	}

	t.Logf("Identical diff: %d unchanged elements", result.Summary.Unchanged)
}

func TestDiffSummary(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	basePath := filepath.Join(t.TempDir(), "base.db")
	headPath := filepath.Join(t.TempDir(), "head.db")

	baseDB, err := storage.Open(basePath)
	if err != nil {
		t.Fatalf("open base: %v", err)
	}
	defer baseDB.Close()

	headDB, err := storage.Open(headPath)
	if err != nil {
		t.Fatalf("open head: %v", err)
	}
	defer headDB.Close()

	baseSpecID, err := parser.ParseDocument(specPath, baseDB)
	if err != nil {
		t.Fatalf("parse base: %v", err)
	}
	headSpecID, err := parser.ParseDocument(specPath, headDB)
	if err != nil {
		t.Fatalf("parse head: %v", err)
	}

	result, err := diff.ComputeDiff(baseDB, headDB, baseSpecID, headSpecID)
	if err != nil {
		t.Fatalf("diff: %v", err)
	}

	// Summary counts should match individual changes
	totalChanges := result.Summary.Added + result.Summary.Removed + result.Summary.Modified
	if totalChanges != len(result.Changes) {
		t.Errorf("summary total changes (%d) != len(changes) (%d)",
			totalChanges, len(result.Changes))
	}

	// Total elements = changed + unchanged
	totalElements := totalChanges + result.Summary.Unchanged
	if totalElements == 0 {
		t.Error("no elements found at all")
	}
	t.Logf("Total elements: %d (+%d -%d ~%d =%d)",
		totalElements, result.Summary.Added, result.Summary.Removed,
		result.Summary.Modified, result.Summary.Unchanged)
}

func TestDiffJSON(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	basePath := filepath.Join(t.TempDir(), "base.db")
	headPath := filepath.Join(t.TempDir(), "head.db")

	baseDB, err := storage.Open(basePath)
	if err != nil {
		t.Fatalf("open base: %v", err)
	}
	defer baseDB.Close()

	headDB, err := storage.Open(headPath)
	if err != nil {
		t.Fatalf("open head: %v", err)
	}
	defer headDB.Close()

	baseSpecID, err := parser.ParseDocument(specPath, baseDB)
	if err != nil {
		t.Fatalf("parse base: %v", err)
	}
	headSpecID, err := parser.ParseDocument(specPath, headDB)
	if err != nil {
		t.Fatalf("parse head: %v", err)
	}

	result, err := diff.ComputeDiff(baseDB, headDB, baseSpecID, headSpecID)
	if err != nil {
		t.Fatalf("diff: %v", err)
	}

	out, err := diff.RenderDiff(result, true)
	if err != nil {
		t.Fatalf("render JSON: %v", err)
	}

	// Verify valid JSON
	var parsed diff.DiffResult
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("parse JSON: %v", err)
	}

	if parsed.Base.SpecPath == "" {
		t.Error("base.spec_path is empty")
	}
	if parsed.Head.SpecPath == "" {
		t.Error("head.spec_path is empty")
	}
	if parsed.Summary.Unchanged == 0 {
		t.Error("JSON summary.unchanged = 0")
	}
}

func TestDiffHumanReadable(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	basePath := filepath.Join(t.TempDir(), "base.db")
	headPath := filepath.Join(t.TempDir(), "head.db")

	baseDB, err := storage.Open(basePath)
	if err != nil {
		t.Fatalf("open base: %v", err)
	}
	defer baseDB.Close()

	headDB, err := storage.Open(headPath)
	if err != nil {
		t.Fatalf("open head: %v", err)
	}
	defer headDB.Close()

	baseSpecID, err := parser.ParseDocument(specPath, baseDB)
	if err != nil {
		t.Fatalf("parse base: %v", err)
	}
	headSpecID, err := parser.ParseDocument(specPath, headDB)
	if err != nil {
		t.Fatalf("parse head: %v", err)
	}

	result, err := diff.ComputeDiff(baseDB, headDB, baseSpecID, headSpecID)
	if err != nil {
		t.Fatalf("diff: %v", err)
	}

	out, err := diff.RenderDiff(result, false)
	if err != nil {
		t.Fatalf("render human: %v", err)
	}

	if out == "" {
		t.Error("human output is empty")
	}
	t.Logf("Human diff output: %d bytes", len(out))
}

func TestDiffMonolithVsModular(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	manifestPath := filepath.Join(projectRoot(), "ddis-modular", "manifest.yaml")

	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}
	if _, err := os.Stat(manifestPath); os.IsNotExist(err) {
		t.Skipf("manifest.yaml not found at %s", manifestPath)
	}

	basePath := filepath.Join(t.TempDir(), "mono.db")
	headPath := filepath.Join(t.TempDir(), "modular.db")

	baseDB, err := storage.Open(basePath)
	if err != nil {
		t.Fatalf("open base: %v", err)
	}
	defer baseDB.Close()

	headDB, err := storage.Open(headPath)
	if err != nil {
		t.Fatalf("open head: %v", err)
	}
	defer headDB.Close()

	baseSpecID, err := parser.ParseDocument(specPath, baseDB)
	if err != nil {
		t.Fatalf("parse monolith: %v", err)
	}
	headSpecID, err := parser.ParseModularSpec(manifestPath, headDB)
	if err != nil {
		t.Fatalf("parse modular: %v", err)
	}

	result, err := diff.ComputeDiff(baseDB, headDB, baseSpecID, headSpecID)
	if err != nil {
		t.Fatalf("diff: %v", err)
	}

	// Cross-format diff should have mostly unchanged elements with some structural differences
	total := result.Summary.Added + result.Summary.Removed + result.Summary.Modified + result.Summary.Unchanged
	t.Logf("Monolith vs Modular: %d total (+%d -%d ~%d =%d)",
		total, result.Summary.Added, result.Summary.Removed,
		result.Summary.Modified, result.Summary.Unchanged)

	// Should have found a significant number of unchanged elements
	if result.Summary.Unchanged < 10 {
		t.Errorf("unchanged = %d, expected >= 10 (shared invariants, ADRs, etc.)", result.Summary.Unchanged)
	}
}
