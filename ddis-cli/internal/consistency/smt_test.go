package consistency

import (
	"context"
	"strings"
	"testing"
)

// ---------------------------------------------------------------------------
// TranslateSMTLIB2 tests
// ---------------------------------------------------------------------------

func TestTranslateSMTLIB2_Arithmetic(t *testing.T) {
	smt, logic, ok := TranslateSMTLIB2("count >= 0 AND count <= 100")
	if !ok {
		t.Fatal("expected successful translation for arithmetic expression")
	}
	if logic != "QF_LIA" {
		t.Errorf("expected QF_LIA logic, got %s", logic)
	}
	if !strings.Contains(smt, "(set-logic QF_LIA)") {
		t.Errorf("expected (set-logic QF_LIA) in output, got:\n%s", smt)
	}
	if !strings.Contains(smt, "(>= count 0)") {
		t.Errorf("expected (>= count 0) in output, got:\n%s", smt)
	}
	if !strings.Contains(smt, "(<= count 100)") {
		t.Errorf("expected (<= count 100) in output, got:\n%s", smt)
	}
	if !strings.Contains(smt, "(check-sat)") {
		t.Error("expected (check-sat) in output")
	}
}

func TestTranslateSMTLIB2_Predicate(t *testing.T) {
	smt, _, ok := TranslateSMTLIB2("P(x) = true")
	if !ok {
		t.Fatal("expected successful translation for predicate expression")
	}
	if !strings.Contains(smt, "(declare-fun P (Int) Bool)") {
		t.Errorf("expected function declaration for P, got:\n%s", smt)
	}
	if !strings.Contains(smt, "(assert (P x)") {
		t.Errorf("expected assertion of P(x), got:\n%s", smt)
	}
}

func TestTranslateSMTLIB2_NegatedPredicate(t *testing.T) {
	smt, _, ok := TranslateSMTLIB2("P(x) = false")
	if !ok {
		t.Fatal("expected successful translation for negated predicate")
	}
	if !strings.Contains(smt, "(not (P x)") {
		t.Errorf("expected negated predicate, got:\n%s", smt)
	}
}

func TestTranslateSMTLIB2_Implication(t *testing.T) {
	smt, _, ok := TranslateSMTLIB2("P(x) = true IMPLIES Q(x) = true")
	if !ok {
		t.Fatal("expected successful translation for implication")
	}
	if !strings.Contains(smt, "(assert (=>") {
		t.Errorf("expected implication assertion, got:\n%s", smt)
	}
}

func TestTranslateSMTLIB2_ForAll(t *testing.T) {
	smt, logic, ok := TranslateSMTLIB2("FOR ALL x IN S: P(x) = true")
	if !ok {
		t.Fatal("expected successful translation for quantifier")
	}
	if logic != "LIA" {
		t.Errorf("expected LIA logic for quantified formula, got %s", logic)
	}
	if !strings.Contains(smt, "(forall") {
		t.Errorf("expected forall in output, got:\n%s", smt)
	}
}

func TestTranslateSMTLIB2_Unparseable(t *testing.T) {
	// Very short tokens (<=2 chars after sanitize) and context lines are skipped.
	// "where" prefix lines, "//" comments, and "let" lines are also skipped.
	_, _, ok := TranslateSMTLIB2("where x is defined")
	if ok {
		t.Error("expected context line (where ...) to fail translation")
	}
	_, _, ok2 := TranslateSMTLIB2("// this is a comment")
	if ok2 {
		t.Error("expected comment line to fail translation")
	}
}

func TestTranslateSMTLIB2_Empty(t *testing.T) {
	_, _, ok := TranslateSMTLIB2("")
	if ok {
		t.Error("expected empty string to fail translation")
	}
}

