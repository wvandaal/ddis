package consistency

// Tier 3: SAT-based contradiction detection.
// Parses semi-formal expressions into propositional clauses (CNF) and checks
// satisfiability using a pure-Go DPLL solver. UNSAT = contradiction.
//
// Per APP-ADR-034: ~80% of semi-formal expressions are parseable. Unparseable
// expressions are skipped (silent degradation to Tier 4 heuristics).

import (
	"database/sql"
	"fmt"
	"regexp"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// Literal represents a propositional variable (positive or negated).
type Literal struct {
	Var    string
	Negate bool
}

// Clause is a disjunction of literals: (A ∨ ¬B ∨ C).
type Clause []Literal

// CNF is a conjunction of clauses: (A ∨ B) ∧ (¬A ∨ C).
type CNF []Clause

var (
	forAllRe    = regexp.MustCompile(`(?i)FOR\s+ALL\s+(\w+)(?:\s+IN\s+(\w+))?`)
	existsRe    = regexp.MustCompile(`(?i)(?:THERE\s+)?EXISTS?\s+(\w+)(?:\s+IN\s+(\w+))?`)
	impliesRe   = regexp.MustCompile(`(?i)(\w[\w\s]*?)\s*(?:IMPLIES|→|=>)\s*(\w[\w\s]*?)$`)
	equalsRe    = regexp.MustCompile(`(?i)(\w[\w\s]*?)\s*(?:=|EQUALS)\s*(\w[\w\s]*?)$`)
	andRe       = regexp.MustCompile(`(?i)\s+AND\s+`)
	orRe        = regexp.MustCompile(`(?i)\s+OR\s+`)
	notRe       = regexp.MustCompile(`(?i)^NOT\s+(.+)$`)
	predicateRe = regexp.MustCompile(`(?i)(\w+)\(([^)]*)\)\s*=\s*(true|false)`)
)

// analyzeSAT runs Tier 3 SAT-based analysis on semi-formal expressions.
func analyzeSAT(db *sql.DB, specID int64) ([]Contradiction, int, error) {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, 0, fmt.Errorf("list invariants: %w", err)
	}

	// Parse semi-formal expressions into CNF.
	type parsed struct {
		invID   string
		clauses CNF
		raw     string
	}

	var parsedInvs []parsed
	for _, inv := range invs {
		if inv.SemiFormal == "" {
			continue
		}
		cnf := parseSemiFormal(inv.SemiFormal, inv.InvariantID)
		if len(cnf) > 0 {
			parsedInvs = append(parsedInvs, parsed{
				invID:   inv.InvariantID,
				clauses: cnf,
				raw:     inv.SemiFormal,
			})
		}
	}

	if len(parsedInvs) < 2 {
		return nil, len(invs), nil
	}

	// Check pairwise satisfiability: for each pair, combine their clauses
	// and check if the combined set is satisfiable.
	var results []Contradiction
	for i := 0; i < len(parsedInvs); i++ {
		for j := i + 1; j < len(parsedInvs); j++ {
			a, b := parsedInvs[i], parsedInvs[j]

			// Combine clauses.
			combined := make(CNF, 0, len(a.clauses)+len(b.clauses))
			combined = append(combined, a.clauses...)
			combined = append(combined, b.clauses...)

			// Run DPLL.
			if !satisfiable(combined) {
				results = append(results, Contradiction{
					Tier:     TierSAT,
					Type:     SATUnsatisfiable,
					ElementA: a.invID,
					ElementB: b.invID,
					Description: fmt.Sprintf(
						"%s and %s have unsatisfiable combined constraints (UNSAT).",
						a.invID, b.invID,
					),
					Evidence: fmt.Sprintf(
						"%s semi-formal: %q. %s semi-formal: %q. Combined: %d clauses, UNSAT.",
						a.invID, truncate(a.raw, 80),
						b.invID, truncate(b.raw, 80),
						len(combined),
					),
					Confidence:     0.85,
					ResolutionHint: "The propositional encodings of these invariants are jointly unsatisfiable. Verify the encoding is faithful.",
				})
			}
		}
	}

	return results, len(parsedInvs), nil
}

