package consistency

// Tier 5: SMT-based contradiction detection via Z3 subprocess.
//
// ddis:implements APP-ADR-038 (Z3 subprocess as Tier 5 SMT)
// ddis:maintains APP-INV-021 (SAT encoding fidelity — SMT extension)
//
// Translates semi-formal expressions to SMT-LIB2 format and invokes Z3
// via exec.CommandContext with stdin/stdout. Graceful degradation when
// Z3 is not in PATH. Handles arithmetic (QF_LIA), uninterpreted functions
// (QF_UF), and quantified arithmetic (LIA).

import (
	"bytes"
	"context"
	"database/sql"
	"fmt"
	"os/exec"
	"regexp"
	"strings"
	"time"

	"github.com/wvandaal/ddis/internal/storage"
)

var (
	// Arithmetic patterns: x > 5, count >= 0, latency <= 200
	smtArithRe = regexp.MustCompile(`(?i)(\w+)\s*(>=|<=|>|<|=)\s*(\d+)`)
	// Quantified: FOR ALL x IN S: P(x)
	smtForAllRe = regexp.MustCompile(`(?i)FOR\s+ALL\s+(\w+)(?:\s+IN\s+\w+)?\s*[,:]\s*(.+)`)
	// Predicate: P(x) = true/false
	smtPredRe = regexp.MustCompile(`(?i)(\w+)\(([^)]*)\)\s*=\s*(true|false)`)
	// Function application: f(x) = y
	smtFuncEqRe = regexp.MustCompile(`(?i)(\w+)\(([^)]*)\)\s*=\s*(\w+)`)
	// Implication: A IMPLIES B
	smtImpliesRe = regexp.MustCompile(`(?i)(.+?)\s+IMPLIES\s+(.+)`)
)

// Z3Available returns true if z3 is in PATH.
func Z3Available() bool {
	_, err := exec.LookPath("z3")
	return err == nil
}

// runZ3 invokes z3 as subprocess with SMT-LIB2 on stdin.
// Returns "sat", "unsat", "unknown", or error.
func runZ3(ctx context.Context, smtInput string) (string, error) {
	if _, err := exec.LookPath("z3"); err != nil {
		return "", fmt.Errorf("z3 not in PATH")
	}
	ctx, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, "z3", "-in", "-smt2")
	cmd.Stdin = strings.NewReader(smtInput)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		if ctx.Err() == context.DeadlineExceeded {
			return "", fmt.Errorf("z3 timeout (30s)")
		}
		return "", fmt.Errorf("z3 error: %w (stderr: %s)", err, stderr.String())
	}
	result := strings.TrimSpace(stdout.String())
	// Z3 may emit multiple lines (e.g., warnings); take first line.
	if idx := strings.IndexByte(result, '\n'); idx >= 0 {
		result = strings.TrimSpace(result[:idx])
	}
	return result, nil
}

// smtTranslation holds a translated SMT-LIB2 fragment.
type smtTranslation struct {
	declarations []string // (declare-const ...) or (declare-fun ...)
	assertions   []string // (assert ...)
	logic        string   // QF_LIA, QF_UF, LIA, ALL
}

// TranslateSMTLIB2 converts a semi-formal expression to SMT-LIB2 format.
// Returns (smtlib2_string, logic_name, ok). Returns ok=false if the
// expression cannot be meaningfully translated to SMT.
func TranslateSMTLIB2(semiFormal string) (string, string, bool) {
	semiFormal = strings.TrimSpace(semiFormal)
	semiFormal = strings.TrimPrefix(semiFormal, "```")
	semiFormal = strings.TrimSuffix(semiFormal, "```")
	semiFormal = strings.TrimSpace(semiFormal)

	if semiFormal == "" {
		return "", "", false
	}

	trans := &smtTranslation{logic: "QF_UF"}
	declaredVars := make(map[string]bool)
	declaredFuns := make(map[string]bool)

	lines := strings.Split(semiFormal, "\n")
	anyTranslated := false

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		lower := strings.ToLower(line)
		// Skip context/definition lines
		if strings.HasPrefix(lower, "where") || strings.HasPrefix(line, "//") ||
			strings.HasPrefix(lower, "let ") || strings.HasPrefix(lower, "encoding") ||
			strings.HasPrefix(lower, "numeric") || strings.HasPrefix(lower, "named ") {
			continue
		}

		if translateLine(line, trans, declaredVars, declaredFuns) {
			anyTranslated = true
		}
	}

	if !anyTranslated {
		return "", "", false
	}

	// Build SMT-LIB2 string
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("(set-logic %s)\n", trans.logic))
	for _, d := range trans.declarations {
		sb.WriteString(d)
		sb.WriteByte('\n')
	}
	for _, a := range trans.assertions {
		sb.WriteString(a)
		sb.WriteByte('\n')
	}
	sb.WriteString("(check-sat)\n")
	return sb.String(), trans.logic, true
}

