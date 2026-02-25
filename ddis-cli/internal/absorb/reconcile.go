package absorb

import (
	"database/sql"
	"fmt"
	"strings"

	"github.com/wvandaal/ddis/internal/autoprompt"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:implements APP-ADR-024 (bilateral specification)
// ddis:maintains APP-INV-032 (symmetric reconciliation)

// matchThreshold is the minimum keyword overlap score for a correspondence.
const matchThreshold = 0.6

// reverseThreshold is the minimum score for considering a spec element implemented.
const reverseThreshold = 0.4

// Reconcile performs bidirectional gap analysis between code patterns and spec.
// Reports THREE categories (APP-INV-032):
//  1. Correspondences: code pattern matches spec element
//  2. Undocumented behavior: code pattern has no spec match (score < matchThreshold)
//  3. Unimplemented spec: spec element has no code evidence (score < reverseThreshold)
func Reconcile(result *AbsorbResult, db *sql.DB, specID int64) error {
	report := &ReconciliationReport{}

	// Load spec elements.
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return fmt.Errorf("list invariants: %w", err)
	}
	adrs, err := storage.ListADRs(db, specID)
	if err != nil {
		return fmt.Errorf("list adrs: %w", err)
	}
	gates, err := storage.ListQualityGates(db, specID)
	if err != nil {
		return fmt.Errorf("list gates: %w", err)
	}

	// Build searchable spec element list.
	type specElement struct {
		id      string
		typ     string
		title   string
		text    string
		words   map[string]bool
	}

	var elements []specElement
	for _, inv := range invs {
		combined := inv.InvariantID + " " + inv.Title + " " + inv.Statement
		elements = append(elements, specElement{
			id:    inv.InvariantID,
			typ:   "invariant",
			title: inv.Title,
			text:  combined,
			words: wordSet(combined),
		})
	}
	for _, adr := range adrs {
		combined := adr.ADRID + " " + adr.Title + " " + adr.Problem
		elements = append(elements, specElement{
			id:    adr.ADRID,
			typ:   "adr",
			title: adr.Title,
			text:  combined,
			words: wordSet(combined),
		})
	}
	for _, g := range gates {
		combined := g.GateID + " " + g.Title + " " + g.Predicate
		elements = append(elements, specElement{
			id:    g.GateID,
			typ:   "gate",
			title: g.Title,
			text:  combined,
			words: wordSet(combined),
		})
	}

	// Track which spec elements have at least one pattern correspondence.
	matched := make(map[string]float64) // element ID -> best score

	// Forward pass: for each code pattern, find best matching spec element.
	for _, p := range result.Patterns {
		bestScore := 0.0
		bestIdx := -1

		// Annotation patterns: direct match by target ID.
		if p.Type == "annotation" {
			target := extractAnnotationTarget(p.Text)
			for i, elem := range elements {
				if elem.id == target {
					bestScore = 1.0
					bestIdx = i
					break
				}
			}
		}

		// If no direct match (or not an annotation), try keyword overlap.
		if bestIdx < 0 {
			pWords := wordSet(p.Text)
			for i, elem := range elements {
				score := keywordOverlap(pWords, elem.words)
				if score > bestScore {
					bestScore = score
					bestIdx = i
				}
			}
		}

		if bestIdx >= 0 && bestScore >= matchThreshold {
			elem := elements[bestIdx]
			report.Correspondences = append(report.Correspondences, Correspondence{
				Pattern:     p,
				SpecElement: elem.id,
				ElementType: elem.typ,
				Score:       bestScore,
			})
			if prev, ok := matched[elem.id]; !ok || bestScore > prev {
				matched[elem.id] = bestScore
			}
		} else {
			suggestion := suggestElementType(p)
			report.UndocumentedBehavior = append(report.UndocumentedBehavior, UndocumentedItem{
				Pattern:    p,
				Suggestion: suggestion,
			})
		}
	}

	// Reverse pass: for each spec element, check if any pattern references it.
	for _, elem := range elements {
		if score, ok := matched[elem.id]; ok && score >= reverseThreshold {
			continue // has code evidence
		}
		report.UnimplementedSpec = append(report.UnimplementedSpec, UnimplementedItem{
			ElementID:   elem.id,
			ElementType: elem.typ,
			Title:       elem.title,
		})
	}

	result.Reconciliation = report
	return nil
}

