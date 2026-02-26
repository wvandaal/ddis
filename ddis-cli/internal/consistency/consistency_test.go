package consistency

import (
	"testing"
)

// --- SAT solver tests ---

func TestSatisfiable_EmptyCNF(t *testing.T) {
	if !satisfiable(CNF{}) {
		t.Error("empty CNF should be satisfiable")
	}
}

func TestSatisfiable_SinglePositive(t *testing.T) {
	cnf := CNF{
		{{Var: "A", Negate: false}},
	}
	if !satisfiable(cnf) {
		t.Error("single positive literal should be satisfiable")
	}
}

func TestSatisfiable_SingleNegative(t *testing.T) {
	cnf := CNF{
		{{Var: "A", Negate: true}},
	}
	if !satisfiable(cnf) {
		t.Error("single negative literal should be satisfiable")
	}
}

func TestUnsatisfiable_Contradiction(t *testing.T) {
	// A AND NOT A — classic contradiction.
	cnf := CNF{
		{{Var: "A", Negate: false}},
		{{Var: "A", Negate: true}},
	}
	if satisfiable(cnf) {
		t.Error("A AND NOT A should be unsatisfiable")
	}
}

func TestSatisfiable_Implication(t *testing.T) {
	// (¬A ∨ B) AND A — forces B.
	cnf := CNF{
		{{Var: "A", Negate: true}, {Var: "B", Negate: false}},
		{{Var: "A", Negate: false}},
	}
	if !satisfiable(cnf) {
		t.Error("(A→B) AND A should be satisfiable (B=true)")
	}
}

func TestUnsatisfiable_ImplicationChain(t *testing.T) {
	// A→B, B→C, C→¬A, A — circular contradiction.
	cnf := CNF{
		{{Var: "A", Negate: true}, {Var: "B", Negate: false}}, // A→B
		{{Var: "B", Negate: true}, {Var: "C", Negate: false}}, // B→C
		{{Var: "C", Negate: true}, {Var: "A", Negate: true}},  // C→¬A
		{{Var: "A", Negate: false}},                            // A
	}
	if satisfiable(cnf) {
		t.Error("circular implication chain with forced A should be unsatisfiable")
	}
}

func TestSatisfiable_ThreeVarSAT(t *testing.T) {
	// (A ∨ B) AND (¬A ∨ C) AND (¬B ∨ ¬C) — satisfiable.
	cnf := CNF{
		{{Var: "A", Negate: false}, {Var: "B", Negate: false}},
		{{Var: "A", Negate: true}, {Var: "C", Negate: false}},
		{{Var: "B", Negate: true}, {Var: "C", Negate: true}},
	}
	if !satisfiable(cnf) {
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
	expr := "P(x) = true"
	cnf := parseSemiFormal(expr, "INV-001")
	if len(cnf) != 1 {
		t.Fatalf("expected 1 clause, got %d", len(cnf))
	}
	if cnf[0][0].Negate {
		t.Error("P(x) = true should produce a positive literal")
	}
}

func TestParseSemiFormal_NegatedPredicate(t *testing.T) {
	expr := "P(x) = false"
	cnf := parseSemiFormal(expr, "INV-001")
	if len(cnf) != 1 {
		t.Fatalf("expected 1 clause, got %d", len(cnf))
	}
	if !cnf[0][0].Negate {
		t.Error("P(x) = false should produce a negative literal")
	}
}

func TestParseSemiFormal_Implication(t *testing.T) {
	expr := "A IMPLIES B"
	cnf := parseSemiFormal(expr, "INV-002")
	if len(cnf) != 1 {
		t.Fatalf("expected 1 clause, got %d", len(cnf))
	}
	if len(cnf[0]) != 2 {
		t.Fatalf("implication clause should have 2 literals, got %d", len(cnf[0]))
	}
	if !cnf[0][0].Negate || cnf[0][1].Negate {
		t.Error("A IMPLIES B should be (¬A ∨ B)")
	}
}

func TestParseSemiFormal_Conjunction(t *testing.T) {
	expr := "A AND B AND C"
	cnf := parseSemiFormal(expr, "INV-003")
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
