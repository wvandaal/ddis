package exemplar

import (
	"fmt"
	"strings"
)

// cueTemplates maps component names to substrate cue templates.
// Each template contains {exemplar} and {target} placeholders.
// All cues follow the 4-element structure:
//  1. Label ("DEMONSTRATION:")
//  2. Attribution (exemplar ID)
//  3. Structural observation (what pattern to notice)
//  4. Transfer instruction ("Apply...to target")
var cueTemplates = map[string]string{
	// Invariant components
	"statement": "DEMONSTRATION: %s's statement precisely names the system property " +
		"being preserved — not a vague goal but a testable predicate. Notice the " +
		"verb choice and specific scope. Apply the same precision to %s's statement.",

	"semi_formal": "DEMONSTRATION: %s's semi-formal predicate uses logical operators " +
		"to express the invariant as a falsifiable proposition. Notice how it binds " +
		"variables to specific domain entities. Express %s's predicate with equivalent formalism.",

	"violation_scenario": "DEMONSTRATION: %s's violation scenario names a specific state " +
		"transition rather than generic failure. Notice how it identifies the exact " +
		"precondition that must be violated. Apply the same specificity to %s's violation scenario.",

	"validation_method": "DEMONSTRATION: %s's validation method specifies a concrete test " +
		"procedure with observable pass/fail criteria. Notice the tool, input, and " +
		"expected output are all named. Define %s's validation with equivalent concreteness.",

	"why_this_matters": "DEMONSTRATION: %s's rationale connects the invariant to a " +
		"user-visible consequence — not 'this is important' but 'without this, X " +
		"breaks for users doing Y.' Ground %s's rationale in the same concrete impact.",

	// ADR components
	"problem": "DEMONSTRATION: %s's problem statement names the specific tension " +
		"being resolved — two competing requirements that cannot both be satisfied " +
		"without a decision. Frame %s's problem as a similar trade-off.",

	"decision_text": "DEMONSTRATION: %s's decision text states the choice AND the " +
		"reasoning. Notice the explicit trade-off acknowledgment. State %s's " +
		"decision with equivalent transparency.",

	"chosen_option": "DEMONSTRATION: %s names its chosen option alongside rejected " +
		"alternatives. The comparison makes the decision legible. Provide " +
		"%s's chosen option with the same contrast.",

	"consequences": "DEMONSTRATION: %s's consequences name both benefits gained and " +
		"costs accepted. Notice the honesty about trade-offs. Document %s's " +
		"consequences with equivalent balance.",

	"tests": "DEMONSTRATION: %s's test section names specific, executable " +
		"verification steps — concrete commands or checks, not vague 'verify " +
		"correctness.' Specify %s's tests with the same precision.",
}

// GenerateSubstrateCue produces a substrate cue for the given exemplar and target.
// Returns empty string if no template exists for the component.
func GenerateSubstrateCue(exemplarID, targetID, component string) string {
	tmpl, ok := cueTemplates[component]
	if !ok {
		return ""
	}
	// Count %s placeholders — all templates have exactly 2
	count := strings.Count(tmpl, "%s")
	if count != 2 {
		return ""
	}
	return fmt.Sprintf(tmpl, exemplarID, targetID)
}
