package consistency

import (
	"testing"
)

// --- SAT solver tests ---

func TestSatisfiable_EmptyCNF(t *testing.T) {
	vm := NewVarMap()
	if !Satisfiable(CNF{}, vm) {
		t.Error("empty CNF should be satisfiable")
	}
}

func TestSatisfiable_SinglePositive(t *testing.T) {
	vm := NewVarMap()
	cnf := CNF{
		{{Var: "A", Negate: false}},
	}
	if !Satisfiable(cnf, vm) {
		t.Error("single positive literal should be satisfiable")
	}
}

func TestSatisfiable_SingleNegative(t *testing.T) {
	vm := NewVarMap()
	cnf := CNF{
		{{Var: "A", Negate: true}},
	}
	if !Satisfiable(cnf, vm) {
		t.Error("single negative literal should be satisfiable")
	}
}

func TestUnsatisfiable_Contradiction(t *testing.T) {
	vm := NewVarMap()
	// A AND NOT A — classic contradiction.
	cnf := CNF{
		{{Var: "A", Negate: false}},
		{{Var: "A", Negate: true}},
	}
	if Satisfiable(cnf, vm) {
		t.Error("A AND NOT A should be unsatisfiable")
	}
}

func TestSatisfiable_Implication(t *testing.T) {
	vm := NewVarMap()
	// (¬A ∨ B) AND A — forces B.
	cnf := CNF{
		{{Var: "A", Negate: true}, {Var: "B", Negate: false}},
		{{Var: "A", Negate: false}},
	}
	if !Satisfiable(cnf, vm) {
		t.Error("(A→B) AND A should be satisfiable (B=true)")
	}
}

func TestUnsatisfiable_ImplicationChain(t *testing.T) {
	vm := NewVarMap()
	// A→B, B→C, C→¬A, A — circular contradiction.
	cnf := CNF{
		{{Var: "A", Negate: true}, {Var: "B", Negate: false}}, // A→B
		{{Var: "B", Negate: true}, {Var: "C", Negate: false}}, // B→C
		{{Var: "C", Negate: true}, {Var: "A", Negate: true}},  // C→¬A
		{{Var: "A", Negate: false}},                            // A
	}
	if Satisfiable(cnf, vm) {
		t.Error("circular implication chain with forced A should be unsatisfiable")
	}
}

func TestSatisfiable_ThreeVarSAT(t *testing.T) {
	vm := NewVarMap()
	// (A ∨ B) AND (¬A ∨ C) AND (¬B ∨ ¬C) — satisfiable.
	cnf := CNF{
		{{Var: "A", Negate: false}, {Var: "B", Negate: false}},
		{{Var: "A", Negate: true}, {Var: "C", Negate: false}},
		{{Var: "B", Negate: true}, {Var: "C", Negate: true}},
	}
	if !Satisfiable(cnf, vm) {
		t.Error("3-variable formula should be satisfiable")
	}
}

// --- Heuristic tests ---

