package annotate

import (
	"os"
	"path/filepath"
	"testing"
)

// ---------------------------------------------------------------------------
// ParseAnnotation
// ---------------------------------------------------------------------------

func TestParseAnnotation_ValidImplements(t *testing.T) {
	input := "ddis:implements APP-INV-001 (round-trip)"
	a := ParseAnnotation(input)
	if a == nil {
		t.Fatal("expected non-nil Annotation")
	}
	if a.Verb != "implements" {
		t.Errorf("expected verb 'implements', got %q", a.Verb)
	}
	if a.Target != "APP-INV-001" {
		t.Errorf("expected target 'APP-INV-001', got %q", a.Target)
	}
	if a.Qualifier != "round-trip" {
		t.Errorf("expected qualifier 'round-trip', got %q", a.Qualifier)
	}
}

func TestParseAnnotation_ValidMaintains(t *testing.T) {
	input := "ddis:maintains INV-006"
	a := ParseAnnotation(input)
	if a == nil {
		t.Fatal("expected non-nil Annotation")
	}
	if a.Verb != "maintains" {
		t.Errorf("expected verb 'maintains', got %q", a.Verb)
	}
	if a.Target != "INV-006" {
		t.Errorf("expected target 'INV-006', got %q", a.Target)
	}
	if a.Qualifier != "" {
		t.Errorf("expected empty qualifier, got %q", a.Qualifier)
	}
}

func TestParseAnnotation_ValidAppADR(t *testing.T) {
	input := "ddis:interfaces APP-ADR-024 (bilateral specification)"
	a := ParseAnnotation(input)
	if a == nil {
		t.Fatal("expected non-nil Annotation")
	}
	if a.Verb != "interfaces" {
		t.Errorf("expected verb 'interfaces', got %q", a.Verb)
	}
	if a.Target != "APP-ADR-024" {
		t.Errorf("expected target 'APP-ADR-024', got %q", a.Target)
	}
	if a.Qualifier != "bilateral specification" {
		t.Errorf("expected qualifier 'bilateral specification', got %q", a.Qualifier)
	}
}

func TestParseAnnotation_ValidGateTarget(t *testing.T) {
	input := "ddis:validates-via Gate-3"
	a := ParseAnnotation(input)
	if a == nil {
		t.Fatal("expected non-nil Annotation")
	}
	if a.Verb != "validates-via" {
		t.Errorf("expected verb 'validates-via', got %q", a.Verb)
	}
	if a.Target != "Gate-3" {
		t.Errorf("expected target 'Gate-3', got %q", a.Target)
	}
}

func TestParseAnnotation_ValidSectionRef(t *testing.T) {
	input := "ddis:relates-to S3.2.1"
	a := ParseAnnotation(input)
	if a == nil {
		t.Fatal("expected non-nil Annotation")
	}
	if a.Target != "S3.2.1" {
		t.Errorf("expected target 'S3.2.1', got %q", a.Target)
	}
}

func TestParseAnnotation_InvalidLine(t *testing.T) {
	cases := []string{
		"just a regular comment",
		"ddis: no verb here",
		"",
		"ddis:unknown-verb APP-INV-001",
		"something else entirely",
	}
	for _, input := range cases {
		a := ParseAnnotation(input)
		if a != nil {
			t.Errorf("expected nil for input %q, got %+v", input, a)
		}
	}
}

func TestParseAnnotation_AllVerbs(t *testing.T) {
	verbs := []string{
		"maintains", "implements", "interfaces", "tests",
		"validates-via", "postcondition", "relates-to", "satisfies",
	}
	for _, verb := range verbs {
		input := "ddis:" + verb + " APP-INV-001"
		a := ParseAnnotation(input)
		if a == nil {
			t.Errorf("expected non-nil Annotation for verb %q", verb)
			continue
		}
		if a.Verb != verb {
			t.Errorf("expected verb %q, got %q", verb, a.Verb)
		}
	}
}

