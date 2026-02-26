package consistency

// Tier 4: Heuristic contradiction detection.
// Detects: polarity inversion, quantifier conflict, numeric bound conflict.

import (
	"database/sql"
	"fmt"
	"regexp"
	"strconv"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

var (
	numericBoundRe = regexp.MustCompile(`(?i)(at\s+most|at\s+least|no\s+more\s+than|no\s+fewer\s+than|exactly|maximum|minimum|≤|≥|<=|>=)\s*(\d+)`)
	quantifierRe   = regexp.MustCompile(`(?i)\b(for\s+all|every|each|all\s+\w+|no\s+\w+\s+may|exists?\s+\w+|there\s+exists?|some\s+\w+|at\s+least\s+one)\b`)
)

// analyzeHeuristic runs Tier 4 heuristic rules.
func analyzeHeuristic(db *sql.DB, specID int64) ([]Contradiction, int, error) {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, 0, fmt.Errorf("list invariants: %w", err)
	}

	var results []Contradiction

	// 1. Polarity inversion between invariant pairs.
	polarityResults := detectPolarityInversion(invs)
	results = append(results, polarityResults...)

	// 2. Quantifier conflicts.
	quantResults := detectQuantifierConflict(invs)
	results = append(results, quantResults...)

	// 3. Numeric bound conflicts.
	numResults := detectNumericBoundConflict(invs)
	results = append(results, numResults...)

	return results, len(invs), nil
}

// detectPolarityInversion finds invariant pairs where one says "must X" and
// the other says "must not X" for the same X (subject overlap required).
func detectPolarityInversion(invs []storage.Invariant) []Contradiction {
	type polarized struct {
		invID    string
		positive string // the action after "must" / "always" / etc.
		negative string // the action after "must not" / "never" / etc.
	}

	var polarizedInvs []polarized
	for _, inv := range invs {
		p := polarized{invID: inv.InvariantID}
		lower := strings.ToLower(inv.Statement)

		// Extract positive and negative directives.
		for _, prefix := range []string{"must not ", "shall not ", "never "} {
			if idx := strings.Index(lower, prefix); idx >= 0 {
				action := extractAction(inv.Statement, idx+len(prefix))
				if action != "" {
					p.negative = action
				}
			}
		}
		for _, prefix := range []string{"must ", "shall ", "always "} {
			if idx := strings.Index(lower, prefix); idx >= 0 {
				// Make sure it's not "must not".
				if idx+len(prefix) < len(lower) && !strings.HasPrefix(lower[idx+len(prefix):], "not ") {
					action := extractAction(inv.Statement, idx+len(prefix))
					if action != "" {
						p.positive = action
					}
				}
			}
		}

		if p.positive != "" || p.negative != "" {
			polarizedInvs = append(polarizedInvs, p)
		}
	}

	var results []Contradiction
	for i := 0; i < len(polarizedInvs); i++ {
		for j := i + 1; j < len(polarizedInvs); j++ {
			a, b := polarizedInvs[i], polarizedInvs[j]

			// Check: A positive vs B negative (or vice versa).
			if a.positive != "" && b.negative != "" && actionOverlap(a.positive, b.negative) > 0.5 {
				results = append(results, Contradiction{
					Tier:       TierHeuristic,
					Type:       PolarityInversion,
					ElementA:   a.invID,
					ElementB:   b.invID,
					Description: fmt.Sprintf(
						"%s mandates %q while %s forbids %q.",
						a.invID, truncate(a.positive, 60),
						b.invID, truncate(b.negative, 60),
					),
					Evidence:       fmt.Sprintf("Positive: %q. Negative: %q.", a.positive, b.negative),
					Confidence:     0.6,
					ResolutionHint: "Verify these address the same subject. If reconciled by scope or context, this is a false positive.",
				})
			}
			if b.positive != "" && a.negative != "" && actionOverlap(b.positive, a.negative) > 0.5 {
				results = append(results, Contradiction{
					Tier:       TierHeuristic,
					Type:       PolarityInversion,
					ElementA:   b.invID,
					ElementB:   a.invID,
					Description: fmt.Sprintf(
						"%s mandates %q while %s forbids %q.",
						b.invID, truncate(b.positive, 60),
						a.invID, truncate(a.negative, 60),
					),
					Evidence:       fmt.Sprintf("Positive: %q. Negative: %q.", b.positive, a.negative),
					Confidence:     0.6,
					ResolutionHint: "Verify these address the same subject. If reconciled by scope or context, this is a false positive.",
				})
			}
		}
	}

	return results
}

