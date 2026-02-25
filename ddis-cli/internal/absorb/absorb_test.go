package absorb

import (
	"os"
	"path/filepath"
	"testing"
)

// ---------------------------------------------------------------------------
// wordSet
// ---------------------------------------------------------------------------

func TestWordSet_BasicSplitting(t *testing.T) {
	ws := wordSet("Hello World Foo")
	if !ws["hello"] {
		t.Error("expected 'hello' in word set")
	}
	if !ws["world"] {
		t.Error("expected 'world' in word set")
	}
	if !ws["foo"] {
		t.Error("expected 'foo' in word set")
	}
}

func TestWordSet_PunctuationStripping(t *testing.T) {
	ws := wordSet("hello, world! (test)")
	if !ws["hello"] {
		t.Errorf("expected 'hello' after comma stripping, got keys: %v", ws)
	}
	if !ws["world"] {
		t.Errorf("expected 'world' after exclamation stripping, got keys: %v", ws)
	}
	if !ws["test"] {
		t.Errorf("expected 'test' after paren stripping, got keys: %v", ws)
	}
}

func TestWordSet_ShortWordFiltering(t *testing.T) {
	ws := wordSet("a to the big cat")
	if ws["a"] {
		t.Error("single-char word 'a' should be filtered out (len < 3)")
	}
	if ws["to"] {
		t.Error("two-char word 'to' should be filtered out (len < 3)")
	}
	if ws["the"] {
		t.Error("stop word 'the' should be filtered out by programmingStopWords")
	}
	if !ws["big"] {
		t.Error("expected 'big' to be included")
	}
	if !ws["cat"] {
		t.Error("expected 'cat' to be included")
	}
}

func TestWordSet_EmptyInput(t *testing.T) {
	ws := wordSet("")
	if len(ws) != 0 {
		t.Errorf("expected empty word set for empty input, got %d entries", len(ws))
	}
}

func TestWordSet_Lowercase(t *testing.T) {
	ws := wordSet("UPPER lower MiXeD")
	if !ws["upper"] {
		t.Error("expected 'upper' (lowercased) in word set")
	}
	if !ws["lower"] {
		t.Error("expected 'lower' in word set")
	}
	if !ws["mixed"] {
		t.Error("expected 'mixed' (lowercased) in word set")
	}
}

// ---------------------------------------------------------------------------
// keywordOverlap
// ---------------------------------------------------------------------------

func TestKeywordOverlap_EmptySets(t *testing.T) {
	empty := map[string]bool{}
	nonempty := map[string]bool{"hello": true}

	if v := keywordOverlap(empty, nonempty); v != 0 {
		t.Errorf("expected 0 for empty first set, got %f", v)
	}
	if v := keywordOverlap(nonempty, empty); v != 0 {
		t.Errorf("expected 0 for empty second set, got %f", v)
	}
	if v := keywordOverlap(empty, empty); v != 0 {
		t.Errorf("expected 0 for two empty sets, got %f", v)
	}
}

func TestKeywordOverlap_IdenticalSets(t *testing.T) {
	a := map[string]bool{"hello": true, "world": true}
	b := map[string]bool{"hello": true, "world": true}

	v := keywordOverlap(a, b)
	if v != 1.0 {
		t.Errorf("expected 1.0 for identical sets, got %f", v)
	}
}

func TestKeywordOverlap_PartialOverlap(t *testing.T) {
	a := map[string]bool{"hello": true, "world": true, "foo": true}
	b := map[string]bool{"hello": true, "world": true, "bar": true}

	// intersection = 2, max(3, 3) = 3 => 2/3
	v := keywordOverlap(a, b)
	expected := 2.0 / 3.0
	if v < expected-0.001 || v > expected+0.001 {
		t.Errorf("expected ~%.4f for partial overlap, got %f", expected, v)
	}
}

func TestKeywordOverlap_DisjointSets(t *testing.T) {
	a := map[string]bool{"hello": true, "world": true}
	b := map[string]bool{"foo": true, "bar": true}

	v := keywordOverlap(a, b)
	if v != 0 {
		t.Errorf("expected 0 for disjoint sets, got %f", v)
	}
}

