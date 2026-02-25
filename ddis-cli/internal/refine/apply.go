package refine

import (
	"database/sql"
	"fmt"
	"strconv"
	"strings"

	"github.com/wvandaal/ddis/internal/autoprompt"
	"github.com/wvandaal/ddis/internal/exemplar"
	"github.com/wvandaal/ddis/internal/state"
	"github.com/wvandaal/ddis/internal/storage"
)

// weakestElement holds the element selected for improvement.
type weakestElement struct {
	ElementType string // "invariant" or "adr"
	ElementID   string
	Title       string
	RawText     string
	Fields      map[string]string
	WeakScore   float64
}

// exemplarEntry is a lightweight representation of a high-quality element for prompting.
type exemplarEntry struct {
	ElementID string
	Title     string
	Content   string // the strong component text
}

// findWeakest finds the element with the worst score on the given dimension.
func findWeakest(db *sql.DB, specID int64, dimension string) (*weakestElement, error) {
	switch dimension {
	case "completeness":
		return findLeastCompleteElement(db, specID)
	case "coherence":
		return findLeastCoherentElement(db, specID)
	case "depth":
		return findShallowestInvariant(db, specID)
	case "coverage":
		return findLeastCoveredElement(db, specID)
	case "formality":
		return findLeastFormalInvariant(db, specID)
	default:
		return findLeastCompleteElement(db, specID)
	}
}

// findLeastCompleteElement returns the invariant or ADR with the fewest non-empty components.
func findLeastCompleteElement(db *sql.DB, specID int64) (*weakestElement, error) {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, err
	}

	var worst *weakestElement
	worstCount := 6 // sentinel above max component count

	for _, inv := range invs {
		fields := exemplar.ExtractInvariantFields(inv)
		count := countNonEmpty(fields)
		if count < worstCount {
			worstCount = count
			worst = &weakestElement{
				ElementType: "invariant",
				ElementID:   inv.InvariantID,
				Title:       inv.Title,
				RawText:     inv.RawText,
				Fields:      fields,
			}
		}
	}

	adrs, err := storage.ListADRs(db, specID)
	if err != nil {
		return nil, err
	}
	for _, adr := range adrs {
		fields := exemplar.ExtractADRFields(adr)
		count := countNonEmpty(fields)
		if count < worstCount {
			worstCount = count
			worst = &weakestElement{
				ElementType: "adr",
				ElementID:   adr.ADRID,
				Title:       adr.Title,
				RawText:     adr.RawText,
				Fields:      fields,
			}
		}
	}

	if worst == nil {
		return nil, fmt.Errorf("no elements found")
	}
	return worst, nil
}

// findLeastCoherentElement returns the element whose section has the most unresolved xrefs.
func findLeastCoherentElement(db *sql.DB, specID int64) (*weakestElement, error) {
	// Find the invariant with the most unresolved outgoing refs in its section
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, err
	}

	var worst *weakestElement
	worstRefs := -1

	for _, inv := range invs {
		refs, err := storage.GetOutgoingRefs(db, specID, inv.SectionID)
		if err != nil {
			continue
		}
		unresolved := 0
		for _, r := range refs {
			if !r.Resolved {
				unresolved++
			}
		}
		if unresolved > worstRefs {
			worstRefs = unresolved
			worst = &weakestElement{
				ElementType: "invariant",
				ElementID:   inv.InvariantID,
				Title:       inv.Title,
				RawText:     inv.RawText,
				Fields:      exemplar.ExtractInvariantFields(inv),
			}
		}
	}

	if worst == nil {
		// Fall back to least complete
		return findLeastCompleteElement(db, specID)
	}
	return worst, nil
}

// findShallowestInvariant returns the invariant with the shortest violation_scenario.
func findShallowestInvariant(db *sql.DB, specID int64) (*weakestElement, error) {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, err
	}

	var worst *weakestElement
	worstLen := 1<<31 - 1 // max int

	for _, inv := range invs {
		vLen := len(inv.ViolationScenario)
		// Missing counts as 0, which is the shallowest
		if vLen < worstLen {
			worstLen = vLen
			worst = &weakestElement{
				ElementType: "invariant",
				ElementID:   inv.InvariantID,
				Title:       inv.Title,
				RawText:     inv.RawText,
				Fields:      exemplar.ExtractInvariantFields(inv),
			}
		}
	}

	if worst == nil {
		return nil, fmt.Errorf("no invariants found")
	}
	return worst, nil
}