// Absorb runs the full absorption pipeline and returns a CommandResult.
func Absorb(opts AbsorbOptions) (*autoprompt.CommandResult, error) {
	// 1. Scan patterns from code.
	result, err := ScanPatterns(opts.CodeRoot)
	if err != nil {
		return nil, fmt.Errorf("scan patterns: %w", err)
	}

	// 2. If --against specified, reconcile.
	if opts.AgainstDB != "" {
		db, err := storage.Open(opts.AgainstDB)
		if err != nil {
			return nil, fmt.Errorf("open spec db: %w", err)
		}
		defer db.Close()

		specID, err := storage.GetFirstSpecID(db)
		if err != nil {
			return nil, fmt.Errorf("no spec found: %w", err)
		}

		if err := Reconcile(result, db, specID); err != nil {
			return nil, fmt.Errorf("reconcile: %w", err)
		}
	}

	// 3. Build output.
	var output string
	if result.Reconciliation != nil {
		output = RenderReconciliation(result.Reconciliation)
	} else {
		output = renderPatternSummary(result)
	}

	// 4. Compute state and guidance via k* budget.
	att := autoprompt.Attenuation(opts.Depth)

	// Estimate confidence from pattern analysis.
	var coverageConf, depthConf int
	if result.Reconciliation != nil {
		total := len(result.Reconciliation.Correspondences) +
			len(result.Reconciliation.UndocumentedBehavior) +
			len(result.Reconciliation.UnimplementedSpec)
		if total > 0 {
			coverageConf = 10 * len(result.Reconciliation.Correspondences) / total
		}
		depthConf = min(10, result.TotalPatterns/10)
	} else {
		coverageConf = 2 // no reconciliation = low coverage confidence
		depthConf = min(10, result.TotalPatterns/10)
	}

	// Build suggested next actions.
	var suggestions []string
	if result.Reconciliation != nil {
		if len(result.Reconciliation.UndocumentedBehavior) > 0 {
			suggestions = append(suggestions,
				fmt.Sprintf("Review %d undocumented patterns for spec crystallization",
					len(result.Reconciliation.UndocumentedBehavior)))
		}
		if len(result.Reconciliation.UnimplementedSpec) > 0 {
			suggestions = append(suggestions,
				fmt.Sprintf("Investigate %d unimplemented spec elements",
					len(result.Reconciliation.UnimplementedSpec)))
		}
		if len(result.Reconciliation.Correspondences) > 0 {
			suggestions = append(suggestions, "Run ddis refine to tighten correspondences")
		}
	} else {
		suggestions = append(suggestions, "Run ddis absorb --against <spec.db> for full reconciliation")
		suggestions = append(suggestions, "Run ddis refine to create draft spec from patterns")
	}

	return &autoprompt.CommandResult{
		Output: output,
		State: autoprompt.StateSnapshot{
			ActiveThread: "absorb",
			Confidence: [5]int{
				coverageConf,     // coverage
				depthConf,        // depth
				5,                // coherence (neutral)
				coverageConf / 2, // completeness
				3,                // formality (low for code-derived)
			},
			LimitingFactor:   "coverage",
			ArtifactsWritten: 0,
			SpecDrift:        estimateDrift(result),
			Iteration:        opts.Depth,
		},
		Guidance: autoprompt.Guidance{
			ObservedMode:  "convergent",
			DoFHint:       "mid",
			SuggestedNext: suggestions,
			Attenuation:   att,
		},
	}, nil
}

// RenderReconciliation formats the report as human-readable text.
func RenderReconciliation(report *ReconciliationReport) string {
	var b strings.Builder

	b.WriteString("## Reconciliation Report\n\n")

	b.WriteString(fmt.Sprintf("### Correspondences (%d)\n\n", len(report.Correspondences)))
	for _, c := range report.Correspondences {
		b.WriteString(fmt.Sprintf("- %s:%d (%s) <-> %s [%s] (score: %.2f)\n",
			c.Pattern.File, c.Pattern.Line, c.Pattern.Type,
			c.SpecElement, c.ElementType, c.Score))
	}

	b.WriteString(fmt.Sprintf("\n### Undocumented Behavior (%d)\n\n", len(report.UndocumentedBehavior)))
	for _, u := range report.UndocumentedBehavior {
		text := u.Pattern.Text
		if len(text) > 80 {
			text = text[:77] + "..."
		}
		b.WriteString(fmt.Sprintf("- %s:%d: %s (suggest: %s)\n",
			u.Pattern.File, u.Pattern.Line, text, u.Suggestion))
	}

	b.WriteString(fmt.Sprintf("\n### Unimplemented Spec (%d)\n\n", len(report.UnimplementedSpec)))
	for _, u := range report.UnimplementedSpec {
		b.WriteString(fmt.Sprintf("- %s [%s]: %s (no code evidence)\n",
			u.ElementID, u.ElementType, u.Title))
	}

	return b.String()
}