// translateLine translates a single line to SMT-LIB2 assertions.
// Returns true if the line was successfully translated.
func translateLine(line string, trans *smtTranslation, declaredVars, declaredFuns map[string]bool) bool {
	line = strings.TrimSpace(line)

	// 1. FOR ALL quantifier → LIA logic
	if m := smtForAllRe.FindStringSubmatch(line); m != nil {
		varName := m[1]
		body := m[2]
		trans.logic = "LIA"
		ensureVarInt(varName, trans, declaredVars)
		// Translate body as quantified assertion
		bodyAssert := translateAtom(body, trans, declaredVars, declaredFuns)
		if bodyAssert != "" {
			trans.assertions = append(trans.assertions,
				fmt.Sprintf("(assert (forall ((%s Int)) %s))", varName, bodyAssert))
			return true
		}
		return false
	}

	// 2. Implication: A IMPLIES B
	if m := smtImpliesRe.FindStringSubmatch(line); m != nil {
		antecedent := translateAtom(strings.TrimSpace(m[1]), trans, declaredVars, declaredFuns)
		consequent := translateAtom(strings.TrimSpace(m[2]), trans, declaredVars, declaredFuns)
		if antecedent != "" && consequent != "" {
			trans.assertions = append(trans.assertions,
				fmt.Sprintf("(assert (=> %s %s))", antecedent, consequent))
			return true
		}
	}

	// 3. Conjunction: A AND B AND C
	if andRe.MatchString(line) {
		parts := andRe.Split(line, -1)
		var conjuncts []string
		for _, part := range parts {
			part = strings.TrimSpace(part)
			if part == "" {
				continue
			}
			atom := translateAtom(part, trans, declaredVars, declaredFuns)
			if atom != "" {
				conjuncts = append(conjuncts, atom)
			}
		}
		if len(conjuncts) > 0 {
			if len(conjuncts) == 1 {
				trans.assertions = append(trans.assertions,
					fmt.Sprintf("(assert %s)", conjuncts[0]))
			} else {
				trans.assertions = append(trans.assertions,
					fmt.Sprintf("(assert (and %s))", strings.Join(conjuncts, " ")))
			}
			return true
		}
	}

	// 4. Single atom
	atom := translateAtom(line, trans, declaredVars, declaredFuns)
	if atom != "" {
		trans.assertions = append(trans.assertions, fmt.Sprintf("(assert %s)", atom))
		return true
	}

	return false
}

