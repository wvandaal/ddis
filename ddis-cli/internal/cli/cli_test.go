package cli

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/storage"
)

// executeCommand runs a cobra command with the given args and returns stdout+stderr output.
func executeCommand(args ...string) (string, error) {
	buf := new(bytes.Buffer)

	// Reset all package-level flag variables to defaults before each test.
	// This prevents state leaking between tests.
	NoGuidance = true // suppress guidance to keep output predictable
	parseOutput = ""
	validateJSON = false
	validateChecks = ""
	validateFocus = 0
	validateLevel = 0
	validateLog = false
	validateOplogPath = ""
	validateCodeRoot = ""

	rootCmd.SetOut(buf)
	rootCmd.SetErr(buf)
	rootCmd.SetArgs(args)

	err := rootCmd.Execute()

	return buf.String(), err
}

// setupTestSpec creates a minimal DDIS spec file and returns the directory and spec path.
func setupTestSpec(t *testing.T) (string, string) {
	t.Helper()
	dir := t.TempDir()
	specPath := filepath.Join(dir, "test_spec.md")
	content := `# Test Spec
## §1 Introduction
This is a test specification.

### §1.1 Overview
The system shall maintain data integrity.

**INV-001: Round-Trip Fidelity**
*parse(render(parse(doc))) is byte-identical to parse(doc)*

Violation scenario: A document parsed and rendered produces different bytes when re-parsed.

Validation: Parse a document, render it, and compare SHA-256 hashes.

// WHY THIS MATTERS: Ensures no information loss.

### ADR-001: SQLite as Storage Backend

#### Problem
Need persistent structured storage for spec elements.

#### Options
A) **SQLite** — embedded, zero-config
- Pros: No external dependencies
- Cons: Single-writer

B) **PostgreSQL** — full RDBMS
- Pros: Concurrency
- Cons: External dependency

#### Decision
**Option A.** SQLite is sufficient.

#### Consequences
Embedded database, single binary distribution.

#### Tests
Parse a spec and verify SQLite tables populated.

## §2 Validation
This section covers validation checks.

**Gate 1: Structural Conformance**

**DO NOT** bypass validation checks in production.
`
	if err := os.WriteFile(specPath, []byte(content), 0644); err != nil {
		t.Fatalf("write test spec: %v", err)
	}
	return dir, specPath
}

// setupParsedDB creates a temp DB from a test spec and returns (dir, dbPath, specID).
func setupParsedDB(t *testing.T) (string, string, int64) {
	t.Helper()
	dir, specPath := setupTestSpec(t)
	dbPath := filepath.Join(dir, "test.ddis.db")

	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse document: %v", err)
	}
	if specID == 0 {
		t.Fatal("specID is 0")
	}
	return dir, dbPath, specID
}

func TestVersionCommand(t *testing.T) {
	out, err := executeCommand("version")
	if err != nil {
		t.Fatalf("version command error: %v", err)
	}
	// Version output goes to stdout which cobra doesn't capture via SetOut for Run (not RunE).
	// The command uses fmt.Printf which goes to os.Stdout.
	// We verify no error occurred; the version string format is tested implicitly.
	_ = out
}

func TestParseCommand(t *testing.T) {
	_, specPath := setupTestSpec(t)
	dir := filepath.Dir(specPath)
	dbPath := filepath.Join(dir, "output.ddis.db")

	_, err := executeCommand("parse", specPath, "-o", dbPath)
	if err != nil {
		t.Fatalf("parse command error: %v", err)
	}

	// Verify DB was created and has content
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open created db: %v", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		t.Fatalf("get spec ID: %v", err)
	}
	if specID == 0 {
		t.Fatal("no spec found in parsed DB")
	}

	// Verify invariants were parsed
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		t.Fatalf("list invariants: %v", err)
	}
	if len(invs) == 0 {
		t.Error("no invariants found in parsed DB")
	}
}

func TestParseCommand_MissingArg(t *testing.T) {
	_, err := executeCommand("parse")
	if err == nil {
		t.Fatal("expected error for missing arg, got nil")
	}
}

func TestValidateCommand(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("validate", dbPath)
	// Validation may pass or fail depending on spec content — we just verify no crash
	if err != nil && err != ErrValidationFailed {
		t.Fatalf("validate command unexpected error: %v", err)
	}
}

func TestValidateCommand_JSON(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	// We need to test --json flag. Since flags are reset in executeCommand,
	// we test via the args.
	_, err := executeCommand("validate", dbPath, "--json")
	if err != nil && err != ErrValidationFailed {
		t.Fatalf("validate --json error: %v", err)
	}
}

func TestValidateCommand_Checks(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("validate", dbPath, "--checks", "1,2")
	if err != nil && err != ErrValidationFailed {
		t.Fatalf("validate --checks error: %v", err)
	}
}

