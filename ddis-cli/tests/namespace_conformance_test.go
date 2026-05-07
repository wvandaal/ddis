package tests

// Conformance harness for the cross-reference + namespace + format-tolerance
// dimensions of the parser. Each fixture directory under tests/fixtures/
// contains a complete modular spec plus a baseline.yaml describing expected
// outcomes. The harness parses each spec, queries the SQLite database, runs
// validator checks, and asserts the baseline contracts.
//
// Schema for baseline.yaml is documented in tests/fixtures/README.md.

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"gopkg.in/yaml.v3"

	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/validator"
)

type baselineFile struct {
	Description string         `yaml:"description"`
	Expect      baselineExpect `yaml:"expect"`
}

type baselineExpect struct {
	ParseErrorSubstring string             `yaml:"parse_error_substring,omitempty"`
	Invariants          *baselineCountIDs  `yaml:"invariants,omitempty"`
	ADRs                *baselineCountIDs  `yaml:"adrs,omitempty"`
	CrossRefs           *baselineXRefs     `yaml:"cross_refs,omitempty"`
	Validation          *baselineValidate  `yaml:"validation,omitempty"`
}

type baselineCountIDs struct {
	Count               *int     `yaml:"count,omitempty"`
	IDs                 []string `yaml:"ids,omitempty"`
	MustPopulateFields  []string `yaml:"must_populate_fields,omitempty"` // e.g. ["violation_scenario", "validation_method"]
}

type baselineXRefs struct {
	Resolved   *int     `yaml:"resolved,omitempty"`
	Unresolved *int     `yaml:"unresolved,omitempty"`
	Targets    []string `yaml:"targets,omitempty"`
}

type baselineValidate struct {
	Errors           *int     `yaml:"errors,omitempty"`
	ErrorMustContain []string `yaml:"error_must_contain,omitempty"`
}

func TestNamespaceConformance(t *testing.T) {
	fixturesDir := "fixtures"
	entries, err := os.ReadDir(fixturesDir)
	if err != nil {
		t.Fatalf("read fixtures dir: %v", err)
	}

	var fixtureDirs []string
	for _, e := range entries {
		if !e.IsDir() {
			continue
		}
		fixtureDirs = append(fixtureDirs, e.Name())
	}
	sort.Strings(fixtureDirs)

	if len(fixtureDirs) == 0 {
		t.Fatalf("no fixture directories found in %s", fixturesDir)
	}

	for _, name := range fixtureDirs {
		name := name // capture for subtest closure
		t.Run(name, func(t *testing.T) {
			runFixture(t, filepath.Join(fixturesDir, name))
		})
	}
}

func runFixture(t *testing.T, fixtureDir string) {
	t.Helper()

	baselinePath := filepath.Join(fixtureDir, "baseline.yaml")
	manifestPath := filepath.Join(fixtureDir, "manifest.yaml")

	baseline := loadBaseline(t, baselinePath)

	dbPath := filepath.Join(t.TempDir(), "fixture.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	specID, err := parser.ParseModularSpec(manifestPath, db)
	if baseline.Expect.ParseErrorSubstring != "" {
		if err == nil {
			t.Fatalf("expected parse to fail with substring %q, but parse succeeded",
				baseline.Expect.ParseErrorSubstring)
		}
		if !strings.Contains(err.Error(), baseline.Expect.ParseErrorSubstring) {
			t.Fatalf("expected parse error to contain %q, got: %v",
				baseline.Expect.ParseErrorSubstring, err)
		}
		return // negative-parse fixtures end here
	}
	if err != nil {
		t.Fatalf("parse failed unexpectedly: %v", err)
	}

	if exp := baseline.Expect.Invariants; exp != nil {
		assertInvariants(t, db, specID, exp)
	}
	if exp := baseline.Expect.ADRs; exp != nil {
		assertADRs(t, db, specID, exp)
	}
	if exp := baseline.Expect.CrossRefs; exp != nil {
		assertCrossRefs(t, db, specID, exp)
	}
	if exp := baseline.Expect.Validation; exp != nil {
		assertValidation(t, db, specID, exp)
	}
}

func loadBaseline(t *testing.T, path string) baselineFile {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read baseline %s: %v", path, err)
	}
	var b baselineFile
	if err := yaml.Unmarshal(data, &b); err != nil {
		t.Fatalf("parse baseline %s: %v", path, err)
	}
	return b
}

func assertInvariants(t *testing.T, db storage.DB, specID int64, exp *baselineCountIDs) {
	t.Helper()
	rows, err := db.Query(
		`SELECT invariant_id FROM invariants WHERE spec_id = ? ORDER BY invariant_id`, specID)
	if err != nil {
		t.Fatalf("query invariants: %v", err)
	}
	defer rows.Close()

	var got []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			t.Fatalf("scan invariant id: %v", err)
		}
		got = append(got, id)
	}

	if exp.Count != nil && len(got) != *exp.Count {
		t.Errorf("invariant count: got %d (%v), want %d", len(got), got, *exp.Count)
	}
	if len(exp.IDs) > 0 {
		assertSetEqual(t, "invariant ids", got, exp.IDs)
	}
	for _, field := range exp.MustPopulateFields {
		// Whitelist column names to prevent SQL injection from baseline files.
		switch field {
		case "violation_scenario", "validation_method", "statement", "semi_formal":
		default:
			t.Errorf("baseline must_populate_fields: unknown column %q", field)
			continue
		}
		var emptyCount int
		query := "SELECT COUNT(*) FROM invariants WHERE spec_id = ? AND (" +
			field + " IS NULL OR " + field + " = '')"
		if err := db.QueryRow(query, specID).Scan(&emptyCount); err != nil {
			t.Fatalf("count empty %s: %v", field, err)
		}
		if emptyCount > 0 {
			t.Errorf("invariants with empty %s column: %d (want 0)", field, emptyCount)
		}
	}
}