// findLeastCoveredElement returns the element with the lowest overall WeakScore.
func findLeastCoveredElement(db *sql.DB, specID int64) (*weakestElement, error) {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, err
	}

	var worst *weakestElement
	worstScore := 100.0 // sentinel

	for _, inv := range invs {
		fields := exemplar.ExtractInvariantFields(inv)
		avgScore := averageWeakScore(fields, "invariant")
		if avgScore < worstScore {
			worstScore = avgScore
			worst = &weakestElement{
				ElementType: "invariant",
				ElementID:   inv.InvariantID,
				Title:       inv.Title,
				RawText:     inv.RawText,
				Fields:      fields,
				WeakScore:   avgScore,
			}
		}
	}

	adrs, err := storage.ListADRs(db, specID)
	if err != nil {
		return nil, err
	}
	for _, adr := range adrs {
		fields := exemplar.ExtractADRFields(adr)
		avgScore := averageWeakScore(fields, "adr")
		if avgScore < worstScore {
			worstScore = avgScore
			worst = &weakestElement{
				ElementType: "adr",
				ElementID:   adr.ADRID,
				Title:       adr.Title,
				RawText:     adr.RawText,
				Fields:      fields,
				WeakScore:   avgScore,
			}
		}
	}

	if worst == nil {
		return nil, fmt.Errorf("no elements found")
	}
	return worst, nil
}

// findLeastFormalInvariant returns the invariant missing or having the weakest semi_formal.
func findLeastFormalInvariant(db *sql.DB, specID int64) (*weakestElement, error) {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, err
	}

	var worst *weakestElement
	worstScore := 100.0

	for _, inv := range invs {
		fields := exemplar.ExtractInvariantFields(inv)
		score := exemplar.WeakScore(fields["semi_formal"], "semi_formal", "invariant")
		if score < worstScore {
			worstScore = score
			worst = &weakestElement{
				ElementType: "invariant",
				ElementID:   inv.InvariantID,
				Title:       inv.Title,
				RawText:     inv.RawText,
				Fields:      fields,
				WeakScore:   score,
			}
		}
	}

	if worst == nil {
		return nil, fmt.Errorf("no invariants found")
	}
	return worst, nil
}

// selectExemplars finds 1-3 high-quality elements for the given dimension.
func selectExemplars(db *sql.DB, specID int64, dimension, excludeID string, maxCount int) []exemplarEntry {
	// Map dimension to the component we want strong exemplars for
	component := dimensionToComponent(dimension)

	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil
	}

	type scored struct {
		id      string
		title   string
		content string
		score   float64
	}

	var candidates []scored
	for _, inv := range invs {
		if inv.InvariantID == excludeID {
			continue
		}
		fields := exemplar.ExtractInvariantFields(inv)
		text := fields[component]
		if text == "" {
			continue
		}
		score := exemplar.WeakScore(text, component, "invariant")
		if score > 0.6 {
			candidates = append(candidates, scored{
				id:      inv.InvariantID,
				title:   inv.Title,
				content: text,
				score:   score,
			})
		}
	}

	// Also check ADRs if the component maps to ADR fields
	adrComponent := dimensionToADRComponent(dimension)
	if adrComponent != "" {
		adrs, err := storage.ListADRs(db, specID)
		if err == nil {
			for _, adr := range adrs {
				if adr.ADRID == excludeID {
					continue
				}
				fields := exemplar.ExtractADRFields(adr)
				text := fields[adrComponent]
				if text == "" {
					continue
				}
				score := exemplar.WeakScore(text, adrComponent, "adr")
				if score > 0.6 {
					candidates = append(candidates, scored{
						id:      adr.ADRID,
						title:   adr.Title,
						content: text,
						score:   score,
					})
				}
			}
		}
	}

	// Sort by score descending, take top N
	for i := 0; i < len(candidates)-1; i++ {
		for j := i + 1; j < len(candidates); j++ {
			if candidates[j].score > candidates[i].score {
				candidates[i], candidates[j] = candidates[j], candidates[i]
			}
		}
	}

	if len(candidates) > maxCount {
		candidates = candidates[:maxCount]
	}

	var result []exemplarEntry
	for _, c := range candidates {
		result = append(result, exemplarEntry{
			ElementID: c.id,
			Title:     c.title,
			Content:   c.content,
		})
	}
	return result
}