// detectQuantifierConflict finds pairs where one uses universal quantifier
// ("for all") and the other uses existential negation ("no X may") on
// overlapping subjects.
func detectQuantifierConflict(invs []storage.Invariant) []Contradiction {
	type quantified struct {
		invID     string
		universal bool   // "for all", "every", "each"
		existNeg  bool   // "no X may", "never"
		subject   string // extracted subject
	}

	var qInvs []quantified
	for _, inv := range invs {
		lower := strings.ToLower(inv.SemiFormal + " " + inv.Statement)
		q := quantified{invID: inv.InvariantID}

		ms := quantifierRe.FindAllStringSubmatch(lower, -1)
		for _, m := range ms {
			qWord := strings.ToLower(m[1])
			switch {
			case strings.Contains(qWord, "for all") || strings.Contains(qWord, "every") || strings.Contains(qWord, "each") || strings.Contains(qWord, "all "):
				q.universal = true
				q.subject = extractSubjectAfter(lower, m[0])
			case strings.Contains(qWord, "no ") || strings.Contains(qWord, "never"):
				q.existNeg = true
				q.subject = extractSubjectAfter(lower, m[0])
			}
		}

		if (q.universal || q.existNeg) && q.subject != "" {
			qInvs = append(qInvs, q)
		}
	}

	var results []Contradiction
	for i := 0; i < len(qInvs); i++ {
		for j := i + 1; j < len(qInvs); j++ {
			a, b := qInvs[i], qInvs[j]
			if a.universal && b.existNeg && subjectOverlap(a.subject, b.subject) > 0.4 {
				results = append(results, Contradiction{
					Tier:       TierHeuristic,
					Type:       QuantifierConflict,
					ElementA:   a.invID,
					ElementB:   b.invID,
					Description: fmt.Sprintf(
						"%s uses universal quantifier over %q while %s uses existential negation over %q.",
						a.invID, truncate(a.subject, 40),
						b.invID, truncate(b.subject, 40),
					),
					Evidence:       fmt.Sprintf("Universal subject: %q. Negation subject: %q.", a.subject, b.subject),
					Confidence:     0.5,
					ResolutionHint: "If the quantifiers range over different domains, this is not a contradiction.",
				})
			}
			if b.universal && a.existNeg && subjectOverlap(b.subject, a.subject) > 0.4 {
				results = append(results, Contradiction{
					Tier:       TierHeuristic,
					Type:       QuantifierConflict,
					ElementA:   b.invID,
					ElementB:   a.invID,
					Description: fmt.Sprintf(
						"%s uses universal quantifier over %q while %s uses existential negation over %q.",
						b.invID, truncate(b.subject, 40),
						a.invID, truncate(a.subject, 40),
					),
					Evidence:       fmt.Sprintf("Universal subject: %q. Negation subject: %q.", b.subject, a.subject),
					Confidence:     0.5,
					ResolutionHint: "If the quantifiers range over different domains, this is not a contradiction.",
				})
			}
		}
	}

	return results
}