func assertADRs(t *testing.T, db storage.DB, specID int64, exp *baselineCountIDs) {
	t.Helper()
	rows, err := db.Query(
		`SELECT adr_id FROM adrs WHERE spec_id = ? ORDER BY adr_id`, specID)
	if err != nil {
		t.Fatalf("query adrs: %v", err)
	}
	defer rows.Close()

	var got []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			t.Fatalf("scan adr id: %v", err)
		}
		got = append(got, id)
	}

	if exp.Count != nil && len(got) != *exp.Count {
		t.Errorf("adr count: got %d (%v), want %d", len(got), got, *exp.Count)
	}
	if len(exp.IDs) > 0 {
		assertSetEqual(t, "adr ids", got, exp.IDs)
	}
}

func assertCrossRefs(t *testing.T, db storage.DB, specID int64, exp *baselineXRefs) {
	t.Helper()

	if exp.Resolved != nil {
		var resolved int
		if err := db.QueryRow(
			`SELECT COUNT(*) FROM cross_references WHERE spec_id = ? AND resolved = 1`,
			specID).Scan(&resolved); err != nil {
			t.Fatalf("count resolved xrefs: %v", err)
		}
		if resolved != *exp.Resolved {
			t.Errorf("resolved cross-refs: got %d, want %d", resolved, *exp.Resolved)
		}
	}

	if exp.Unresolved != nil {
		var unresolved int
		if err := db.QueryRow(
			`SELECT COUNT(*) FROM cross_references WHERE spec_id = ? AND resolved = 0`,
			specID).Scan(&unresolved); err != nil {
			t.Fatalf("count unresolved xrefs: %v", err)
		}
		if unresolved != *exp.Unresolved {
			// Surface the actual unresolved targets to make diagnosis easier.
			rows, _ := db.Query(
				`SELECT ref_target, ref_type, source_line FROM cross_references
				 WHERE spec_id = ? AND resolved = 0`, specID)
			var details []string
			if rows != nil {
				defer rows.Close()
				for rows.Next() {
					var target, typ string
					var line int
					_ = rows.Scan(&target, &typ, &line)
					details = append(details, target+"("+typ+"@L"+itoa(line)+")")
				}
			}
			t.Errorf("unresolved cross-refs: got %d %v, want %d",
				unresolved, details, *exp.Unresolved)
		}
	}

	if len(exp.Targets) > 0 {
		rows, err := db.Query(
			`SELECT DISTINCT ref_target FROM cross_references
			 WHERE spec_id = ? AND ref_type IN ('invariant','app_invariant','adr','app_adr')`,
			specID)
		if err != nil {
			t.Fatalf("query xref targets: %v", err)
		}
		defer rows.Close()
		var got []string
		for rows.Next() {
			var target string
			if err := rows.Scan(&target); err != nil {
				t.Fatalf("scan target: %v", err)
			}
			got = append(got, target)
		}
		assertSetEqual(t, "cross-ref targets", got, exp.Targets)
	}
}

func assertValidation(t *testing.T, db storage.DB, specID int64, exp *baselineValidate) {
	t.Helper()

	// Scope conformance assertions to Check 1 (cross-reference integrity).
	// Other checks (glossary, quality gates, witness coverage) generate noise
	// against the minimal fixtures and are irrelevant to the parser dimensions
	// this suite pins.
	report, err := validator.Validate(db, specID, validator.ValidateOptions{
		CheckIDs: []int{1},
	})
	if err != nil {
		t.Fatalf("validate: %v", err)
	}

	if exp.Errors != nil {
		if report.Errors > *exp.Errors {
			// Collect error messages for diagnostic output.
			var messages []string
			for _, r := range report.Results {
				for _, f := range r.Findings {
					if f.Severity == validator.SeverityError {
						messages = append(messages, f.Message)
					}
				}
			}
			t.Errorf("validation errors: got %d, want <= %d. Errors: %v",
				report.Errors, *exp.Errors, messages)
		}
	}

	if len(exp.ErrorMustContain) > 0 {
		var allMessages []string
		for _, r := range report.Results {
			for _, f := range r.Findings {
				allMessages = append(allMessages, f.Message)
			}
		}
		joined := strings.Join(allMessages, " | ")
		for _, want := range exp.ErrorMustContain {
			if !strings.Contains(joined, want) {
				t.Errorf("expected validation messages to contain %q, got: %v",
					want, allMessages)
			}
		}
	}
}

// assertSetEqual compares two string slices as unordered sets.
func assertSetEqual(t *testing.T, label string, got, want []string) {
	t.Helper()
	gotSet := toSet(got)
	wantSet := toSet(want)

	var missing, extra []string
	for w := range wantSet {
		if !gotSet[w] {
			missing = append(missing, w)
		}
	}
	for g := range gotSet {
		if !wantSet[g] {
			extra = append(extra, g)
		}
	}
	if len(missing) > 0 || len(extra) > 0 {
		sort.Strings(missing)
		sort.Strings(extra)
		t.Errorf("%s set mismatch: missing %v, extra %v", label, missing, extra)
	}
}

func toSet(xs []string) map[string]bool {
	m := make(map[string]bool, len(xs))
	for _, x := range xs {
		m[x] = true
	}
	return m
}

// itoa is a tiny local helper to keep imports minimal in error paths.
func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	neg := n < 0
	if neg {
		n = -n
	}
	var buf [20]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	if neg {
		i--
		buf[i] = '-'
	}
	return string(buf[i:])
}