// parseSemiFormal converts a semi-formal expression to CNF.
// Returns empty CNF if the expression cannot be parsed.
func parseSemiFormal(expr, invID string) CNF {
	// Normalize whitespace and remove code-fence markers.
	expr = strings.TrimSpace(expr)
	expr = strings.TrimPrefix(expr, "```")
	expr = strings.TrimSuffix(expr, "```")
	expr = strings.TrimSpace(expr)

	var cnf CNF

	// Split on newlines and process each line.
	lines := strings.Split(expr, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "WHERE") || strings.HasPrefix(line, "//") {
			continue
		}

		// Try to parse as a predicate assertion: P(x) = true/false.
		if m := predicateRe.FindStringSubmatch(line); m != nil {
			pred := strings.TrimSpace(m[1])
			args := strings.TrimSpace(m[2])
			val := strings.ToLower(strings.TrimSpace(m[3]))

			varName := fmt.Sprintf("%s_%s_%s", invID, pred, sanitize(args))
			if val == "true" {
				cnf = append(cnf, Clause{{Var: varName, Negate: false}})
			} else {
				cnf = append(cnf, Clause{{Var: varName, Negate: true}})
			}
			continue
		}

		// Try to parse implications: A IMPLIES B → ¬A ∨ B.
		if m := impliesRe.FindStringSubmatch(line); m != nil {
			antecedent := sanitize(strings.TrimSpace(m[1]))
			consequent := sanitize(strings.TrimSpace(m[2]))
			varA := fmt.Sprintf("%s_%s", invID, antecedent)
			varB := fmt.Sprintf("%s_%s", invID, consequent)
			cnf = append(cnf, Clause{
				{Var: varA, Negate: true},
				{Var: varB, Negate: false},
			})
			continue
		}

		// Try AND: split into conjuncts.
		if andRe.MatchString(line) {
			parts := andRe.Split(line, -1)
			for _, part := range parts {
				part = strings.TrimSpace(part)
				if part == "" {
					continue
				}
				lit := parseLiteral(part, invID)
				if lit.Var != "" {
					cnf = append(cnf, Clause{lit})
				}
			}
			continue
		}

		// Try as a simple assertion.
		lit := parseLiteral(line, invID)
		if lit.Var != "" {
			cnf = append(cnf, Clause{lit})
		}
	}

	return cnf
}

// parseLiteral parses a single literal from text.
func parseLiteral(text, invID string) Literal {
	text = strings.TrimSpace(text)
	if text == "" {
		return Literal{}
	}

	// Check for negation.
	if m := notRe.FindStringSubmatch(text); m != nil {
		inner := sanitize(strings.TrimSpace(m[1]))
		if inner == "" {
			return Literal{}
		}
		return Literal{Var: fmt.Sprintf("%s_%s", invID, inner), Negate: true}
	}

	name := sanitize(text)
	if name == "" {
		return Literal{}
	}
	return Literal{Var: fmt.Sprintf("%s_%s", invID, name), Negate: false}
}

// sanitize reduces a string to a safe variable name.
func sanitize(s string) string {
	s = strings.TrimSpace(s)
	s = strings.Map(func(r rune) rune {
		if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') || r == '_' {
			return r
		}
		return '_'
	}, s)
	// Collapse consecutive underscores and trim.
	for strings.Contains(s, "__") {
		s = strings.ReplaceAll(s, "__", "_")
	}
	s = strings.Trim(s, "_")
	if len(s) > 60 {
		s = s[:60]
	}
	return s
}

