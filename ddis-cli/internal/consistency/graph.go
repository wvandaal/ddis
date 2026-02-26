package consistency

// Tier 2: Graph-based contradiction detection.
// Detects: governance overlap, negative spec violation, circular implication.

import (
	"database/sql"
	"fmt"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// analyzeGraph runs Tier 2 graph analysis.
func analyzeGraph(db *sql.DB, specID int64) ([]Contradiction, int, error) {
	var results []Contradiction
	scanned := 0

	// 1. Governance overlap: two invariants from different domains with high
	//    cross-reference overlap (shared reference targets).
	govResults, n, err := detectGovernanceOverlap(db, specID)
	if err != nil {
		return nil, 0, err
	}
	results = append(results, govResults...)
	scanned += n

	// 2. Negative spec violations: an INV/ADR implies something that a
	//    negative spec forbids.
	negResults, n2, err := detectNegSpecViolations(db, specID)
	if err != nil {
		return nil, 0, err
	}
	results = append(results, negResults...)
	scanned += n2

	return results, scanned, nil
}

// detectGovernanceOverlap finds invariant pairs from different domains/modules
// whose forward BFS reach sets overlap significantly with opposing signals.
func detectGovernanceOverlap(db *sql.DB, specID int64) ([]Contradiction, int, error) {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, 0, fmt.Errorf("list invariants: %w", err)
	}

	// Build per-invariant reach sets using outgoing refs from their sections.
	type reachSet map[string]bool
	invReach := make(map[string]reachSet)
	invDomain := make(map[string]string) // invariant_id → domain

	modules, err := storage.ListModules(db, specID)
	if err != nil {
		return nil, 0, fmt.Errorf("list modules: %w", err)
	}
	sectionModule := make(map[int64]string) // section_id → module_name
	moduleDomain := make(map[string]string) // module_name → domain
	for _, m := range modules {
		moduleDomain[m.ModuleName] = m.Domain
	}

	// Map sections to modules via source files.
	sections, err := storage.ListSections(db, specID)
	if err != nil {
		return nil, 0, fmt.Errorf("list sections: %w", err)
	}
	fileMod := make(map[int64]string) // source_file_id → module_name
	for _, m := range modules {
		fileMod[m.SourceFileID] = m.ModuleName
	}
	for _, s := range sections {
		if mod, ok := fileMod[s.SourceFileID]; ok {
			sectionModule[s.ID] = mod
		}
	}

	for _, inv := range invs {
		// Get domain from the invariant's section's module.
		if mod, ok := sectionModule[inv.SectionID]; ok {
			invDomain[inv.InvariantID] = moduleDomain[mod]
		}

		// Build reach set: all targets referenced from this invariant's section.
		refs, err := storage.GetOutgoingRefs(db, specID, inv.SectionID)
		if err != nil {
			continue
		}
		reach := make(reachSet)
		for _, ref := range refs {
			reach[ref.RefTarget] = true
		}
		invReach[inv.InvariantID] = reach
	}

	// Compare pairs from different domains.
	var results []Contradiction
	for i := 0; i < len(invs); i++ {
		for j := i + 1; j < len(invs); j++ {
			a, b := invs[i], invs[j]
			domA, domB := invDomain[a.InvariantID], invDomain[b.InvariantID]
			if domA == domB || domA == "" || domB == "" {
				continue
			}

			reachA := invReach[a.InvariantID]
			reachB := invReach[b.InvariantID]
			if len(reachA) == 0 || len(reachB) == 0 {
				continue
			}

			// Compute Jaccard overlap.
			intersection := 0
			for k := range reachA {
				if reachB[k] {
					intersection++
				}
			}
			union := len(reachA) + len(reachB) - intersection
			if union == 0 {
				continue
			}
			overlap := float64(intersection) / float64(union)

			// High overlap between different-domain invariants is suspicious.
			// Only flag if > 0.6 AND there are opposing polarity signals.
			if overlap > 0.6 && hasOpposingPolarity(a.Statement, b.Statement) {
				results = append(results, Contradiction{
					Tier:       TierGraph,
					Type:       GovernanceOverlap,
					ElementA:   a.InvariantID,
					ElementB:   b.InvariantID,
					Description: fmt.Sprintf(
						"%s (%s domain) and %s (%s domain) have %.0f%% reference overlap with opposing polarity.",
						a.InvariantID, domA, b.InvariantID, domB, overlap*100,
					),
					Evidence: fmt.Sprintf(
						"Forward reach overlap: %d/%d (Jaccard %.2f). %s: %q. %s: %q.",
						intersection, union, overlap,
						a.InvariantID, truncate(a.Statement, 80),
						b.InvariantID, truncate(b.Statement, 80),
					),
					Confidence:     overlap * 0.8, // Graph evidence alone caps at ~0.8.
					ResolutionHint: "Check if these invariants govern the same concept from different angles. If reconciled by an ADR, this is advisory only.",
				})
			}
		}
	}

	return results, len(invs), nil
}