// translateAtom translates an atomic expression to an SMT-LIB2 term.
func translateAtom(text string, trans *smtTranslation, declaredVars, declaredFuns map[string]bool) string {
	text = strings.TrimSpace(text)
	if text == "" {
		return ""
	}

	// NOT X
	if m := notRe.FindStringSubmatch(text); m != nil {
		inner := translateAtom(strings.TrimSpace(m[1]), trans, declaredVars, declaredFuns)
		if inner != "" {
			return fmt.Sprintf("(not %s)", inner)
		}
		return ""
	}

	// Arithmetic: x > 5, count >= 0, latency <= 200
	if m := smtArithRe.FindStringSubmatch(text); m != nil {
		varName := m[1]
		op := m[2]
		value := m[3]
		// Skip if var name looks like a quantifier keyword
		lowerVar := strings.ToLower(varName)
		if lowerVar == "for" || lowerVar == "exists" || lowerVar == "there" {
			return ""
		}
		if trans.logic == "QF_UF" {
			trans.logic = "QF_LIA"
		}
		ensureVarInt(varName, trans, declaredVars)
		smtOp := op
		if op == "=" {
			smtOp = "="
		}
		return fmt.Sprintf("(%s %s %s)", smtOp, varName, value)
	}

	// P(x) = true/false (predicate)
	if m := smtPredRe.FindStringSubmatch(text); m != nil {
		pred := m[1]
		args := m[2]
		val := strings.ToLower(m[3])
		lowerPred := strings.ToLower(pred)
		if lowerPred == "for" || lowerPred == "exists" || lowerPred == "there" {
			return ""
		}
		ensureFunBool(pred, trans, declaredFuns)
		argNames := splitArgs(args)
		for _, a := range argNames {
			ensureVarInt(a, trans, declaredVars)
		}
		call := fmt.Sprintf("(%s %s)", pred, strings.Join(argNames, " "))
		if val == "false" {
			return fmt.Sprintf("(not %s)", call)
		}
		return call
	}

	// f(x) = y (function application with non-boolean result)
	if m := smtFuncEqRe.FindStringSubmatch(text); m != nil {
		funcName := m[1]
		args := m[2]
		result := m[3]
		lowerFunc := strings.ToLower(funcName)
		if lowerFunc == "for" || lowerFunc == "exists" || lowerFunc == "there" ||
			lowerFunc == "true" || lowerFunc == "false" {
			return ""
		}
		ensureFunInt(funcName, trans, declaredFuns)
		argNames := splitArgs(args)
		for _, a := range argNames {
			ensureVarInt(a, trans, declaredVars)
		}
		ensureVarInt(result, trans, declaredVars)
		return fmt.Sprintf("(= (%s %s) %s)", funcName, strings.Join(argNames, " "), result)
	}

	// Fallback: treat as boolean constant
	name := sanitize(text)
	if name != "" && len(name) > 2 {
		ensureVarBool(name, trans, declaredVars)
		return name
	}

	return ""
}

func ensureVarInt(name string, trans *smtTranslation, declared map[string]bool) {
	if !declared[name] {
		declared[name] = true
		trans.declarations = append(trans.declarations,
			fmt.Sprintf("(declare-const %s Int)", name))
	}
}

func ensureVarBool(name string, trans *smtTranslation, declared map[string]bool) {
	if !declared[name] {
		declared[name] = true
		trans.declarations = append(trans.declarations,
			fmt.Sprintf("(declare-const %s Bool)", name))
	}
}

func ensureFunBool(name string, trans *smtTranslation, declared map[string]bool) {
	if !declared[name] {
		declared[name] = true
		trans.declarations = append(trans.declarations,
			fmt.Sprintf("(declare-fun %s (Int) Bool)", name))
	}
}

func ensureFunInt(name string, trans *smtTranslation, declared map[string]bool) {
	if !declared[name] {
		declared[name] = true
		trans.declarations = append(trans.declarations,
			fmt.Sprintf("(declare-fun %s (Int) Int)", name))
	}
}

func splitArgs(args string) []string {
	parts := strings.Split(args, ",")
	var result []string
	for _, p := range parts {
		p = strings.TrimSpace(p)
		if p != "" {
			result = append(result, sanitize(p))
		}
	}
	if len(result) == 0 {
		result = append(result, "x_default")
	}
	return result
}

