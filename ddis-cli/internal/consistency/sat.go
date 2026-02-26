package consistency

// Tier 3: SAT-based contradiction detection via gophersat.
//
// ddis:maintains APP-ADR-034 (superseded — gophersat retained for fast propositional path)
// ddis:maintains APP-INV-021 (SAT encoding fidelity — Tier 3 propositional)
//
// Parses semi-formal expressions into propositional clauses (CNF) using a
// GLOBAL variable namespace (APP-INV-021), then solves via gophersat CDCL.
// UNSAT = contradiction. Unparseable expressions degrade to Tier 4.

import (
	"database/sql"
	"fmt"
	"regexp"
	"strings"

	"github.com/crillab/gophersat/solver"
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
	forAllBodyRe = regexp.MustCompile(`(?i)FOR\s+ALL\s+\w+(?:\s+IN\s+\w+)?\s*[,:]\s*(.+)`)
	existsBodyRe = regexp.MustCompile(`(?i)(?:THERE\s+)?EXISTS?\s+\w+(?:\s+IN\s+\w+)?\s*[,:]\s*(.+)`)
	impliesRe    = regexp.MustCompile(`(?i)(.+?)\s+IMPLIES\s+(.+)`)
	andRe        = regexp.MustCompile(`(?i)\s+AND\s+`)
	orRe         = regexp.MustCompile(`(?i)\s+OR\s+`)
	notRe        = regexp.MustCompile(`(?i)^NOT\s+(.+)$`)
	predicateRe  = regexp.MustCompile(`(?i)(\w+)\(([^)]*)\)\s*=\s*(\w+)`)
	dotEqualsRe  = regexp.MustCompile(`(?i)(\w+(?:\.\w+)+)\s*=\s*(\w+)`)
	funcCallRe   = regexp.MustCompile(`(?i)(\w+)\(([^)]*)\)`)
	arrowRe      = regexp.MustCompile(`(.+?)\s*(?:→|=>)\s*(.+)`)
)