func TestValidateCommand_Level(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("validate", dbPath, "--level", "1")
	if err != nil && err != ErrValidationFailed {
		t.Fatalf("validate --level error: %v", err)
	}
}

func TestValidateCommand_Focus(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("validate", dbPath, "--focus", "1")
	if err != nil && err != ErrValidationFailed {
		t.Fatalf("validate --focus error: %v", err)
	}
}

func TestValidateCommand_NonexistentDB(t *testing.T) {
	_, err := executeCommand("validate", "/tmp/nonexistent_test.db")
	if err == nil {
		t.Fatal("expected error for nonexistent DB")
	}
}

func TestCoverageCommand(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("coverage", dbPath)
	if err != nil {
		t.Fatalf("coverage error: %v", err)
	}
}

func TestDriftCommand(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("drift", dbPath)
	if err != nil {
		t.Fatalf("drift error: %v", err)
	}
}

func TestDriftCommand_Report(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("drift", dbPath, "--report")
	if err != nil {
		t.Fatalf("drift --report error: %v", err)
	}
}

func TestQueryCommand(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("query", "INV-001", dbPath)
	if err != nil {
		t.Fatalf("query error: %v", err)
	}
}

func TestQueryCommand_List(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("query", "--list", "invariants", dbPath)
	if err != nil {
		t.Fatalf("query --list error: %v", err)
	}
}

func TestQueryCommand_Stats(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("query", "--stats", dbPath)
	if err != nil {
		t.Fatalf("query --stats error: %v", err)
	}
}

func TestSearchCommand(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("search", "integrity", dbPath)
	if err != nil {
		t.Fatalf("search error: %v", err)
	}
}

func TestSpecCommand(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("spec", dbPath)
	if err != nil {
		t.Fatalf("spec error: %v", err)
	}
}

func TestChecklistCommand(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("checklist", dbPath)
	if err != nil {
		t.Fatalf("checklist error: %v", err)
	}
}

func TestImpactCommand(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("impact", "INV-001", dbPath)
	if err != nil {
		t.Fatalf("impact error: %v", err)
	}
}

func TestImplorderCommand(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("impl-order", dbPath)
	if err != nil {
		t.Fatalf("impl-order error: %v", err)
	}
}

func TestProgressCommand(t *testing.T) {
	_, dbPath, _ := setupParsedDB(t)

	_, err := executeCommand("progress", dbPath)
	if err != nil {
		t.Fatalf("progress error: %v", err)
	}
}

func TestEmitRecoveryHint(t *testing.T) {
	tests := []struct {
		errMsg string
		want   string
	}{
		{"no .ddis.db file found", "ddis parse"},
		{"open database", "ddis parse"},
		{"no spec found", "ddis parse"},
		{"empty query", "ddis search"},
		{"unknown command", "ddis next"},
	}

	for _, tc := range tests {
		// Capture stderr via redirect
		old := os.Stderr
		r, w, _ := os.Pipe()
		os.Stderr = w

		emitRecoveryHint(fmt.Errorf("%s", tc.errMsg))

		w.Close()
		var buf bytes.Buffer
		buf.ReadFrom(r)
		os.Stderr = old

		if !strings.Contains(buf.String(), tc.want) {
			t.Errorf("emitRecoveryHint(%q) output %q, want to contain %q", tc.errMsg, buf.String(), tc.want)
		}
	}
}

func TestNoGuidanceFlag(t *testing.T) {
	// The -q flag should be registered
	f := rootCmd.PersistentFlags().Lookup("no-guidance")
	if f == nil {
		t.Fatal("--no-guidance flag not found on root command")
	}
	if f.Shorthand != "q" {
		t.Errorf("shorthand = %q, want %q", f.Shorthand, "q")
	}
}

func TestCommandGroups(t *testing.T) {
	// Verify all expected groups exist
	groups := rootCmd.Groups()
	expectedGroups := map[string]bool{
		"core":        false,
		"investigate": false,
		"improvement": false,
		"planning":    false,
		"utility":     false,
	}
	for _, g := range groups {
		expectedGroups[g.ID] = true
	}
	for id, found := range expectedGroups {
		if !found {
			t.Errorf("missing command group: %s", id)
		}
	}
}

func TestCommandRegistration(t *testing.T) {
	// Verify key commands are registered
	expectedCmds := []string{
		"parse", "validate", "coverage", "drift", "search", "query",
		"witness", "challenge", "scan", "version", "spec", "next",
		"context", "impact", "checklist", "progress", "impl-order",
	}

	registered := make(map[string]bool)
	for _, cmd := range rootCmd.Commands() {
		registered[cmd.Name()] = true
	}

	for _, name := range expectedCmds {
		if !registered[name] {
			t.Errorf("command %q not registered on root", name)
		}
	}
}