// satisfiable checks if a CNF formula is satisfiable using DPLL.
func satisfiable(cnf CNF) bool {
	if len(cnf) == 0 {
		return true
	}

	// Collect all variables.
	vars := make(map[string]bool)
	for _, clause := range cnf {
		for _, lit := range clause {
			vars[lit.Var] = true
		}
	}

	assignment := make(map[string]bool)
	return dpll(cnf, assignment, vars)
}

// dpll implements the Davis–Putnam–Logemann–Loveland algorithm.
func dpll(cnf CNF, assignment map[string]bool, vars map[string]bool) bool {
	// Unit propagation.
	cnf, assignment = unitPropagate(cnf, assignment)

	// Check for empty clause (contradiction).
	for _, clause := range cnf {
		if len(clause) == 0 {
			return false
		}
	}

	// Check if all clauses are satisfied.
	if len(cnf) == 0 {
		return true
	}

	// Pure literal elimination.
	cnf, assignment = pureLiteral(cnf, assignment)
	if len(cnf) == 0 {
		return true
	}
	for _, clause := range cnf {
		if len(clause) == 0 {
			return false
		}
	}

	// Choose an unassigned variable.
	var chosen string
	for _, clause := range cnf {
		for _, lit := range clause {
			if _, ok := assignment[lit.Var]; !ok {
				chosen = lit.Var
				break
			}
		}
		if chosen != "" {
			break
		}
	}
	if chosen == "" {
		return true // All assigned, no empty clauses.
	}

	// Branch: try true, then false.
	for _, val := range []bool{true, false} {
		newAssign := copyAssignment(assignment)
		newAssign[chosen] = val
		newCNF := simplify(cnf, chosen, val)
		if dpll(newCNF, newAssign, vars) {
			return true
		}
	}
	return false
}

// unitPropagate applies unit clauses (clauses with exactly one literal).
func unitPropagate(cnf CNF, assignment map[string]bool) (CNF, map[string]bool) {
	changed := true
	for changed {
		changed = false
		for _, clause := range cnf {
			if len(clause) == 1 {
				lit := clause[0]
				expected := !lit.Negate
				if val, ok := assignment[lit.Var]; ok {
					if val != expected {
						// Conflict — return CNF with empty clause.
						return CNF{{}}, assignment
					}
					continue
				}
				assignment[lit.Var] = expected
				cnf = simplify(cnf, lit.Var, expected)
				changed = true
				break
			}
		}
	}
	return cnf, assignment
}

// pureLiteral eliminates variables that appear in only one polarity.
func pureLiteral(cnf CNF, assignment map[string]bool) (CNF, map[string]bool) {
	pos := make(map[string]bool)
	neg := make(map[string]bool)
	for _, clause := range cnf {
		for _, lit := range clause {
			if _, ok := assignment[lit.Var]; ok {
				continue
			}
			if lit.Negate {
				neg[lit.Var] = true
			} else {
				pos[lit.Var] = true
			}
		}
	}

	for v := range pos {
		if !neg[v] {
			assignment[v] = true
			cnf = simplify(cnf, v, true)
		}
	}
	for v := range neg {
		if !pos[v] {
			assignment[v] = false
			cnf = simplify(cnf, v, false)
		}
	}

	return cnf, assignment
}

// simplify removes clauses satisfied by the assignment and removes false
// literals from remaining clauses.
func simplify(cnf CNF, variable string, value bool) CNF {
	var result CNF
	for _, clause := range cnf {
		satisfied := false
		var remaining Clause
		for _, lit := range clause {
			if lit.Var == variable {
				if lit.Negate != value { // literal is true
					satisfied = true
					break
				}
				// literal is false — skip it
				continue
			}
			remaining = append(remaining, lit)
		}
		if !satisfied {
			result = append(result, remaining)
		}
	}
	return result
}

// copyAssignment creates a shallow copy of the assignment map.
func copyAssignment(m map[string]bool) map[string]bool {
	c := make(map[string]bool, len(m))
	for k, v := range m {
		c[k] = v
	}
	return c
}