// renderPatternSummary formats patterns without reconciliation.
func renderPatternSummary(result *AbsorbResult) string {
	var b strings.Builder

	b.WriteString("## Absorption Scan Summary\n\n")
	b.WriteString(fmt.Sprintf("Files scanned: %d\n", result.TotalFiles))
	b.WriteString(fmt.Sprintf("Patterns found: %d\n\n", result.TotalPatterns))

	// Group by type.
	byType := make(map[string]int)
	for _, p := range result.Patterns {
		byType[p.Type]++
	}

	b.WriteString("### Pattern Distribution\n\n")
	for typ, count := range byType {
		b.WriteString(fmt.Sprintf("- %s: %d\n", typ, count))
	}

	b.WriteString("\n### Suggested Spec Structure\n\n")
	b.WriteString("Based on code patterns, the following spec elements are suggested:\n\n")

	if byType["assertion"] > 0 {
		b.WriteString(fmt.Sprintf("- **Invariants**: %d assertion patterns detected\n", byType["assertion"]))
	}
	if byType["error_return"] > 0 {
		b.WriteString(fmt.Sprintf("- **Error handling ADRs**: %d error return patterns\n", byType["error_return"]))
	}
	if byType["interface_def"] > 0 {
		b.WriteString(fmt.Sprintf("- **Interface specifications**: %d interface definitions\n", byType["interface_def"]))
	}
	if byType["state_transition"] > 0 {
		b.WriteString(fmt.Sprintf("- **State machine specs**: %d state transitions\n", byType["state_transition"]))
	}
	if byType["guard_clause"] > 0 {
		b.WriteString(fmt.Sprintf("- **Guard invariants**: %d guard clauses\n", byType["guard_clause"]))
	}

	return b.String()
}

// wordSet splits text into a set of lowercase words, filtering short tokens.
func wordSet(text string) map[string]bool {
	words := make(map[string]bool)
	for _, w := range strings.Fields(strings.ToLower(text)) {
		// Strip punctuation and skip short words.
		w = strings.Trim(w, ".,;:!?(){}[]\"'`")
		if len(w) >= 3 {
			words[w] = true
		}
	}
	return words
}

// keywordOverlap computes |intersection| / max(|set1|, |set2|).
func keywordOverlap(a, b map[string]bool) float64 {
	if len(a) == 0 || len(b) == 0 {
		return 0
	}

	intersection := 0
	for w := range a {
		if b[w] {
			intersection++
		}
	}

	denom := len(a)
	if len(b) > denom {
		denom = len(b)
	}

	return float64(intersection) / float64(denom)
}

// extractAnnotationTarget pulls the spec element ID from a ddis annotation comment.
func extractAnnotationTarget(text string) string {
	// Annotation text example: "implements APP-INV-032 (qualifier)"
	// or "maintains INV-006"
	lower := strings.ToLower(text)
	idx := strings.Index(lower, "ddis:")
	if idx < 0 {
		return ""
	}
	after := text[idx:]
	parts := strings.Fields(after)
	if len(parts) >= 2 {
		return parts[1]
	}
	return ""
}

// suggestElementType proposes what kind of spec element a pattern should become.
func suggestElementType(p Pattern) string {
	switch p.Type {
	case "assertion":
		return "invariant"
	case "error_return":
		return "adr"
	case "guard_clause":
		return "invariant"
	case "state_transition":
		return "state_machine"
	case "interface_def":
		return "interface_spec"
	default:
		return "section"
	}
}

// estimateDrift computes a rough drift score from reconciliation data.
func estimateDrift(result *AbsorbResult) float64 {
	if result.Reconciliation == nil {
		return -1 // unknown
	}
	r := result.Reconciliation
	unspec := len(r.UndocumentedBehavior)
	unimpl := len(r.UnimplementedSpec)
	// Drift formula from the spec: |unspecified| + |unimplemented| + 2*|contradictions|
	// We have no contradiction detection here, so just sum the two categories.
	return float64(unspec + unimpl)
}

// min returns the smaller of two ints.
func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