func TestExtractForbiddenAction(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"Must NOT use online learning", "use online learning"},
		{"DO NOT return results without a computable score", "return results without a computable score"},
		{"Never modify existing oplog records", "modify existing oplog records"},
		{"Shall not modify files outside root", "modify files outside root"},
		{"No constraint here", ""},
	}
	for _, tt := range tests {
		got := extractForbiddenAction(tt.input)
		if got != tt.want {
			t.Errorf("extractForbiddenAction(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestHasOpposingPolarity(t *testing.T) {
	tests := []struct {
		a, b string
		want bool
	}{
		{"must include all references", "must not include external references", true},
		{"always produce deterministic output", "never produce non-deterministic state", true},
		{"minimize context size", "maximize coverage", true},
		{"parse the manifest", "validate the index", false},
	}
	for _, tt := range tests {
		got := hasOpposingPolarity(tt.a, tt.b)
		if got != tt.want {
			t.Errorf("hasOpposingPolarity(%q, %q) = %v, want %v", tt.a, tt.b, got, tt.want)
		}
	}
}

func TestSignificantWords(t *testing.T) {
	got := significantWords("the quick brown fox must not jump over")
	expected := map[string]bool{"quick": true, "brown": true, "jump": true, "over": true}
	for _, w := range got {
		if !expected[w] {
			t.Errorf("unexpected word %q in significant words", w)
		}
	}
}

func TestActionOverlap(t *testing.T) {
	tests := []struct {
		a, b string
		min  float64
	}{
		{"use online learning in search", "use online learning for indices", 0.5},
		{"compile the binary", "validate the spec", 0.0},
	}
	for _, tt := range tests {
		got := actionOverlap(tt.a, tt.b)
		if got < tt.min {
			t.Errorf("actionOverlap(%q, %q) = %.2f, want >= %.2f", tt.a, tt.b, got, tt.min)
		}
	}
}

// --- Semi-formal parser tests ---

func TestParseSemiFormal_Predicate(t *testing.T) {
	vm := NewVarMap()
	expr := "P(x) = true"
	cnf := ParseSemiFormal(expr, vm)
	if len(cnf) != 1 {
		t.Fatalf("expected 1 clause, got %d", len(cnf))
	}
	if cnf[0][0].Negate {
		t.Error("P(x) = true should produce a positive literal")
	}
}

func TestParseSemiFormal_NegatedPredicate(t *testing.T) {
	vm := NewVarMap()
	expr := "P(x) = false"
	cnf := ParseSemiFormal(expr, vm)
	if len(cnf) != 1 {
		t.Fatalf("expected 1 clause, got %d", len(cnf))
	}
	if !cnf[0][0].Negate {
		t.Error("P(x) = false should produce a negative literal")
	}
}

func TestParseSemiFormal_Implication(t *testing.T) {
	vm := NewVarMap()
	// Use realistic variable names (len > 2) since the parser filters
	// single-letter atoms as noise per APP-INV-021.
	expr := "valid(spec) = true IMPLIES render(spec) = true"
	cnf := ParseSemiFormal(expr, vm)
	if len(cnf) != 1 {
		t.Fatalf("expected 1 clause, got %d", len(cnf))
	}
	if len(cnf[0]) != 2 {
		t.Fatalf("implication clause should have 2 literals, got %d", len(cnf[0]))
	}
	if !cnf[0][0].Negate || cnf[0][1].Negate {
		t.Error("antecedent IMPLIES consequent should be (¬ant ∨ cons)")
	}
}

func TestParseSemiFormal_Conjunction(t *testing.T) {
	vm := NewVarMap()
	// Use predicate forms that the parser recognizes as real atoms.
	expr := "valid(spec) = true AND render(spec) = true AND parse(spec) = true"
	cnf := ParseSemiFormal(expr, vm)
	if len(cnf) != 3 {
		t.Fatalf("expected 3 unit clauses, got %d", len(cnf))
	}
}

// --- Dedup tests ---

func TestDedup(t *testing.T) {
	cs := []Contradiction{
		{ElementA: "A", ElementB: "B", Confidence: 0.5, Type: PolarityInversion},
		{ElementA: "B", ElementB: "A", Confidence: 0.8, Type: GovernanceOverlap},
		{ElementA: "C", ElementB: "D", Confidence: 0.6, Type: NegSpecViolation},
	}
	got := dedup(cs)
	if len(got) != 2 {
		t.Fatalf("expected 2 after dedup, got %d", len(got))
	}
	// The A-B pair should keep the higher-confidence one.
	for _, c := range got {
		if (c.ElementA == "A" && c.ElementB == "B") || (c.ElementA == "B" && c.ElementB == "A") {
			if c.Confidence != 0.8 {
				t.Errorf("expected confidence 0.8 for A-B pair, got %f", c.Confidence)
			}
		}
	}
}

// --- Cosine similarity tests ---

func TestCosineSim_Identical(t *testing.T) {
	a := map[string]float64{"x": 1.0, "y": 2.0}
	sim := cosineSim(a, a)
	if sim < 0.99 {
		t.Errorf("identical vectors should have cosine ~1.0, got %f", sim)
	}
}

func TestCosineSim_Orthogonal(t *testing.T) {
	a := map[string]float64{"x": 1.0}
	b := map[string]float64{"y": 1.0}
	sim := cosineSim(a, b)
	if sim != 0 {
		t.Errorf("orthogonal vectors should have cosine 0, got %f", sim)
	}
}

func TestCosineSim_Empty(t *testing.T) {
	a := map[string]float64{}
	b := map[string]float64{"x": 1.0}
	sim := cosineSim(a, b)
	if sim != 0 {
		t.Errorf("empty vector should give cosine 0, got %f", sim)
	}
}

// --- Truncate tests ---

func TestTruncate(t *testing.T) {
	if truncate("hello world", 20) != "hello world" {
		t.Error("short string should not be truncated")
	}
	got := truncate("hello world, this is long", 15)
	if len(got) > 15 {
		t.Errorf("truncated string should be <= 15 chars, got %d", len(got))
	}
}

// ---------------------------------------------------------------------------
// Behavioral test: APP-INV-019 — Contradiction Graph Soundness
// ddis:tests APP-INV-019
// ---------------------------------------------------------------------------

func TestAPPINV019_NoFalsePositives(t *testing.T) {
	// A negative spec says "Must NOT embed API keys in source files".
	// An invariant says "Source files must not contain embedded API keys".
	// Both AGREE (both negative) — this is NOT a contradiction.
	// The current impliesForbidden uses bag-of-words overlap which flags
	// this as a false positive because the words overlap.

	// Setup: neg spec forbids "embed API keys in source files"
	forbidden := extractForbiddenAction("Must NOT embed API keys in source files")
	if forbidden == "" {
		t.Fatal("expected non-empty forbidden action")
	}

	// An invariant that AGREES with the prohibition (also negative):
	agreeStatement := "Source files must not contain embedded API keys or secrets"
	agreeSemiFormal := "FOR ALL file IN source_files: NOT contains_api_key(file)"

	// This should NOT be flagged as implying the forbidden action,
	// because the invariant is itself a prohibition.
	if impliesForbidden(agreeStatement, agreeSemiFormal, forbidden) {
		t.Error("APP-INV-019 VIOLATED: impliesForbidden flagged a non-contradicting invariant " +
			"(both the neg spec and the invariant agree: don't embed keys). " +
			"This is a false positive — zero false positives required.")
	}

	// An invariant that genuinely CONTRADICTS the prohibition:
	contradictStatement := "All API keys must be embedded directly in configuration source files"
	contradictSemiFormal := "FOR ALL key IN api_keys: embed_in_source(key) = true"

	// This SHOULD be flagged — the invariant positively requires the forbidden action.
	if !impliesForbidden(contradictStatement, contradictSemiFormal, forbidden) {
		t.Error("impliesForbidden missed a genuine contradiction: invariant requires embedding keys while neg spec forbids it")
	}
}

// ---------------------------------------------------------------------------
// Behavioral test: APP-INV-021 — SAT Encoding Fidelity
// ddis:tests APP-INV-021
// ---------------------------------------------------------------------------

func TestAPPINV021_EncodingFidelity(t *testing.T) {
	// Test that real semi-formal expressions from the DDIS CLI spec
	// produce non-empty CNF when parsed.

	vm := NewVarMap()

	realSemiFormals := []struct {
		label string
		expr  string
	}{
		// Simple predicate forms that SHOULD parse
		{"simple-predicate", "P(x) = true"},
		{"simple-implies", "A IMPLIES B"},
		{"simple-conjunction", "A AND B AND C"},
		// Real semi-formals from the spec
		{"APP-INV-002", `FOR ALL db, specID, checks:
  Validate(db, specID, checks).Results = Validate(db, specID, checks).Results`},
		{"APP-INV-004", "authority(graph_with_x) IMPLIES authority(graph_without_x)"},
		{"APP-INV-049", "FOR ALL w IN witnesses: w.type = test IMPLIES EXISTS t IN tests: t.name = w.evidence AND t.result = pass"},
	}

	parsed := 0
	for _, sf := range realSemiFormals {
		cnf := ParseSemiFormal(sf.expr, vm)
		if len(cnf) > 0 {
			parsed++
			t.Logf("  %s: %d clauses", sf.label, len(cnf))
		} else {
			t.Logf("  %s: EMPTY (not parsed)", sf.label)
		}
	}

	// The invariant requires that semi-formal expressions produce non-empty CNF.
	// The first 3 (simple forms) should always parse. The real semi-formals
	// should now also parse with the gophersat rewrite. At least 5/6 expected.
	if parsed < 5 {
		t.Errorf("APP-INV-021 VIOLATED: only %d/%d semi-formals produced non-empty CNF "+
			"(expected at least 5 with gophersat rewrite)", parsed, len(realSemiFormals))
	}

	// Cross-invariant namespace: two invariants sharing a variable name
	// must use the SAME variable so pairwise UNSAT can be detected.
	// CRITICAL: use a SHARED VarMap for both — this is the global namespace.
	sharedVM := NewVarMap()
	cnfA := ParseSemiFormal("render(x) = true", sharedVM)
	cnfB := ParseSemiFormal("render(x) = false", sharedVM)

	if len(cnfA) == 0 || len(cnfB) == 0 {
		t.Fatal("namespace test: predicates must parse (render(x)=true/false)")
	}

	// With global namespace (shared VarMap), both use the SAME variable ID
	// for "render_x", so the combined formula (render_x AND NOT render_x) is UNSAT.
	combined := make(CNF, 0, len(cnfA)+len(cnfB))
	combined = append(combined, cnfA...)
	combined = append(combined, cnfB...)

	if Satisfiable(combined, sharedVM) {
		t.Error("APP-INV-021 VIOLATED: render(x)=true AND render(x)=false should be UNSAT " +
			"with global variable namespace. Variables must share IDs across invariants.")
	}
}