func TestKeywordOverlap_AsymmetricSizes(t *testing.T) {
	a := map[string]bool{"hello": true}
	b := map[string]bool{"hello": true, "world": true, "foo": true}

	// intersection = 1, max(1, 3) = 3 => 1/3
	v := keywordOverlap(a, b)
	expected := 1.0 / 3.0
	if v < expected-0.001 || v > expected+0.001 {
		t.Errorf("expected ~%.4f, got %f", expected, v)
	}
}

// ---------------------------------------------------------------------------
// extractAnnotationTarget
// ---------------------------------------------------------------------------

func TestExtractAnnotationTarget_ValidAnnotation(t *testing.T) {
	input := "// ddis:implements APP-INV-032 (qualifier)"
	got := extractAnnotationTarget(input)
	if got != "APP-INV-032" {
		t.Errorf("expected 'APP-INV-032', got %q", got)
	}
}

func TestExtractAnnotationTarget_NoQualifier(t *testing.T) {
	input := "// ddis:maintains INV-006"
	got := extractAnnotationTarget(input)
	if got != "INV-006" {
		t.Errorf("expected 'INV-006', got %q", got)
	}
}

func TestExtractAnnotationTarget_NoDdisPrefix(t *testing.T) {
	input := "// just a regular comment"
	got := extractAnnotationTarget(input)
	if got != "" {
		t.Errorf("expected empty string for non-ddis comment, got %q", got)
	}
}

func TestExtractAnnotationTarget_ShortInput(t *testing.T) {
	input := "ab"
	got := extractAnnotationTarget(input)
	if got != "" {
		t.Errorf("expected empty string for short input, got %q", got)
	}
}

func TestExtractAnnotationTarget_DdisAtEnd(t *testing.T) {
	// "ddis:" with nothing after — only one part, so should return ""
	input := "// ddis:"
	got := extractAnnotationTarget(input)
	if got != "" {
		t.Errorf("expected empty string for bare ddis:, got %q", got)
	}
}

func TestExtractAnnotationTarget_CaseInsensitive(t *testing.T) {
	input := "// DDIS:implements APP-INV-001 (test)"
	got := extractAnnotationTarget(input)
	if got != "APP-INV-001" {
		t.Errorf("expected 'APP-INV-001', got %q", got)
	}
}

// ---------------------------------------------------------------------------
// suggestElementType
// ---------------------------------------------------------------------------

func TestSuggestElementType_Assertion(t *testing.T) {
	p := Pattern{Type: "assertion"}
	if got := suggestElementType(p); got != "invariant" {
		t.Errorf("assertion => expected 'invariant', got %q", got)
	}
}

func TestSuggestElementType_ErrorReturn(t *testing.T) {
	p := Pattern{Type: "error_return"}
	if got := suggestElementType(p); got != "adr" {
		t.Errorf("error_return => expected 'adr', got %q", got)
	}
}

func TestSuggestElementType_GuardClause(t *testing.T) {
	p := Pattern{Type: "guard_clause"}
	if got := suggestElementType(p); got != "invariant" {
		t.Errorf("guard_clause => expected 'invariant', got %q", got)
	}
}

func TestSuggestElementType_StateTransition(t *testing.T) {
	p := Pattern{Type: "state_transition"}
	if got := suggestElementType(p); got != "state_machine" {
		t.Errorf("state_transition => expected 'state_machine', got %q", got)
	}
}

func TestSuggestElementType_InterfaceDef(t *testing.T) {
	p := Pattern{Type: "interface_def"}
	if got := suggestElementType(p); got != "interface_spec" {
		t.Errorf("interface_def => expected 'interface_spec', got %q", got)
	}
}

func TestSuggestElementType_Unknown(t *testing.T) {
	p := Pattern{Type: "something_else"}
	if got := suggestElementType(p); got != "section" {
		t.Errorf("unknown type => expected 'section', got %q", got)
	}
}

// ---------------------------------------------------------------------------
// ScanPatterns (integration-style, temp dir)
// ---------------------------------------------------------------------------

