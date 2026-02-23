package exemplar

import (
	"fmt"
	"math"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// Length thresholds for weakness detection.
var invariantThresholds = map[string]int{
	"statement":          40,
	"semi_formal":        60,
	"violation_scenario": 100,
	"validation_method":  60,
	"why_this_matters":   40,
}

var adrThresholds = map[string]int{
	"problem":       40,
	"decision_text": 60,
	"chosen_option": 20,
	"consequences":  40,
	"tests":         20,
}

// DetectGaps finds missing or weak components for an element.
// If opts.Gap is non-empty, only that component is checked.
func DetectGaps(elementType string, fields map[string]string, opts Options) []ComponentGap {
	components := ComponentsForType(elementType)
	if components == nil {
		return nil
	}

	thresholds := invariantThresholds
	if elementType == "adr" {
		thresholds = adrThresholds
	}

	var gaps []ComponentGap
	for _, comp := range components {
		if opts.Gap != "" && comp != opts.Gap {
			continue
		}
		text := fields[comp]
		score := WeakScore(text, comp, elementType)
		if score >= 0.6 {
			continue // strong enough
		}
		gap := ComponentGap{
			Component: comp,
			WeakScore: score,
		}
		if text == "" {
			gap.Severity = "missing"
			gap.Detail = fmt.Sprintf("No %s specified", humanName(comp))
		} else {
			gap.Severity = "weak"
			gap.Detail = fmt.Sprintf("%s is present but below quality threshold (%d chars, threshold %d)",
				humanName(comp), len(text), thresholds[comp])
		}
		gaps = append(gaps, gap)
	}
	return gaps
}

// WeakScore computes a [0,1] quality score for a component's text.
// Returns 0.0 for empty text, up to 1.0 for strong text.
func WeakScore(text, component, elementType string) float64 {
	if text == "" {
		return 0.0
	}

	thresholds := invariantThresholds
	if elementType == "adr" {
		thresholds = adrThresholds
	}

	threshold, ok := thresholds[component]
	if !ok {
		threshold = 40
	}

	// Base score: length relative to threshold, capped at 1.0
	base := math.Min(1.0, float64(len(text))/float64(threshold))

	// Structural bonus for specific components
	bonus := structuralBonus(text, component)

	return math.Min(1.0, base+bonus)
}

// structuralBonus awards +0.15 for structural markers in specific components.
func structuralBonus(text, component string) float64 {
	lower := strings.ToLower(text)
	switch component {
	case "semi_formal":
		// Logical operators: ∀, ∃, ⟹, ∧, ∨, →, ⊆, forall, implies
		for _, marker := range []string{"∀", "∃", "⟹", "∧", "∨", "→", "⊆", "forall", "implies", "iff"} {
			if strings.Contains(lower, strings.ToLower(marker)) {
				return 0.15
			}
		}
	case "violation_scenario":
		for _, marker := range []string{"when", "if ", "suppose", "given"} {
			if strings.Contains(lower, marker) {
				return 0.15
			}
		}
	case "validation_method":
		for _, marker := range []string{"check", "verify", "test", "run", "execute", "assert", "grep", "parse"} {
			if strings.Contains(lower, marker) {
				return 0.15
			}
		}
	}
	return 0.0
}

// ExtractInvariantFields returns a component name → text map for an invariant.
func ExtractInvariantFields(inv storage.Invariant) map[string]string {
	return map[string]string{
		"statement":          inv.Statement,
		"semi_formal":        inv.SemiFormal,
		"violation_scenario": inv.ViolationScenario,
		"validation_method":  inv.ValidationMethod,
		"why_this_matters":   inv.WhyThisMatters,
	}
}

// ExtractADRFields returns a component name → text map for an ADR.
func ExtractADRFields(adr storage.ADR) map[string]string {
	return map[string]string{
		"problem":       adr.Problem,
		"decision_text": adr.DecisionText,
		"chosen_option": adr.ChosenOption,
		"consequences":  adr.Consequences,
		"tests":         adr.Tests,
	}
}

// humanName converts component keys to human-readable names.
func humanName(component string) string {
	switch component {
	case "statement":
		return "statement"
	case "semi_formal":
		return "semi-formal predicate"
	case "violation_scenario":
		return "violation scenario"
	case "validation_method":
		return "validation method"
	case "why_this_matters":
		return "why-this-matters rationale"
	case "problem":
		return "problem statement"
	case "decision_text":
		return "decision text"
	case "chosen_option":
		return "chosen option"
	case "consequences":
		return "consequences"
	case "tests":
		return "tests"
	default:
		return strings.ReplaceAll(component, "_", " ")
	}
}
