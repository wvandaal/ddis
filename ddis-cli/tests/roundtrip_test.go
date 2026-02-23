package tests

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/renderer"
	"github.com/wvandaal/ddis/internal/storage"
)

// projectRoot returns the DDIS project root (parent of ddis-cli/).
func projectRoot() string {
	// Try environment variable first
	if root := os.Getenv("DDIS_PROJECT_ROOT"); root != "" {
		return root
	}
	// Default to the known path on the VPS
	return "/data/projects/ddis"
}

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