// ---------------------------------------------------------------------------
// ExtractComment
// ---------------------------------------------------------------------------

func TestExtractComment_GoStyle(t *testing.T) {
	got := ExtractComment("  // ddis:implements APP-INV-001", []string{"//"})
	if got != "ddis:implements APP-INV-001" {
		t.Errorf("expected stripped Go comment, got %q", got)
	}
}

func TestExtractComment_PythonStyle(t *testing.T) {
	got := ExtractComment("  # ddis:maintains INV-006", []string{"#"})
	if got != "ddis:maintains INV-006" {
		t.Errorf("expected stripped Python comment, got %q", got)
	}
}

func TestExtractComment_HTMLStyle(t *testing.T) {
	got := ExtractComment("<!-- ddis:implements APP-INV-001 -->", []string{"<!--"})
	if got != "ddis:implements APP-INV-001" {
		t.Errorf("expected stripped HTML comment, got %q", got)
	}
}

func TestExtractComment_SQLStyle(t *testing.T) {
	got := ExtractComment("  -- ddis:implements APP-INV-001", []string{"--"})
	if got != "ddis:implements APP-INV-001" {
		t.Errorf("expected stripped SQL comment, got %q", got)
	}
}

func TestExtractComment_NoMatch(t *testing.T) {
	got := ExtractComment("func main() {}", []string{"//"})
	if got != "" {
		t.Errorf("expected empty string for non-comment line, got %q", got)
	}
}

func TestExtractComment_EmptyLine(t *testing.T) {
	got := ExtractComment("", []string{"//"})
	if got != "" {
		t.Errorf("expected empty string for empty line, got %q", got)
	}
}

// ---------------------------------------------------------------------------
// LookupCommentPrefixes
// ---------------------------------------------------------------------------

func TestLookupCommentPrefixes_Go(t *testing.T) {
	prefixes, lang := LookupCommentPrefixes("main.go")
	if len(prefixes) != 1 || prefixes[0] != "//" {
		t.Errorf("expected ['//'] for .go, got %v", prefixes)
	}
	if lang != "Go" {
		t.Errorf("expected language 'Go', got %q", lang)
	}
}

func TestLookupCommentPrefixes_Python(t *testing.T) {
	prefixes, lang := LookupCommentPrefixes("script.py")
	if len(prefixes) != 1 || prefixes[0] != "#" {
		t.Errorf("expected ['#'] for .py, got %v", prefixes)
	}
	if lang != "Python" {
		t.Errorf("expected language 'Python', got %q", lang)
	}
}

func TestLookupCommentPrefixes_Rust(t *testing.T) {
	prefixes, lang := LookupCommentPrefixes("lib.rs")
	if len(prefixes) != 1 || prefixes[0] != "//" {
		t.Errorf("expected ['//'] for .rs, got %v", prefixes)
	}
	if lang != "Rust" {
		t.Errorf("expected language 'Rust', got %q", lang)
	}
}

func TestLookupCommentPrefixes_SQL(t *testing.T) {
	prefixes, lang := LookupCommentPrefixes("schema.sql")
	if len(prefixes) != 1 || prefixes[0] != "--" {
		t.Errorf("expected ['--'] for .sql, got %v", prefixes)
	}
	if lang != "SQL" {
		t.Errorf("expected language 'SQL', got %q", lang)
	}
}

func TestLookupCommentPrefixes_HTML(t *testing.T) {
	prefixes, lang := LookupCommentPrefixes("page.html")
	if len(prefixes) != 1 || prefixes[0] != "<!--" {
		t.Errorf("expected ['<!--'] for .html, got %v", prefixes)
	}
	if lang != "HTML" {
		t.Errorf("expected language 'HTML', got %q", lang)
	}
}

