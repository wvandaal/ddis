//go:build integration

package tests

import (
	"crypto/sha256"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/renderer"
	"github.com/wvandaal/ddis/internal/storage"
)

func TestRoundTripMonolith(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	original, err := os.ReadFile(specPath)
	if err != nil {
		t.Fatalf("read spec: %v", err)
	}

	// Parse into temp DB
	dbPath := filepath.Join(t.TempDir(), "test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	// Render back
	outputPath := filepath.Join(t.TempDir(), "output.md")
	if err := renderer.RenderMonolith(db, specID, outputPath); err != nil {
		t.Fatalf("render: %v", err)
	}

	// Compare byte-for-byte
	rendered, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatalf("read output: %v", err)
	}

	if string(original) != string(rendered) {
		t.Errorf("round-trip fidelity failed: output differs from input (%d vs %d bytes)",
			len(rendered), len(original))
	}
}

func TestRoundTripModular(t *testing.T) {
	manifestPath := filepath.Join(projectRoot(), "ddis-modular", "manifest.yaml")
	if _, err := os.Stat(manifestPath); os.IsNotExist(err) {
		t.Skipf("manifest.yaml not found at %s", manifestPath)
	}

	// Parse
	dbPath := filepath.Join(t.TempDir(), "modular.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseModularSpec(manifestPath, db)
	if err != nil {
		t.Fatalf("parse modular: %v", err)
	}

	// Render to temp dir
	outputDir := filepath.Join(t.TempDir(), "modular_out")
	if err := renderer.RenderModular(db, specID, outputDir); err != nil {
		t.Fatalf("render modular: %v", err)
	}

	// Compare each file
	filesToCompare := []string{
		"constitution/system.md",
		"modules/core-standard.md",
		"modules/element-specifications.md",
		"modules/modularization.md",
		"modules/guidance-operations.md",
		"manifest.yaml",
	}

	for _, f := range filesToCompare {
		origPath := filepath.Join(projectRoot(), "ddis-modular", f)
		renderedPath := filepath.Join(outputDir, f)

		orig, err := os.ReadFile(origPath)
		if err != nil {
			t.Errorf("read original %s: %v", f, err)
			continue
		}

		rendered, err := os.ReadFile(renderedPath)
		if err != nil {
			t.Errorf("read rendered %s: %v", f, err)
			continue
		}

		if string(orig) != string(rendered) {
			t.Errorf("%s: round-trip failed (%d vs %d bytes)", f, len(rendered), len(orig))
		}
	}
}

// =============================================================================
// APP-INV-001: Round-Trip Fidelity (Property Tests)
// =============================================================================
//
// Verifies the formal predicate:
//   ∀ spec ∈ ValidSpecs: render(parse(spec)) = spec (byte-level identity)

// TestRoundTripCLISpec verifies round-trip fidelity on the CLI's own specification.
// This is the self-bootstrapping property test: the tool round-trips its own spec.
func TestRoundTripCLISpec(t *testing.T) {
	manifestPath := filepath.Join(projectRoot(), "ddis-cli-spec", "manifest.yaml")
	if _, err := os.Stat(manifestPath); os.IsNotExist(err) {
		t.Skipf("CLI spec manifest not found at %s", manifestPath)
	}

	// Parse
	dbPath := filepath.Join(t.TempDir(), "cli-spec.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseModularSpec(manifestPath, db)
	if err != nil {
		t.Fatalf("parse CLI spec: %v", err)
	}

	// Render to temp dir
	outputDir := filepath.Join(t.TempDir(), "cli_spec_out")
	if err := renderer.RenderModular(db, specID, outputDir); err != nil {
		t.Fatalf("render CLI spec: %v", err)
	}

	// Compare each file byte-for-byte with SHA-256 verification
	filesToCompare := []string{
		"constitution/system.md",
		"modules/parse-pipeline.md",
		"modules/search-intelligence.md",
		"modules/query-validation.md",
		"modules/lifecycle-ops.md",
		"manifest.yaml",
	}

	for _, f := range filesToCompare {
		origPath := filepath.Join(projectRoot(), "ddis-cli-spec", f)
		renderedPath := filepath.Join(outputDir, f)

		orig, err := os.ReadFile(origPath)
		if err != nil {
			t.Errorf("read original %s: %v", f, err)
			continue
		}

		rendered, err := os.ReadFile(renderedPath)
		if err != nil {
			t.Errorf("read rendered %s: %v", f, err)
			continue
		}

		origHash := sha256.Sum256(orig)
		renderedHash := sha256.Sum256(rendered)

		if origHash != renderedHash {
			t.Errorf("%s: SHA-256 mismatch (%d vs %d bytes)", f, len(orig), len(rendered))
			// Find first differing line for diagnostics
			origLines := strings.Split(string(orig), "\n")
			renderedLines := strings.Split(string(rendered), "\n")
			for i := 0; i < len(origLines) && i < len(renderedLines); i++ {
				if origLines[i] != renderedLines[i] {
					t.Errorf("  first diff at line %d:\n    orig:     %q\n    rendered: %q",
						i+1, origLines[i], renderedLines[i])
					break
				}
			}
			if len(origLines) != len(renderedLines) {
				t.Errorf("  line count: orig=%d, rendered=%d", len(origLines), len(renderedLines))
			}
		} else {
			t.Logf("%s: SHA-256 match (%x, %d bytes)", f, origHash[:8], len(orig))
		}
	}
}

// TestRoundTripIdempotency verifies multi-stage round-trip stability:
//   render(parse(render(parse(spec)))) = render(parse(spec))
func TestRoundTripIdempotency(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	// Stage 1: parse → render
	db1Path := filepath.Join(t.TempDir(), "stage1.db")
	db1, err := storage.Open(db1Path)
	if err != nil {
		t.Fatalf("open db1: %v", err)
	}
	defer db1.Close()

	specID1, err := parser.ParseDocument(specPath, db1)
	if err != nil {
		t.Fatalf("parse stage 1: %v", err)
	}

	stage1Path := filepath.Join(t.TempDir(), "stage1.md")
	if err := renderer.RenderMonolith(db1, specID1, stage1Path); err != nil {
		t.Fatalf("render stage 1: %v", err)
	}

	// Stage 2: parse(stage1 output) → render
	db2Path := filepath.Join(t.TempDir(), "stage2.db")
	db2, err := storage.Open(db2Path)
	if err != nil {
		t.Fatalf("open db2: %v", err)
	}
	defer db2.Close()

	specID2, err := parser.ParseDocument(stage1Path, db2)
	if err != nil {
		t.Fatalf("parse stage 2: %v", err)
	}

	stage2Path := filepath.Join(t.TempDir(), "stage2.md")
	if err := renderer.RenderMonolith(db2, specID2, stage2Path); err != nil {
		t.Fatalf("render stage 2: %v", err)
	}

	// Compare stage 1 and stage 2 outputs
	stage1, err := os.ReadFile(stage1Path)
	if err != nil {
		t.Fatalf("read stage 1: %v", err)
	}
	stage2, err := os.ReadFile(stage2Path)
	if err != nil {
		t.Fatalf("read stage 2: %v", err)
	}

	hash1 := sha256.Sum256(stage1)
	hash2 := sha256.Sum256(stage2)

	if hash1 != hash2 {
		t.Errorf("idempotency failed: stage1 (%d bytes, %x) != stage2 (%d bytes, %x)",
			len(stage1), hash1[:8], len(stage2), hash2[:8])
	} else {
		t.Logf("APP-INV-001 idempotency: stage1 == stage2 (%x, %d bytes)", hash1[:8], len(stage1))
	}

	// Also verify stage 1 equals original (the basic round-trip)
	original, err := os.ReadFile(specPath)
	if err != nil {
		t.Fatalf("read original: %v", err)
	}
	origHash := sha256.Sum256(original)
	if origHash != hash1 {
		t.Errorf("stage1 differs from original (%d vs %d bytes)", len(stage1), len(original))
	}
}

// TestRoundTripLinePreservation verifies that every line in the original
// appears in the rendered output at the same position.
func TestRoundTripLinePreservation(t *testing.T) {
	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	original, err := os.ReadFile(specPath)
	if err != nil {
		t.Fatalf("read spec: %v", err)
	}

	dbPath := filepath.Join(t.TempDir(), "linetest.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	outputPath := filepath.Join(t.TempDir(), "linetest.md")
	if err := renderer.RenderMonolith(db, specID, outputPath); err != nil {
		t.Fatalf("render: %v", err)
	}

	rendered, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatalf("read output: %v", err)
	}

	origLines := strings.Split(string(original), "\n")
	renderedLines := strings.Split(string(rendered), "\n")

	// Line count must match
	if len(origLines) != len(renderedLines) {
		t.Fatalf("line count mismatch: original=%d, rendered=%d", len(origLines), len(renderedLines))
	}

	// Every line must match at the same position
	diffs := 0
	for i := 0; i < len(origLines); i++ {
		if origLines[i] != renderedLines[i] {
			diffs++
			if diffs <= 5 {
				t.Errorf("line %d differs:\n  orig:     %q\n  rendered: %q", i+1, origLines[i], renderedLines[i])
			}
		}
	}

	if diffs > 5 {
		t.Errorf("... and %d more differing lines", diffs-5)
	}

	// Verify blank line count preserved
	origBlanks := 0
	renderedBlanks := 0
	for _, l := range origLines {
		if strings.TrimSpace(l) == "" {
			origBlanks++
		}
	}
	for _, l := range renderedLines {
		if strings.TrimSpace(l) == "" {
			renderedBlanks++
		}
	}
	if origBlanks != renderedBlanks {
		t.Errorf("blank line count: original=%d, rendered=%d", origBlanks, renderedBlanks)
	}

	t.Logf("APP-INV-001 line preservation: %d lines, %d blank lines, %d diffs",
		len(origLines), origBlanks, diffs)
}

// hashFile computes SHA-256 of a file and returns the hex string.
func hashFile(path string) (string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	h := sha256.Sum256(data)
	return fmt.Sprintf("%x", h), nil
}