// analyzeSAT runs Tier 3 SAT-based analysis on semi-formal expressions.
func analyzeSAT(db *sql.DB, specID int64) ([]Contradiction, int, error) {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, 0, fmt.Errorf("list invariants: %w", err)
	}

	// Global variable namespace shared across ALL invariants.
	vm := NewVarMap()

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
		cnf := ParseSemiFormal(inv.SemiFormal, vm)
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

	// Check pairwise satisfiability via gophersat.
	var results []Contradiction
	for i := 0; i < len(parsedInvs); i++ {
		for j := i + 1; j < len(parsedInvs); j++ {
			a, b := parsedInvs[i], parsedInvs[j]

			combined := make(CNF, 0, len(a.clauses)+len(b.clauses))
			combined = append(combined, a.clauses...)
			combined = append(combined, b.clauses...)

			if !Satisfiable(combined, vm) {
				results = append(results, Contradiction{
					Tier:     TierSAT,
					Type:     SATUnsatisfiable,
					ElementA: a.invID,
					ElementB: b.invID,
					Description: fmt.Sprintf(
						"%s and %s have unsatisfiable combined constraints (UNSAT via gophersat).",
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

// ParseSemiFormal converts a semi-formal expression to CNF using a global
// variable namespace. Returns empty CNF if the expression cannot be parsed.
func ParseSemiFormal(expr string, vm *VarMap) CNF {
	expr = strings.TrimSpace(expr)
	expr = strings.TrimPrefix(expr, "```")
	expr = strings.TrimSuffix(expr, "```")
	expr = strings.TrimSpace(expr)

	var cnf CNF

	lines := strings.Split(expr, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		// Skip definition/context lines
		lower := strings.ToLower(line)
		if strings.HasPrefix(lower, "where") || strings.HasPrefix(line, "//") ||
			strings.HasPrefix(lower, "let ") || strings.HasPrefix(lower, "encoding") ||
			strings.HasPrefix(lower, "numeric") || strings.HasPrefix(lower, "named ") {
			continue
		}

		clausesFromLine := parseLine(line, vm)
		cnf = append(cnf, clausesFromLine...)
	}

	return cnf
}

// parseLine converts a single line of semi-formal text to zero or more clauses.
func parseLine(line string, vm *VarMap) CNF {
	line = strings.TrimSpace(line)
	if line == "" {
		return nil
	}

	// 1. FOR ALL x IN S: body → parse body as conjunction
	if m := forAllBodyRe.FindStringSubmatch(line); m != nil {
		return parseLine(m[1], vm)
	}

	// 2. EXISTS x IN S: body → parse body (treated as assertion)
	if m := existsBodyRe.FindStringSubmatch(line); m != nil {
		return parseLine(m[1], vm)
	}

	// 3. A IMPLIES B → (¬A ∨ B)
	if m := impliesRe.FindStringSubmatch(line); m != nil {
		antLits := parseAtom(strings.TrimSpace(m[1]), vm)
		consLits := parseAtom(strings.TrimSpace(m[2]), vm)
		if len(antLits) > 0 && len(consLits) > 0 {
			// ¬antecedent ∨ consequent
			var clause Clause
			for _, lit := range antLits {
				clause = append(clause, Literal{Var: lit.Var, Negate: !lit.Negate})
			}
			clause = append(clause, consLits...)
			return CNF{clause}
		}
	}

	// 3b. Arrow notation: A → B or A => B
	if m := arrowRe.FindStringSubmatch(line); m != nil {
		antLits := parseAtom(strings.TrimSpace(m[1]), vm)
		consLits := parseAtom(strings.TrimSpace(m[2]), vm)
		if len(antLits) > 0 && len(consLits) > 0 {
			var clause Clause
			for _, lit := range antLits {
				clause = append(clause, Literal{Var: lit.Var, Negate: !lit.Negate})
			}
			clause = append(clause, consLits...)
			return CNF{clause}
		}
	}

	// 4. A AND B AND C → separate unit clauses
	if andRe.MatchString(line) {
		parts := andRe.Split(line, -1)
		var cnf CNF
		for _, part := range parts {
			part = strings.TrimSpace(part)
			if part == "" {
				continue
			}
			sub := parseLine(part, vm)
			if len(sub) > 0 {
				cnf = append(cnf, sub...)
			} else {
				lits := parseAtom(part, vm)
				for _, lit := range lits {
					cnf = append(cnf, Clause{lit})
				}
			}
		}
		if len(cnf) > 0 {
			return cnf
		}
	}

	// 5. A OR B → single disjunctive clause
	if orRe.MatchString(line) {
		parts := orRe.Split(line, -1)
		var clause Clause
		for _, part := range parts {
			part = strings.TrimSpace(part)
			if part == "" {
				continue
			}
			lits := parseAtom(part, vm)
			clause = append(clause, lits...)
		}
		if len(clause) > 0 {
			return CNF{clause}
		}
	}

	// 6. Single atom
	lits := parseAtom(line, vm)
	if len(lits) > 0 {
		var cnf CNF
		for _, lit := range lits {
			cnf = append(cnf, Clause{lit})
		}
		return cnf
	}

	return nil
}

// parseAtom converts an atomic expression to literals using the global VarMap.
func parseAtom(text string, vm *VarMap) []Literal {
	text = strings.TrimSpace(text)
	if text == "" {
		return nil
	}

	// NOT X
	if m := notRe.FindStringSubmatch(text); m != nil {
		inner := parseAtom(strings.TrimSpace(m[1]), vm)
		for i := range inner {
			inner[i].Negate = !inner[i].Negate
		}
		return inner
	}

	// P(x) = value (predicate with truth value)
	if m := predicateRe.FindStringSubmatch(text); m != nil {
		pred := strings.TrimSpace(m[1])
		args := strings.TrimSpace(m[2])
		val := strings.ToLower(strings.TrimSpace(m[3]))
		varName := MakePredicateVar(pred, args)
		negate := val == "false"
		return []Literal{{Var: varName, Negate: negate}}
	}

	// x.property = value (dot-path equality)
	if m := dotEqualsRe.FindStringSubmatch(text); m != nil {
		path := strings.TrimSpace(m[1])
		value := strings.TrimSpace(m[2])
		varName := MakeDotVar(path, value)
		return []Literal{{Var: varName, Negate: false}}
	}

	// f(args) — function call without = (treat as positive assertion)
	if m := funcCallRe.FindStringSubmatch(text); m != nil {
		pred := strings.TrimSpace(m[1])
		args := strings.TrimSpace(m[2])
		// Skip quantifier keywords
		lowerPred := strings.ToLower(pred)
		if lowerPred == "for" || lowerPred == "exists" || lowerPred == "there" {
			return nil
		}
		varName := MakePredicateVar(pred, args)
		return []Literal{{Var: varName, Negate: false}}
	}

	// Simple equality: X = Y
	eqParts := strings.SplitN(text, "=", 2)
	if len(eqParts) == 2 {
		lhs := strings.TrimSpace(eqParts[0])
		rhs := strings.TrimSpace(eqParts[1])
		if lhs != "" && rhs != "" && !strings.Contains(lhs, " ") {
			varName := sanitize(lhs) + "_eq_" + sanitize(rhs)
			return []Literal{{Var: varName, Negate: false}}
		}
	}

	// Fallback: treat the whole thing as a propositional variable
	name := sanitize(text)
	if name != "" && len(name) > 2 {
		return []Literal{{Var: name, Negate: false}}
	}

	return nil
}

// Satisfiable checks if a CNF formula is satisfiable using gophersat CDCL.
func Satisfiable(cnf CNF, vm *VarMap) bool {
	if len(cnf) == 0 {
		return true
	}

	// Convert from named Literal/Clause/CNF to gophersat's int-based format.
	intClauses := make([][]int, 0, len(cnf))
	for _, clause := range cnf {
		if len(clause) == 0 {
			return false // empty clause = immediate UNSAT
		}
		intClause := make([]int, 0, len(clause))
		for _, lit := range clause {
			varID := vm.Get(lit.Var)
			if lit.Negate {
				intClause = append(intClause, -varID)
			} else {
				intClause = append(intClause, varID)
			}
		}
		intClauses = append(intClauses, intClause)
	}

	pb := solver.ParseSlice(intClauses)
	s := solver.New(pb)
	status := s.Solve()
	return status == solver.Sat
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
	for strings.Contains(s, "__") {
		s = strings.ReplaceAll(s, "__", "_")
	}
	s = strings.Trim(s, "_")
	if len(s) > 60 {
		s = s[:60]
	}
	return s
}