func TestScanPatterns_FindsAnnotation(t *testing.T) {
	dir := t.TempDir()

	goFile := filepath.Join(dir, "example.go")
	content := `package example

// ddis:implements APP-INV-001 (test annotation)

func Foo() error {
	if x == nil {
		return fmt.Errorf("x is nil")
	}
	return nil
}
`
	if err := os.WriteFile(goFile, []byte(content), 0644); err != nil {
		t.Fatalf("write test file: %v", err)
	}

	result, err := ScanPatterns(dir)
	if err != nil {
		t.Fatalf("ScanPatterns: %v", err)
	}

	if result.TotalPatterns == 0 {
		t.Fatal("expected at least one pattern from annotated Go file")
	}

	foundAnnotation := false
	for _, p := range result.Patterns {
		if p.Type == "annotation" {
			foundAnnotation = true
			if p.Confidence != 1.0 {
				t.Errorf("annotation confidence should be 1.0, got %f", p.Confidence)
			}
			break
		}
	}
	if !foundAnnotation {
		t.Error("expected to find an 'annotation' type pattern")
	}
}

func TestScanPatterns_FindsHeuristicPatterns(t *testing.T) {
	dir := t.TempDir()

	// Use a domain-specific guard clause long enough to pass minPatternLength
	// and complex enough to survive boilerplate filtering.
	goFile := filepath.Join(dir, "heuristic.go")
	content := `package heuristic

func ValidateSpecElement(specID string, element SpecElement, validationCtx *Context) error {
	if specID != "" && element.CrossRefCount > 0 && validationCtx.ActiveSpec != nil && validationCtx.Threshold > 0 {
		return nil
	}
	return nil
}
`
	if err := os.WriteFile(goFile, []byte(content), 0644); err != nil {
		t.Fatalf("write test file: %v", err)
	}

	result, err := ScanPatterns(dir)
	if err != nil {
		t.Fatalf("ScanPatterns: %v", err)
	}

	foundGuard := false
	for _, p := range result.Patterns {
		if p.Type == "guard_clause" {
			foundGuard = true
			break
		}
	}
	if !foundGuard {
		t.Error("expected to find a 'guard_clause' heuristic pattern for domain-specific guard")
	}
}

func TestScanPatterns_EmptyDirectory(t *testing.T) {
	dir := t.TempDir()

	result, err := ScanPatterns(dir)
	if err != nil {
		t.Fatalf("ScanPatterns on empty dir: %v", err)
	}

	if result.TotalPatterns != 0 {
		t.Errorf("expected 0 patterns in empty dir, got %d", result.TotalPatterns)
	}
}

// ---------------------------------------------------------------------------
// estimateDrift
// ---------------------------------------------------------------------------

func TestEstimateDrift_NilReconciliation(t *testing.T) {
	result := &AbsorbResult{Reconciliation: nil}
	if v := estimateDrift(result); v != -1 {
		t.Errorf("nil reconciliation => expected -1, got %f", v)
	}
}

func TestEstimateDrift_EmptyReport(t *testing.T) {
	result := &AbsorbResult{
		Reconciliation: &ReconciliationReport{},
	}
	if v := estimateDrift(result); v != 0 {
		t.Errorf("empty report => expected 0, got %f", v)
	}
}

func TestEstimateDrift_WithItems(t *testing.T) {
	result := &AbsorbResult{
		Reconciliation: &ReconciliationReport{
			UndocumentedBehavior: []UndocumentedItem{
				{Pattern: Pattern{Type: "assertion"}, Suggestion: "invariant"},
				{Pattern: Pattern{Type: "error_return"}, Suggestion: "adr"},
			},
			UnimplementedSpec: []UnimplementedItem{
				{ElementID: "INV-001", ElementType: "invariant", Title: "Test"},
			},
		},
	}

	// 2 undocumented + 1 unimplemented = 3
	if v := estimateDrift(result); v != 3.0 {
		t.Errorf("expected drift of 3, got %f", v)
	}
}

func TestEstimateDrift_OnlyUndocumented(t *testing.T) {
	result := &AbsorbResult{
		Reconciliation: &ReconciliationReport{
			UndocumentedBehavior: []UndocumentedItem{
				{Pattern: Pattern{Type: "assertion"}, Suggestion: "invariant"},
			},
		},
	}
	if v := estimateDrift(result); v != 1.0 {
		t.Errorf("expected drift of 1, got %f", v)
	}
}

func TestEstimateDrift_OnlyUnimplemented(t *testing.T) {
	result := &AbsorbResult{
		Reconciliation: &ReconciliationReport{
			UnimplementedSpec: []UnimplementedItem{
				{ElementID: "INV-001"},
				{ElementID: "INV-002"},
				{ElementID: "INV-003"},
			},
		},
	}
	if v := estimateDrift(result); v != 3.0 {
		t.Errorf("expected drift of 3, got %f", v)
	}
}
