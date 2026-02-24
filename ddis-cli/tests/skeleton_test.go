package tests

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/skeleton"
	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/validator"
)

func TestSkeletonGenerateParseValidate(t *testing.T) {
	// Generate skeleton with two domains
	outDir := filepath.Join(t.TempDir(), "test-spec")
	opts := skeleton.Options{
		Name:    "Test Spec",
		Domains: []string{"core", "auth"},
		Output:  outDir,
	}
	result, err := skeleton.Generate(opts)
	if err != nil {
		t.Fatalf("Generate: %v", err)
	}
	if len(result.Files) == 0 {
		t.Fatal("expected generated files, got none")
	}
	if result.OutputDir != outDir {
		t.Fatalf("OutputDir = %q, want %q", result.OutputDir, outDir)
	}

	// Verify expected files exist
	for _, path := range []string{
		filepath.Join(outDir, "manifest.yaml"),
		filepath.Join(outDir, "constitution", "system.md"),
		filepath.Join(outDir, "modules", "core.md"),
		filepath.Join(outDir, "modules", "auth.md"),
	} {
		if _, err := os.Stat(path); err != nil {
			t.Fatalf("expected file %s to exist: %v", path, err)
		}
	}

	// Parse skeleton via ParseModularSpec
	manifestPath := filepath.Join(outDir, "manifest.yaml")
	dbPath := filepath.Join(t.TempDir(), "skel-test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()
	specID, err := parser.ParseModularSpec(manifestPath, db)
	if err != nil {
		t.Fatalf("ParseModularSpec: %v", err)
	}
	if specID <= 0 {
		t.Fatalf("specID = %d, want > 0", specID)
	}

	// Validate — some checks should pass
	report, err := validator.Validate(db, specID, validator.ValidateOptions{})
	if err != nil {
		t.Fatalf("Validate: %v", err)
	}
	if report == nil {
		t.Fatal("expected non-nil report")
	}
	if report.TotalChecks == 0 {
		t.Fatal("expected at least one check to run")
	}

	passedCount := 0
	for _, r := range report.Results {
		if r.Passed {
			passedCount++
		}
	}
	if passedCount == 0 {
		t.Fatal("expected at least one validation check to pass")
	}
}

func TestSkeletonSingleDomain(t *testing.T) {
	outDir := filepath.Join(t.TempDir(), "single-domain")
	opts := skeleton.Options{
		Name:    "Simple Spec",
		Domains: []string{"core"},
		Output:  outDir,
	}
	result, err := skeleton.Generate(opts)
	if err != nil {
		t.Fatalf("Generate: %v", err)
	}
	// manifest + constitution + 1 module = 3 files
	if len(result.Files) != 3 {
		t.Fatalf("expected 3 files, got %d", len(result.Files))
	}

	// Verify manifest has two-tier mode
	manifestData, err := os.ReadFile(filepath.Join(outDir, "manifest.yaml"))
	if err != nil {
		t.Fatalf("read manifest: %v", err)
	}
	if !strings.Contains(string(manifestData), `tier_mode: "two-tier"`) {
		t.Fatal("expected two-tier mode in manifest for single domain")
	}

	// Parse should succeed
	dbPath := filepath.Join(t.TempDir(), "single.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()
	specID, err := parser.ParseModularSpec(filepath.Join(outDir, "manifest.yaml"), db)
	if err != nil {
		t.Fatalf("ParseModularSpec: %v", err)
	}
	if specID <= 0 {
		t.Fatalf("specID = %d, want > 0", specID)
	}
}

func TestSkeletonMultipleDomains(t *testing.T) {
	outDir := filepath.Join(t.TempDir(), "multi-domain")
	opts := skeleton.Options{
		Name:    "Multi Domain Spec",
		Domains: []string{"api", "storage", "auth"},
		Output:  outDir,
	}
	result, err := skeleton.Generate(opts)
	if err != nil {
		t.Fatalf("Generate: %v", err)
	}
	// manifest + constitution + 3 modules = 5 files
	if len(result.Files) != 5 {
		t.Fatalf("expected 5 files, got %d", len(result.Files))
	}

	// Verify manifest has three-tier mode
	manifestData, err := os.ReadFile(filepath.Join(outDir, "manifest.yaml"))
	if err != nil {
		t.Fatalf("read manifest: %v", err)
	}
	if !strings.Contains(string(manifestData), `tier_mode: "three-tier"`) {
		t.Fatal("expected three-tier mode in manifest for multiple domains")
	}

	// Each module should have correct frontmatter
	for _, domain := range opts.Domains {
		modData, err := os.ReadFile(filepath.Join(outDir, "modules", domain+".md"))
		if err != nil {
			t.Fatalf("read module %s: %v", domain, err)
		}
		if !strings.Contains(string(modData), "module: "+domain) {
			t.Fatalf("module %s missing 'module: %s' in frontmatter", domain, domain)
		}
		if !strings.Contains(string(modData), "domain: "+domain) {
			t.Fatalf("module %s missing 'domain: %s' in frontmatter", domain, domain)
		}
	}
}

func TestSkeletonOutputDirCreated(t *testing.T) {
	outDir := filepath.Join(t.TempDir(), "nested", "deep", "spec")
	opts := skeleton.Options{
		Name:    "Deep Spec",
		Domains: []string{"core"},
		Output:  outDir,
	}
	result, err := skeleton.Generate(opts)
	if err != nil {
		t.Fatalf("Generate: %v", err)
	}
	if _, err := os.Stat(result.OutputDir); err != nil {
		t.Fatalf("output dir not created: %v", err)
	}
}

func TestSkeletonTotalLines(t *testing.T) {
	outDir := filepath.Join(t.TempDir(), "lines-test")
	opts := skeleton.Options{
		Name:    "Lines Spec",
		Domains: []string{"core"},
		Output:  outDir,
	}
	result, err := skeleton.Generate(opts)
	if err != nil {
		t.Fatalf("Generate: %v", err)
	}
	if result.TotalLines <= 0 {
		t.Fatal("expected TotalLines > 0")
	}

	sum := 0
	for _, f := range result.Files {
		sum += f.Lines
	}
	if sum != result.TotalLines {
		t.Fatalf("TotalLines = %d, but sum of file lines = %d", result.TotalLines, sum)
	}
}
