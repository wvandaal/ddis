package refine

// ddis:maintains APP-INV-023 (prompt self-containment — Apply bundles element content, exemplars, and gap analysis into complete prompt)
// ddis:implements APP-INV-035 (guidance attenuation — enforces TokenTarget budget; trims exemplar set on deep conversations via KStarEff)

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
	RawText   string // full element text for Gestalt demonstrations
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
		rawText string
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
				rawText: inv.RawText,
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
						rawText: adr.RawText,
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
			RawText:   c.rawText,
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

// capitalizeFirst returns s with its first letter capitalized.
func capitalizeFirst(s string) string {
	if s == "" {
		return s
	}
	return strings.ToUpper(s[:1]) + s[1:]
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

// dimensionFraming returns spec-first framing text that formalizes what quality
// means for the given dimension before any task is assigned. This activates the
// LLM's domain understanding (Gestalt spec-first framing principle).
func dimensionFraming(dimension string) string {
	switch dimension {
	case "completeness":
		return "A complete invariant creates an interlocking proof structure: " +
			"the statement asserts the property, the semi-formal predicate makes it " +
			"mechanically checkable, the violation scenario proves it is falsifiable, " +
			"the validation method makes it testable, and why-this-matters connects " +
			"it to system value. Each component constrains interpretation of the others. " +
			"A complete ADR similarly requires that the problem, decision, chosen option " +
			"rationale, consequences, and tests form a coherent decision record where " +
			"no component can be removed without losing explanatory power."
	case "coherence":
		return "Coherence means every reference resolves and every claim connects " +
			"to the web of specification. An isolated element is dead weight; a connected " +
			"element amplifies the specification's explanatory power. Cross-references " +
			"must point to existing elements. WHY NOT annotations must reference " +
			"rejected alternatives. Semi-formal predicates must use terms defined in " +
			"the glossary or earlier in the spec."
	case "depth":
		return "Depth is the distance between a violation scenario and reality. " +
			"A shallow scenario states the obvious; a deep scenario captures the " +
			"non-obvious failure mode that only emerges under specific conditions. " +
			"Deep validation methods go beyond 'check that X holds' to describe " +
			"the exact procedure, inputs, and expected outputs."
	case "coverage":
		return "Coverage measures how much of the domain surface the element " +
			"addresses. A high-coverage element anticipates the full range of " +
			"situations where the property applies. For invariants, this means " +
			"the violation scenario covers edge cases, not just the happy path. " +
			"For ADRs, this means consequences address both positive and negative " +
			"outcomes, and tests verify both the chosen option and rejected alternatives."
	case "formality":
		return "Formality is the bridge between natural language intent and mechanical " +
			"verification. A semi-formal predicate translates English claims into " +
			"logical structure that a validator can check. It uses quantifiers " +
			"(FOR ALL, THERE EXISTS), logical connectives (AND, OR, IMPLIES), " +
			"and domain-specific predicates with precise scope."
	default:
		return "Quality in specification means every element earns its place " +
			"through interlocking components that constrain interpretation, " +
			"connect to the broader spec web, and enable mechanical verification."
	}
}

// activatingDirective returns a dimension-specific output instruction that uses
// spec language to trigger deep reasoning, replacing generic "Return ONLY" instructions.
func activatingDirective(dimension string, weak *weakestElement) string {
	switch dimension {
	case "completeness":
		return fmt.Sprintf("Rewrite %s so every component interlocks. "+
			"What would a reviewer need to see to trust this property holds? "+
			"Preserve all existing correct content. Output only the improved element "+
			"in the same markdown format.", weak.ElementID)
	case "coherence":
		return fmt.Sprintf("Rewrite %s so every reference resolves and every claim "+
			"connects to the specification web. Add cross-references where the element "+
			"touches other spec elements. Output only the improved element "+
			"in the same markdown format.", weak.ElementID)
	case "depth":
		return fmt.Sprintf("Deepen %s. What is the non-obvious failure mode? "+
			"Under what specific conditions does this property break? "+
			"Replace shallow scenarios with concrete, falsifiable ones. "+
			"Output only the improved element in the same markdown format.", weak.ElementID)
	case "coverage":
		return fmt.Sprintf("Broaden %s to cover the full domain surface. "+
			"What edge cases are missing? What situations does the current "+
			"element fail to anticipate? Output only the improved element "+
			"in the same markdown format.", weak.ElementID)
	case "formality":
		return fmt.Sprintf("Add or strengthen the semi-formal predicate for %s. "+
			"What are the quantifiers, the domain, and the logical connectives? "+
			"Translate the English claim into structure a machine could evaluate. "+
			"Output only the improved element in the same markdown format.", weak.ElementID)
	default:
		return fmt.Sprintf("Improve %s on the %s dimension. "+
			"Output only the improved element in the same markdown format.",
			weak.ElementID, dimension)
	}
}

// Apply generates an LLM edit prompt with exemplars for the selected dimension.
// The prompt follows Gestalt principles: spec-first framing → demonstrations → element → activating directive.
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

	// 5. Assemble Gestalt-optimized prompt (4 sections):
	//   1. Spec-first dimension framing (activates domain understanding)
	//   2. Full exemplar demonstrations (primes quality pattern)
	//   3. Current element + diagnosis (now LLM has context)
	//   4. Activating directive (triggers deep work in spec language)
	tokenTarget := autoprompt.TokenTarget(iteration)

	// Section 1: Spec-first dimension framing (~150 tokens)
	framing := fmt.Sprintf(
		"## %s in DDIS Specification\n\n%s\n",
		capitalizeFirst(dimension), dimensionFraming(dimension))

	// Section 2: Full exemplar demonstrations (shows complete elements, not just components)
	var exemSection strings.Builder
	if len(exemplars) > 0 {
		exemSection.WriteString("\n## Exemplar Demonstrations\n\n")
		exemSection.WriteString("The following elements demonstrate excellence. Study their structure, tone, and depth.\n\n")
		for i, ex := range exemplars {
			// Use full RawText if available; fall back to component text
			content := ex.RawText
			if content == "" {
				content = ex.Content
			}
			fmt.Fprintf(&exemSection, "### Exemplar %d: %s (%s)\n\n%s\n\n",
				i+1, ex.ElementID, ex.Title, content)
		}
	}

	// Section 3: Current element + diagnosis
	currentElement := fmt.Sprintf(
		"\n## Element to Improve: %s (%s)\n\nType: %s | Weak dimension: %s\n\n%s\n",
		weak.ElementID, weak.Title, weak.ElementType, dimension, weak.RawText)

	// Section 4: Activating directive (spec-language output instruction)
	directive := fmt.Sprintf("\n## Your Task\n\n%s\n", activatingDirective(dimension, weak))

	// 6. Budget trimming: if total exceeds TokenTarget, trim in priority order
	// Rough estimate: 1 token ~ 4 chars
	// Priority: trim exemplars from N to 1 → shorten framing → never trim element or directive
	charBudget := tokenTarget * 4
	totalChars := len(framing) + exemSection.Len() + len(currentElement) + len(directive)

	if totalChars > charBudget && len(exemplars) > 1 {
		// Reduce exemplars to 1 (still keep one demonstration)
		exemSection.Reset()
		exemSection.WriteString("\n## Exemplar Demonstration\n\n")
		content := exemplars[0].RawText
		if content == "" {
			content = exemplars[0].Content
		}
		fmt.Fprintf(&exemSection, "### %s (%s)\n\n%s\n\n",
			exemplars[0].ElementID, exemplars[0].Title, content)
		totalChars = len(framing) + exemSection.Len() + len(currentElement) + len(directive)
	}
	if totalChars > charBudget {
		// Shorten dimension framing to 1-sentence version
		framing = fmt.Sprintf("## Improving %s\n\n", dimension)
		totalChars = len(framing) + exemSection.Len() + len(currentElement) + len(directive)
	}
	if totalChars > charBudget && exemSection.Len() > 0 {
		// Drop exemplars entirely as last resort before touching element/directive
		exemSection.Reset()
	}
	// NEVER trim current element or activating directive

	// 7. Assemble final prompt (Gestalt order: framing → demonstrations → element → directive)
	var prompt strings.Builder
	prompt.WriteString(framing)
	prompt.WriteString(exemSection.String())
	prompt.WriteString(currentElement)
	prompt.WriteString(directive)

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