// detectNumericBoundConflict finds invariant pairs with incompatible numeric constraints.
func detectNumericBoundConflict(invs []storage.Invariant) []Contradiction {
	type bound struct {
		kind  string // "at_most", "at_least", "exactly"
		value int
		ctx   string // surrounding text for subject matching
	}
	type numBound struct {
		invID  string
		bounds []bound
	}

	var numInvs []numBound
	for _, inv := range invs {
		combined := inv.Statement + " " + inv.SemiFormal
		ms := numericBoundRe.FindAllStringSubmatchIndex(combined, -1)
		if len(ms) == 0 {
			continue
		}

		nb := numBound{invID: inv.InvariantID}
		for _, loc := range ms {
			kindStr := strings.ToLower(combined[loc[2]:loc[3]])
			valStr := combined[loc[4]:loc[5]]
			val, err := strconv.Atoi(valStr)
			if err != nil {
				continue
			}

			var kind string
			switch {
			case strings.Contains(kindStr, "at most") || strings.Contains(kindStr, "no more") || strings.Contains(kindStr, "maximum") || kindStr == "≤" || kindStr == "<=":
				kind = "at_most"
			case strings.Contains(kindStr, "at least") || strings.Contains(kindStr, "no fewer") || strings.Contains(kindStr, "minimum") || kindStr == "≥" || kindStr == ">=":
				kind = "at_least"
			case strings.Contains(kindStr, "exactly"):
				kind = "exactly"
			default:
				continue
			}

			// Extract surrounding context (±30 chars) for subject matching.
			start := loc[0] - 30
			if start < 0 {
				start = 0
			}
			end := loc[1] + 30
			if end > len(combined) {
				end = len(combined)
			}
			ctx := combined[start:end]

			nb.bounds = append(nb.bounds, bound{kind: kind, value: val, ctx: ctx})
		}
		if len(nb.bounds) > 0 {
			numInvs = append(numInvs, nb)
		}
	}

	var results []Contradiction
	for i := 0; i < len(numInvs); i++ {
		for j := i + 1; j < len(numInvs); j++ {
			a, b := numInvs[i], numInvs[j]
			for _, ba := range a.bounds {
				for _, bb := range b.bounds {
					// Only compare if subjects overlap.
					if subjectOverlap(ba.ctx, bb.ctx) < 0.3 {
						continue
					}

					conflict := false
					var desc string
					switch {
					case ba.kind == "at_most" && bb.kind == "at_least" && ba.value < bb.value:
						conflict = true
						desc = fmt.Sprintf("at most %d vs at least %d", ba.value, bb.value)
					case ba.kind == "at_least" && bb.kind == "at_most" && ba.value > bb.value:
						conflict = true
						desc = fmt.Sprintf("at least %d vs at most %d", ba.value, bb.value)
					case ba.kind == "exactly" && bb.kind == "exactly" && ba.value != bb.value:
						conflict = true
						desc = fmt.Sprintf("exactly %d vs exactly %d", ba.value, bb.value)
					case ba.kind == "exactly" && bb.kind == "at_most" && ba.value > bb.value:
						conflict = true
						desc = fmt.Sprintf("exactly %d vs at most %d", ba.value, bb.value)
					case ba.kind == "exactly" && bb.kind == "at_least" && ba.value < bb.value:
						conflict = true
						desc = fmt.Sprintf("exactly %d vs at least %d", ba.value, bb.value)
					}

					if conflict {
						results = append(results, Contradiction{
							Tier:       TierHeuristic,
							Type:       NumericBoundConflict,
							ElementA:   a.invID,
							ElementB:   b.invID,
							Description: fmt.Sprintf(
								"%s and %s have incompatible numeric bounds: %s.",
								a.invID, b.invID, desc,
							),
							Evidence:       fmt.Sprintf("Bound A context: %q. Bound B context: %q.", truncate(ba.ctx, 80), truncate(bb.ctx, 80)),
							Confidence:     0.7,
							ResolutionHint: "Verify these bounds apply to the same quantity. Different scopes may resolve the conflict.",
						})
					}
				}
			}
		}
	}

	return results
}

// extractAction pulls the first clause after a directive keyword.
func extractAction(text string, offset int) string {
	if offset >= len(text) {
		return ""
	}
	action := text[offset:]
	// Take up to first major punctuation.
	for _, sep := range []string{". ", ", ", "; ", " — ", " - "} {
		if idx := strings.Index(action, sep); idx > 0 && idx < 80 {
			action = action[:idx]
		}
	}
	if len(action) > 80 {
		action = action[:80]
	}
	return strings.TrimSpace(action)
}

// actionOverlap computes word overlap ratio between two action strings.
func actionOverlap(a, b string) float64 {
	wordsA := significantWords(a)
	wordsB := significantWords(b)
	if len(wordsA) == 0 || len(wordsB) == 0 {
		return 0
	}

	setB := make(map[string]bool)
	for _, w := range wordsB {
		setB[w] = true
	}

	matches := 0
	for _, w := range wordsA {
		if setB[w] {
			matches++
		}
	}

	// Normalize by the smaller set.
	minLen := len(wordsA)
	if len(wordsB) < minLen {
		minLen = len(wordsB)
	}
	return float64(matches) / float64(minLen)
}

// subjectOverlap computes the word overlap ratio between subject strings.
func subjectOverlap(a, b string) float64 {
	return actionOverlap(a, b)
}

// extractSubjectAfter pulls a few words after a quantifier match.
func extractSubjectAfter(text, match string) string {
	idx := strings.Index(text, match)
	if idx < 0 {
		return ""
	}
	after := text[idx+len(match):]
	words := strings.Fields(after)
	if len(words) > 5 {
		words = words[:5]
	}
	return strings.Join(words, " ")
}