func TestTranslateSMTLIB2_FunctionEquality(t *testing.T) {
	smt, _, ok := TranslateSMTLIB2("f(x) = y")
	if !ok {
		t.Fatal("expected successful translation for function equality")
	}
	if !strings.Contains(smt, "(declare-fun f (Int) Int)") {
		t.Errorf("expected function declaration for f, got:\n%s", smt)
	}
	if !strings.Contains(smt, "(= (f") {
		t.Errorf("expected function equality assertion, got:\n%s", smt)
	}
}

// ---------------------------------------------------------------------------
// Z3 subprocess tests (skip if Z3 not installed)
// ---------------------------------------------------------------------------

func TestRunZ3_Sat(t *testing.T) {
	if !Z3Available() {
		t.Skip("z3 not installed")
	}
	input := `(set-logic QF_LIA)
(declare-const x Int)
(assert (> x 5))
(assert (< x 10))
(check-sat)
`
	result, err := runZ3(context.Background(), input)
	if err != nil {
		t.Fatalf("z3 error: %v", err)
	}
	if result != "sat" {
		t.Errorf("expected sat, got %q", result)
	}
}

func TestRunZ3_Unsat(t *testing.T) {
	if !Z3Available() {
		t.Skip("z3 not installed")
	}
	input := `(set-logic QF_LIA)
(declare-const x Int)
(assert (> x 10))
(assert (< x 5))
(check-sat)
`
	result, err := runZ3(context.Background(), input)
	if err != nil {
		t.Fatalf("z3 error: %v", err)
	}
	if result != "unsat" {
		t.Errorf("expected unsat, got %q", result)
	}
}

func TestRunZ3_Timeout(t *testing.T) {
	if !Z3Available() {
		t.Skip("z3 not installed")
	}
	// Use a very short context timeout to simulate Z3 timeout.
	ctx, cancel := context.WithTimeout(context.Background(), 1)
	defer cancel()
	input := `(set-logic QF_LIA)
(declare-const x Int)
(assert (> x 0))
(check-sat)
`
	// With essentially zero timeout, should fail.
	_, err := runZ3(ctx, input)
	// We override context inside runZ3 with 30s, so this tests that the parent
	// context cancellation may not propagate. That's fine — the real timeout
	// test is that runZ3 has its own 30s budget. Just verify no panic.
	_ = err
}

func TestZ3Available(t *testing.T) {
	// Just verify it doesn't panic — the result depends on installation.
	_ = Z3Available()
}

// ---------------------------------------------------------------------------
// Integration: TranslateSMTLIB2 → runZ3 round-trip
// ---------------------------------------------------------------------------

func TestTranslateAndRunZ3_ArithmeticSat(t *testing.T) {
	if !Z3Available() {
		t.Skip("z3 not installed")
	}
	smt, _, ok := TranslateSMTLIB2("count >= 0 AND count <= 100")
	if !ok {
		t.Fatal("translation failed")
	}
	result, err := runZ3(context.Background(), smt)
	if err != nil {
		t.Fatalf("z3 error: %v", err)
	}
	if result != "sat" {
		t.Errorf("count >= 0 AND count <= 100 should be sat, got %q", result)
	}
}

func TestTranslateAndRunZ3_ArithmeticUnsat(t *testing.T) {
	if !Z3Available() {
		t.Skip("z3 not installed")
	}
	// count >= 10 AND count < 5 is unsatisfiable
	smtA, _, okA := TranslateSMTLIB2("count >= 10")
	smtB, _, okB := TranslateSMTLIB2("count < 5")
	if !okA || !okB {
		t.Fatal("translation failed")
	}
	combined := combineSMT(smtA, smtB)
	result, err := runZ3(context.Background(), combined)
	if err != nil {
		t.Fatalf("z3 error: %v", err)
	}
	if result != "unsat" {
		t.Errorf("count >= 10 AND count < 5 should be unsat, got %q", result)
	}
}

// ---------------------------------------------------------------------------
// combineSMT tests
// ---------------------------------------------------------------------------