// dimensionToComponent maps quality dimensions to invariant component names.
func dimensionToComponent(dimension string) string {
	switch dimension {
	case "completeness":
		return "statement" // least likely to be empty, so exemplars show full structure
	case "coherence":
		return "validation_method"
	case "depth":
		return "violation_scenario"
	case "coverage":
		return "statement"
	case "formality":
		return "semi_formal"
	default:
		return "statement"
	}
}

// dimensionToADRComponent maps quality dimensions to ADR component names (empty if N/A).
func dimensionToADRComponent(dimension string) string {
	switch dimension {
	case "completeness":
		return "chosen_option"
	case "coherence":
		return "consequences"
	case "depth":
		return "tests"
	case "coverage":
		return "problem"
	default:
		return ""
	}
}

// countNonEmpty returns the number of non-empty fields in the map.
func countNonEmpty(fields map[string]string) int {
	count := 0
	for _, v := range fields {
		if v != "" {
			count++
		}
	}
	return count
}

// averageWeakScore computes the mean WeakScore across all components for an element.
func averageWeakScore(fields map[string]string, elementType string) float64 {
	components := exemplar.ComponentsForType(elementType)
	if len(components) == 0 {
		return 0
	}
	total := 0.0
	for _, comp := range components {
		total += exemplar.WeakScore(fields[comp], comp, elementType)
	}
	return total / float64(len(components))
}