func TestLookupCommentPrefixes_Unknown(t *testing.T) {
	prefixes, lang := LookupCommentPrefixes("data.xyz")
	if prefixes != nil {
		t.Errorf("expected nil for unknown extension, got %v", prefixes)
	}
	if lang != "" {
		t.Errorf("expected empty language for unknown extension, got %q", lang)
	}
}

func TestLookupCommentPrefixes_CaseInsensitive(t *testing.T) {
	prefixes, lang := LookupCommentPrefixes("Main.GO")
	if len(prefixes) != 1 || prefixes[0] != "//" {
		t.Errorf("expected ['//'] for .GO (case insensitive), got %v", prefixes)
	}
	if lang != "Go" {
		t.Errorf("expected language 'Go', got %q", lang)
	}
}

// ---------------------------------------------------------------------------
// AnnotationRe (regex direct testing)
// ---------------------------------------------------------------------------

func TestAnnotationRe_ValidPatterns(t *testing.T) {
	cases := []struct {
		input string
		verb  string
		target string
	}{
		{"ddis:implements APP-INV-001", "implements", "APP-INV-001"},
		{"ddis:maintains INV-006", "maintains", "INV-006"},
		{"ddis:interfaces APP-ADR-024 (bilateral)", "interfaces", "APP-ADR-024"},
		{"ddis:validates-via Gate-3", "validates-via", "Gate-3"},
		{"ddis:tests ADR-015", "tests", "ADR-015"},
		{"ddis:postcondition APP-INV-032 (symmetric)", "postcondition", "APP-INV-032"},
		{"ddis:relates-to S3.2.1", "relates-to", "S3.2.1"},
		{"ddis:satisfies APP-INV-035", "satisfies", "APP-INV-035"},
	}

	for _, tc := range cases {
		m := AnnotationRe.FindStringSubmatch(tc.input)
		if m == nil {
			t.Errorf("AnnotationRe did not match %q", tc.input)
			continue
		}
		if m[1] != tc.verb {
			t.Errorf("for %q: expected verb %q, got %q", tc.input, tc.verb, m[1])
		}
		if m[2] != tc.target {
			t.Errorf("for %q: expected target %q, got %q", tc.input, tc.target, m[2])
		}
	}
}

func TestAnnotationRe_InvalidPatterns(t *testing.T) {
	cases := []string{
		"ddis:unknown-verb APP-INV-001",
		"implements APP-INV-001",
		"ddis: APP-INV-001",
		"ddis:implements NOTVALID",
		"ddis:implements",
	}
	for _, input := range cases {
		m := AnnotationRe.FindStringSubmatch(input)
		if m != nil {
			t.Errorf("AnnotationRe should NOT match %q, got %v", input, m)
		}
	}
}

// ---------------------------------------------------------------------------
// Scan (integration-style, temp dir)
// ---------------------------------------------------------------------------

func TestScan_FindsAnnotatedFiles(t *testing.T) {
	dir := t.TempDir()

	goFile := filepath.Join(dir, "example.go")
	content := `package example

// ddis:implements APP-INV-001 (round-trip)
func Foo() {}

// ddis:maintains APP-ADR-002 (design decision)
func Bar() {}
`
	if err := os.WriteFile(goFile, []byte(content), 0644); err != nil {
		t.Fatalf("write test file: %v", err)
	}

	result, err := Scan(ScanOptions{Root: dir})
	if err != nil {
		t.Fatalf("Scan: %v", err)
	}

	if result.TotalFound != 2 {
		t.Errorf("expected 2 annotations, got %d", result.TotalFound)
	}
	if result.FilesScanned < 1 {
		t.Errorf("expected at least 1 file scanned, got %d", result.FilesScanned)
	}

	// Verify first annotation
	if len(result.Annotations) >= 1 {
		a := result.Annotations[0]
		if a.Verb != "implements" {
			t.Errorf("first annotation verb: expected 'implements', got %q", a.Verb)
		}
		if a.Target != "APP-INV-001" {
			t.Errorf("first annotation target: expected 'APP-INV-001', got %q", a.Target)
		}
		if a.Qualifier != "round-trip" {
			t.Errorf("first annotation qualifier: expected 'round-trip', got %q", a.Qualifier)
		}
		if a.Language != "Go" {
			t.Errorf("first annotation language: expected 'Go', got %q", a.Language)
		}
	}
}

