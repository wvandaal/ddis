package exemplar

// ddis:maintains APP-INV-012 (LSI dimension bound)

import (
	"database/sql"
	"fmt"

	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

// Analyze is the top-level entry point for exemplar analysis.
// It detects gaps in the target element, finds corpus exemplars, and
// generates substrate cues for each gap.
func Analyze(db *sql.DB, specID int64, lsi *search.LSIIndex, opts Options) (*ExemplarResult, error) {
	target := opts.Target

	// Resolve target element
	elementType, title, fields, err := resolveTarget(db, specID, target)
	if err != nil {
		return nil, fmt.Errorf("resolve target %q: %w", target, err)
	}

	// Step 1: Detect gaps
	gaps := DetectGaps(elementType, fields, opts)

	// Step 2+3: Find exemplars (includes scoring and cue generation)
	exemplars, err := FindExemplars(db, specID, target, elementType, gaps, lsi, opts)
	if err != nil {
		return nil, fmt.Errorf("find exemplars: %w", err)
	}

	// Ensure non-nil slices for clean JSON serialization ([] not null)
	if gaps == nil {
		gaps = []ComponentGap{}
	}
	if exemplars == nil {
		exemplars = []Exemplar{}
	}

	// Generate guidance
	guidance := generateGuidance(target, gaps, exemplars, opts)

	return &ExemplarResult{
		Target:      target,
		ElementType: elementType,
		Title:       title,
		Gaps:        gaps,
		Exemplars:   exemplars,
		Guidance:    guidance,
	}, nil
}

// resolveTarget looks up the element in the DB and returns its type, title, and fields.
func resolveTarget(db *sql.DB, specID int64, target string) (string, string, map[string]string, error) {
	// Try invariant first
	inv, err := storage.GetInvariant(db, specID, target)
	if err == nil && inv != nil {
		return "invariant", inv.Title, ExtractInvariantFields(*inv), nil
	}

	// Try ADR
	adr, err := storage.GetADR(db, specID, target)
	if err == nil && adr != nil {
		return "adr", adr.Title, ExtractADRFields(*adr), nil
	}

	return "", "", nil, fmt.Errorf("element %q not found as invariant or ADR", target)
}

// generateGuidance produces a human-readable guidance string.
func generateGuidance(target string, gaps []ComponentGap, exemplars []Exemplar, opts Options) string {
	if len(gaps) == 0 {
		return fmt.Sprintf("%s has no component gaps. All fields present and above quality threshold.", target)
	}

	minScore := opts.MinScore
	if minScore <= 0 {
		minScore = 0.3
	}

	aboveThreshold := len(exemplars)
	gapCount := len(gaps)

	if aboveThreshold == 0 {
		return fmt.Sprintf("%s is missing %d component(s). No exemplars found above quality threshold (%.1f). Consider lowering --min-score or adding a richer corpus with --corpus.",
			target, gapCount, minScore)
	}

	return fmt.Sprintf("%s is missing %d component(s). %d exemplar(s) found above quality threshold (%.1f). Study the demonstrated component(s), then write %s's with equivalent specificity.",
		target, gapCount, aboveThreshold, minScore, target)
}