// analyzeSMT runs Tier 5 Z3-based analysis on semi-formal expressions.
// Only processes expressions that have arithmetic, quantifier, or function
// patterns that Tier 3 (propositional) cannot fully represent.
func analyzeSMT(db *sql.DB, specID int64) ([]Contradiction, int, error) {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, 0, fmt.Errorf("list invariants: %w", err)
	}

	type translatable struct {
		invID string
		smt   string
		logic string
		raw   string
	}

	var translated []translatable
	for _, inv := range invs {
		if inv.SemiFormal == "" {
			continue
		}
		smt, logic, ok := TranslateSMTLIB2(inv.SemiFormal)
		if ok {
			translated = append(translated, translatable{
				invID: inv.InvariantID,
				smt:   smt,
				logic: logic,
				raw:   inv.SemiFormal,
			})
		}
	}

	if len(translated) < 2 {
		return nil, len(invs), nil
	}

	// Check pairwise satisfiability via Z3
	ctx := context.Background()
	var results []Contradiction
	for i := 0; i < len(translated); i++ {
		for j := i + 1; j < len(translated); j++ {
			a, b := translated[i], translated[j]

			// Combine both into a single SMT check
			combined := combineSMT(a.smt, b.smt)
			result, err := runZ3(ctx, combined)
			if err != nil {
				continue // Z3 error on this pair — skip
			}

			if result == "unsat" {
				results = append(results, Contradiction{
					Tier:     TierSMT,
					Type:     SMTUnsatisfiable,
					ElementA: a.invID,
					ElementB: b.invID,
					Description: fmt.Sprintf(
						"%s and %s have unsatisfiable combined constraints (UNSAT via Z3 SMT).",
						a.invID, b.invID,
					),
					Evidence: fmt.Sprintf(
						"%s semi-formal: %q. %s semi-formal: %q. Z3 verdict: unsat.",
						a.invID, truncate(a.raw, 80),
						b.invID, truncate(b.raw, 80),
					),
					Confidence:     0.95,
					ResolutionHint: "Z3 SMT solver proved these constraints are jointly unsatisfiable. Review the semi-formal expressions for genuine conflict.",
				})
			}
		}
	}

	return results, len(translated), nil
}

// combineSMT merges two SMT-LIB2 programs into one for pairwise checking.
// Extracts declarations and assertions from both, deduplicates declarations,
// and uses the most expressive logic.
func combineSMT(a, b string) string {
	aDecls, aAsserts, aLogic := parseSMTLib2(a)
	bDecls, bAsserts, bLogic := parseSMTLib2(b)

	// Merge logic: pick the most expressive
	logic := mergeLogic(aLogic, bLogic)

	// Deduplicate declarations
	seen := make(map[string]bool)
	var decls []string
	for _, d := range append(aDecls, bDecls...) {
		if !seen[d] {
			seen[d] = true
			decls = append(decls, d)
		}
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("(set-logic %s)\n", logic))
	for _, d := range decls {
		sb.WriteString(d)
		sb.WriteByte('\n')
	}
	for _, a := range aAsserts {
		sb.WriteString(a)
		sb.WriteByte('\n')
	}
	for _, a := range bAsserts {
		sb.WriteString(a)
		sb.WriteByte('\n')
	}
	sb.WriteString("(check-sat)\n")
	return sb.String()
}

// parseSMTLib2 extracts declarations, assertions, and logic from an SMT-LIB2 string.
func parseSMTLib2(smt string) (decls, asserts []string, logic string) {
	logic = "QF_UF"
	for _, line := range strings.Split(smt, "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "(set-logic ") {
			logic = strings.TrimSuffix(strings.TrimPrefix(line, "(set-logic "), ")")
		} else if strings.HasPrefix(line, "(declare-") {
			decls = append(decls, line)
		} else if strings.HasPrefix(line, "(assert ") {
			asserts = append(asserts, line)
		}
	}
	return
}

// mergeLogic returns the most expressive logic for combining two formulas.
func mergeLogic(a, b string) string {
	rank := map[string]int{
		"QF_UF":  1,
		"QF_LIA": 2,
		"LIA":    3,
		"ALL":    4,
	}
	ra, ok := rank[a]
	if !ok {
		ra = 4
	}
	rb, ok := rank[b]
	if !ok {
		rb = 4
	}
	// If mixing UF and LIA theories, use ALL
	isUF := a == "QF_UF" || b == "QF_UF"
	isLIA := a == "QF_LIA" || a == "LIA" || b == "QF_LIA" || b == "LIA"
	if isUF && isLIA {
		return "ALL"
	}
	if ra >= rb {
		return a
	}
	return b
}