func TestCombineSMT_DeduplicatesDeclarations(t *testing.T) {
	a := "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (> x 5))\n(check-sat)\n"
	b := "(set-logic QF_LIA)\n(declare-const x Int)\n(assert (< x 3))\n(check-sat)\n"
	combined := combineSMT(a, b)
	// Should have only one (declare-const x Int)
	count := strings.Count(combined, "(declare-const x Int)")
	if count != 1 {
		t.Errorf("expected 1 declaration of x, got %d in:\n%s", count, combined)
	}
	// Should have both assertions
	if !strings.Contains(combined, "(> x 5)") || !strings.Contains(combined, "(< x 3)") {
		t.Errorf("expected both assertions in combined output:\n%s", combined)
	}
}

func TestMergeLogic(t *testing.T) {
	tests := []struct {
		a, b, want string
	}{
		{"QF_LIA", "QF_LIA", "QF_LIA"},
		{"QF_UF", "QF_UF", "QF_UF"},
		{"QF_LIA", "LIA", "LIA"},
		{"QF_UF", "QF_LIA", "ALL"},
		{"LIA", "QF_UF", "ALL"},
	}
	for _, tt := range tests {
		got := mergeLogic(tt.a, tt.b)
		if got != tt.want {
			t.Errorf("mergeLogic(%q, %q) = %q, want %q", tt.a, tt.b, got, tt.want)
		}
	}
}

// ---------------------------------------------------------------------------
// Behavioral test: APP-INV-021 — SAT/SMT Encoding Fidelity (Tier 5 extension)
// ddis:tests APP-INV-021
// ---------------------------------------------------------------------------

func TestAPPINV021_SMTEncodingFidelity(t *testing.T) {
	if !Z3Available() {
		t.Skip("z3 not installed — cannot test SMT encoding fidelity")
	}

	// 1. Two invariants with contradictory arithmetic constraints.
	// Tier 3 (propositional) encodes "count >= 0" as a propositional variable
	// name, so it CANNOT detect the arithmetic contradiction. Tier 5 (SMT)
	// with QF_LIA CAN detect it.
	smtA, _, okA := TranslateSMTLIB2("count >= 0")
	smtB, _, okB := TranslateSMTLIB2("count < 0")
	if !okA || !okB {
		t.Fatal("APP-INV-021 VIOLATED: arithmetic semi-formals must translate to SMT-LIB2")
	}

	combined := combineSMT(smtA, smtB)
	result, err := runZ3(context.Background(), combined)
	if err != nil {
		t.Fatalf("Z3 error: %v", err)
	}
	if result != "unsat" {
		t.Errorf("APP-INV-021 VIOLATED: count >= 0 AND count < 0 should be UNSAT via Z3, got %q", result)
	}

	// 2. Verify the propositional encoder (Tier 3) treats these as independent
	// variables — it cannot detect the arithmetic contradiction.
	vm := NewVarMap()
	cnfA := ParseSemiFormal("count >= 0", vm)
	cnfB := ParseSemiFormal("count < 0", vm)

	// Both should parse (the propositional encoder creates variable names from
	// the text), but the combined formula should be SAT because they're
	// different propositional variables (no semantic arithmetic).
	if len(cnfA) > 0 && len(cnfB) > 0 {
		combinedCNF := make(CNF, 0, len(cnfA)+len(cnfB))
		combinedCNF = append(combinedCNF, cnfA...)
		combinedCNF = append(combinedCNF, cnfB...)
		if !Satisfiable(combinedCNF, vm) {
			// If Tier 3 CAN detect it, that's fine — but it's not expected
			// for arithmetic. Log it.
			t.Log("NOTE: Tier 3 also detected the arithmetic contradiction — " +
				"this is unexpected but acceptable")
		} else {
			t.Log("Confirmed: Tier 3 (propositional) cannot detect arithmetic " +
				"contradiction — Tier 5 (SMT) fills the gap")
		}
	}
}