// Apply generates an LLM edit prompt with exemplars for the selected dimension.
// The prompt follows: demonstrations > constraints.
func Apply(db *sql.DB, specID int64, iteration int) (*autoprompt.CommandResult, error) {
	// 1. Get selected dimension from state (or recompute via plan logic)
	dimension, err := state.Get(db, specID, "refine_focus_"+strconv.Itoa(iteration))
	if err != nil {
		// Recompute: run plan logic inline
		sid, _, sidErr := computeSpecInternalDrift(db, specID)
		if sidErr != nil {
			return nil, fmt.Errorf("compute drift for plan fallback: %w", sidErr)
		}
		conf := deriveConfidence(sid)
		dimension, _ = selectFocusDimension(conf)
	}

	// 2. Find the weakest element in that dimension
	weak, err := findWeakest(db, specID, dimension)
	if err != nil {
		return nil, fmt.Errorf("find weakest element: %w", err)
	}

	// 3. Determine exemplar budget from k* attenuation
	kStar := autoprompt.KStarEff(iteration)
	maxExemplars := 3
	if kStar <= 5 {
		maxExemplars = 1
	} else if kStar <= 8 {
		maxExemplars = 2
	}

	// 4. Select exemplars
	exemplars := selectExemplars(db, specID, dimension, weak.ElementID, maxExemplars)

	// 5. Load negative specs for constraint generation
	negSpecs, _ := storage.ListNegativeSpecs(db, specID)

	// 6. Assemble prompt sections
	tokenTarget := autoprompt.TokenTarget(iteration)

	// Section A: Spec-first framing (~200 tokens)
	framing := fmt.Sprintf(
		"You are improving the **%s** dimension of a DDIS specification element.\n"+
			"The element below scores poorly on %s. Your task is to rewrite it\n"+
			"so that it matches the quality demonstrated by the exemplars.\n"+
			"Preserve all existing correct content. Only add or improve.\n",
		dimension, dimension)

	// Section B: Current element (variable size)
	currentElement := fmt.Sprintf(
		"\n## Current Element: %s (%s)\nType: %s\n\n%s\n",
		weak.ElementID, weak.Title, weak.ElementType, weak.RawText)

	// Section C: Exemplar demonstrations
	var exemSection strings.Builder
	if len(exemplars) > 0 {
		exemSection.WriteString("\n## Exemplar Demonstrations\n\n")
		for i, ex := range exemplars {
			fmt.Fprintf(&exemSection, "### Exemplar %d: %s (%s)\n%s\n\n",
				i+1, ex.ElementID, ex.Title, ex.Content)
		}
	}

	// Section D: Quality criteria (~100 tokens)
	criteria := fmt.Sprintf(
		"\n## Quality Criteria for %s\n"+
			"- Every invariant MUST have: statement, semi-formal predicate, violation scenario, validation method, why-this-matters\n"+
			"- Every ADR MUST have: problem, decision text, chosen option, consequences, tests\n"+
			"- Cross-references must resolve to existing elements\n"+
			"- Semi-formal predicates should use logical operators\n",
		dimension)

	// Section E: Constraints from negative specs (~50 tokens)
	var constraints strings.Builder
	if len(negSpecs) > 0 {
		constraints.WriteString("\n## Constraints (DO NOT)\n")
		cap := 3
		if len(negSpecs) < cap {
			cap = len(negSpecs)
		}
		for _, ns := range negSpecs[:cap] {
			fmt.Fprintf(&constraints, "- %s\n", ns.ConstraintText)
		}
	}

	// Section F: Output format (~50 tokens)
	outputFormat := "\n## Output Format\nReturn ONLY the improved element in the same markdown format as the original.\n" +
		"Do not include explanations or commentary outside the element.\n"

	// 7. Budget trimming: if total exceeds TokenTarget, trim in order
	// Rough estimate: 1 token ~ 4 chars
	totalChars := len(framing) + len(currentElement) + exemSection.Len() +
		len(criteria) + constraints.Len() + len(outputFormat)
	charBudget := tokenTarget * 4

	constraintStr := constraints.String()
	criteriaStr := criteria

	if totalChars > charBudget {
		// Trim constraints first
		constraintStr = ""
		totalChars = len(framing) + len(currentElement) + exemSection.Len() +
			len(criteriaStr) + len(outputFormat)
	}
	if totalChars > charBudget {
		// Trim criteria second
		criteriaStr = ""
		totalChars = len(framing) + len(currentElement) + exemSection.Len() + len(outputFormat)
	}
	if totalChars > charBudget && len(exemplars) > 1 {
		// Reduce exemplars to 1
		var trimmed strings.Builder
		trimmed.WriteString("\n## Exemplar Demonstration\n\n")
		fmt.Fprintf(&trimmed, "### Exemplar: %s (%s)\n%s\n\n",
			exemplars[0].ElementID, exemplars[0].Title, exemplars[0].Content)
		exemSection.Reset()
		exemSection.WriteString(trimmed.String())
	}
	// NEVER trim framing or current element

	// 8. Assemble final prompt
	var prompt strings.Builder
	prompt.WriteString(framing)
	prompt.WriteString(currentElement)
	prompt.WriteString(exemSection.String())
	prompt.WriteString(criteriaStr)
	prompt.WriteString(constraintStr)
	prompt.WriteString(outputFormat)

	// 9. Store apply state
	_ = state.Set(db, specID, "refine_target_"+strconv.Itoa(iteration), weak.ElementID)

	// 10. Build CommandResult
	attenuation := autoprompt.Attenuation(iteration)

	return &autoprompt.CommandResult{
		Output: prompt.String(),
		State: autoprompt.StateSnapshot{
			ActiveThread:   "refine",
			LimitingFactor: dimension,
			Iteration:      iteration,
			ModeObserved:   "crystallization",
		},
		Guidance: autoprompt.Guidance{
			ObservedMode: "crystallization",
			DoFHint:      "very_low",
			SuggestedNext: []string{
				"Apply the generated edit to the spec",
				"Then run 'ddis refine judge' to evaluate",
			},
			TranslationHint: fmt.Sprintf("Improving %s (%s) on %s dimension", weak.ElementID, weak.Title, dimension),
			RelevantContext: []string{weak.ElementID},
			Attenuation:     attenuation,
		},
	}, nil
}