func TestScan_MultiLanguage(t *testing.T) {
	dir := t.TempDir()

	goFile := filepath.Join(dir, "example.go")
	goContent := `package example
// ddis:implements APP-INV-001
func Foo() {}
`
	pyFile := filepath.Join(dir, "script.py")
	pyContent := `# ddis:maintains APP-ADR-002 (design)
def bar():
    pass
`
	if err := os.WriteFile(goFile, []byte(goContent), 0644); err != nil {
		t.Fatalf("write Go file: %v", err)
	}
	if err := os.WriteFile(pyFile, []byte(pyContent), 0644); err != nil {
		t.Fatalf("write Python file: %v", err)
	}

	result, err := Scan(ScanOptions{Root: dir})
	if err != nil {
		t.Fatalf("Scan: %v", err)
	}

	if result.TotalFound != 2 {
		t.Errorf("expected 2 annotations across Go and Python, got %d", result.TotalFound)
	}

	if result.ByLanguage["Go"] != 1 {
		t.Errorf("expected 1 Go annotation, got %d", result.ByLanguage["Go"])
	}
	if result.ByLanguage["Python"] != 1 {
		t.Errorf("expected 1 Python annotation, got %d", result.ByLanguage["Python"])
	}
}

func TestScan_EmptyDirectory(t *testing.T) {
	dir := t.TempDir()

	result, err := Scan(ScanOptions{Root: dir})
	if err != nil {
		t.Fatalf("Scan on empty dir: %v", err)
	}

	if result.TotalFound != 0 {
		t.Errorf("expected 0 annotations in empty dir, got %d", result.TotalFound)
	}
}

func TestScan_SkipsUnrecognizedExtension(t *testing.T) {
	dir := t.TempDir()

	// Write a file with unrecognized extension
	unknownFile := filepath.Join(dir, "data.xyz")
	if err := os.WriteFile(unknownFile, []byte("ddis:implements APP-INV-001"), 0644); err != nil {
		t.Fatalf("write unknown file: %v", err)
	}

	result, err := Scan(ScanOptions{Root: dir})
	if err != nil {
		t.Fatalf("Scan: %v", err)
	}

	if result.TotalFound != 0 {
		t.Errorf("expected 0 annotations for unrecognized file, got %d", result.TotalFound)
	}
	if result.FilesSkipped < 1 {
		t.Errorf("expected at least 1 skipped file, got %d", result.FilesSkipped)
	}
}

func TestScan_EmptyRoot(t *testing.T) {
	_, err := Scan(ScanOptions{Root: ""})
	if err == nil {
		t.Error("expected error for empty root")
	}
}

func TestScan_ByVerbCounts(t *testing.T) {
	dir := t.TempDir()

	goFile := filepath.Join(dir, "example.go")
	content := `package example

// ddis:implements APP-INV-001
// ddis:implements APP-INV-002
// ddis:maintains APP-ADR-001
`
	if err := os.WriteFile(goFile, []byte(content), 0644); err != nil {
		t.Fatalf("write test file: %v", err)
	}

	result, err := Scan(ScanOptions{Root: dir})
	if err != nil {
		t.Fatalf("Scan: %v", err)
	}

	if result.ByVerb["implements"] != 2 {
		t.Errorf("expected 2 implements, got %d", result.ByVerb["implements"])
	}
	if result.ByVerb["maintains"] != 1 {
		t.Errorf("expected 1 maintains, got %d", result.ByVerb["maintains"])
	}
}