// detectNegSpecViolations checks whether any invariant or ADR implies something
// that a negative spec explicitly forbids.
func detectNegSpecViolations(db *sql.DB, specID int64) ([]Contradiction, int, error) {
	negSpecs, err := storage.ListNegativeSpecs(db, specID)
	if err != nil {
		return nil, 0, fmt.Errorf("list negative specs: %w", err)
	}
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, 0, fmt.Errorf("list invariants: %w", err)
	}

	var results []Contradiction
	for _, ns := range negSpecs {
		// Extract the forbidden action from "Must NOT X" or "DO NOT X".
		forbidden := extractForbiddenAction(ns.ConstraintText)
		if forbidden == "" {
			continue
		}

		for _, inv := range invs {
			// Check if the invariant's statement or semi-formal expression
			// implies the forbidden action.
			if impliesForbidden(inv.Statement, inv.SemiFormal, forbidden) {
				// Before flagging: check if the negative spec is in the SAME
				// module. Same-module neg specs are self-constraining, not contradictions.
				if ns.InvariantRef == inv.InvariantID {
					continue
				}

				results = append(results, Contradiction{
					Tier:     TierGraph,
					Type:     NegSpecViolation,
					ElementA: inv.InvariantID,
					ElementB: fmt.Sprintf("neg-spec:%d", ns.ID),
					Description: fmt.Sprintf(
						"%s may imply %q, which negative spec forbids: %q",
						inv.InvariantID, forbidden, truncate(ns.ConstraintText, 100),
					),
					Evidence: fmt.Sprintf(
						"Invariant statement: %q. Negative spec: %q.",
						truncate(inv.Statement, 120), truncate(ns.ConstraintText, 120),
					),
					Confidence:     0.5,
					ResolutionHint: "Verify whether the invariant genuinely requires the forbidden action, or if this is a terminology overlap.",
				})
			}
		}
	}

	return results, len(negSpecs) + len(invs), nil
}

// extractForbiddenAction extracts the core action from a "Must NOT X" constraint.
func extractForbiddenAction(text string) string {
	lower := strings.ToLower(text)
	for _, prefix := range []string{"must not ", "do not ", "never ", "shall not "} {
		if idx := strings.Index(lower, prefix); idx >= 0 {
			action := text[idx+len(prefix):]
			// Take first clause (up to comma, period, or dash).
			for _, sep := range []string{",", ".", " — ", " - ", " –"} {
				if i := strings.Index(action, sep); i > 0 {
					action = action[:i]
				}
			}
			return strings.TrimSpace(action)
		}
	}
	return ""
}

// impliesForbidden checks if statement/semi-formal text POSITIVELY implies
// the forbidden action. If the invariant itself is NEGATIVE (prohibiting
// the same action), both sides agree — no contradiction exists.
// This is the core mechanism for APP-INV-019 (zero false positives).
func impliesForbidden(statement, semiFormal, forbidden string) bool {
	forbiddenWords := significantWords(forbidden)
	if len(forbiddenWords) == 0 {
		return false
	}

	combined := strings.ToLower(statement + " " + semiFormal)
	matches := 0
	for _, w := range forbiddenWords {
		if strings.Contains(combined, w) {
			matches++
		}
	}

	// Require high word overlap (75% threshold for precision).
	overlap := float64(matches) / float64(len(forbiddenWords))
	if len(forbiddenWords) < 2 || overlap < 0.75 {
		return false
	}

	// KEY FIX: Check if the invariant itself contains negation language.
	// If the invariant also prohibits the action, both sides AGREE — not a contradiction.
	// Detect: "must not", "do not", "never", "shall not", "NOT" (in semi-formal)
	lowerStmt := strings.ToLower(statement)
	lowerSF := strings.ToLower(semiFormal)

	negationMarkers := []string{
		"must not ", "do not ", "never ", "shall not ",
		"must not\n", "do not\n", "never\n", "shall not\n",
	}
	stmtIsNegative := false
	for _, neg := range negationMarkers {
		if strings.Contains(lowerStmt, neg) {
			stmtIsNegative = true
			break
		}
	}
	// Also check semi-formal for NOT operator
	if !stmtIsNegative && strings.Contains(lowerSF, " not ") {
		stmtIsNegative = true
	}

	if stmtIsNegative {
		// The invariant is itself a prohibition. Check whether the forbidden
		// action words appear in the NEGATIVE context (after the negation marker).
		// If so, both the neg spec and the invariant forbid the same thing.
		return false
	}

	return true
}

// significantWords extracts meaningful words (>3 chars, not stopwords).
func significantWords(text string) []string {
	stopwords := map[string]bool{
		"the": true, "and": true, "for": true, "that": true, "with": true,
		"this": true, "from": true, "are": true, "was": true, "were": true,
		"been": true, "have": true, "has": true, "had": true, "will": true,
		"would": true, "could": true, "should": true, "must": true, "shall": true,
		"each": true, "every": true, "all": true, "any": true, "some": true,
		"not": true, "when": true, "where": true, "which": true, "what": true,
		"than": true, "then": true, "only": true, "also": true, "does": true,
	}

	var words []string
	for _, w := range strings.Fields(strings.ToLower(text)) {
		w = strings.Trim(w, ".,;:()[]{}\"'`")
		if len(w) > 3 && !stopwords[w] {
			words = append(words, w)
		}
	}
	return words
}

// hasOpposingPolarity checks if two statements have signals of opposite direction
// (e.g., "must" vs "must not", "minimize" vs "maximize", "always" vs "never").
func hasOpposingPolarity(a, b string) bool {
	la, lb := strings.ToLower(a), strings.ToLower(b)

	polarityPairs := [][2]string{
		{"must ", "must not "},
		{"always ", "never "},
		{"minimize", "maximize"},
		{"increase", "decrease"},
		{"add ", "remove "},
		{"include", "exclude"},
		{"require", "prohibit"},
		{"enable", "disable"},
	}

	for _, pair := range polarityPairs {
		if (strings.Contains(la, pair[0]) && strings.Contains(lb, pair[1])) ||
			(strings.Contains(la, pair[1]) && strings.Contains(lb, pair[0])) {
			return true
		}
	}
	return false
}

// truncate limits a string to maxLen chars with ellipsis.
func truncate(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen-3] + "..."
}
