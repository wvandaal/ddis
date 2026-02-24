package drift

import (
	"database/sql"
	"fmt"
	"strings"

	"github.com/wvandaal/ddis/internal/exemplar"
	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

// Remediate produces an actionable remediation package for the highest-priority
// drift item. Returns nil when drift is zero.
func Remediate(db *sql.DB, specID int64, lsi *search.LSIIndex) (*RemediationPackage, error) {
	report, err := Analyze(db, specID, Options{})
	if err != nil {
		return nil, fmt.Errorf("analyze drift: %w", err)
	}

	if report.EffectiveDrift == 0 {
		return nil, nil
	}

	// Pick highest-priority item: correctness > coherence > depth
	target, driftType, location := pickTarget(report)
	if target == "" {
		return nil, nil
	}

	pkg := &RemediationPackage{
		Target:        target,
		DriftType:     driftType,
		Priority:      1,
		TotalDrift:    report.EffectiveDrift,
		ExpectedDrift: report.EffectiveDrift - 1,
		Guidance:      []string{},
	}

	// Determine if target is an invariant/ADR (Path A) or other (Path B)
	isInvariant := strings.HasPrefix(target, "INV-") || strings.Contains(target, "-INV-")
	isADR := strings.HasPrefix(target, "ADR-") || strings.Contains(target, "-ADR-")

	if isInvariant || isADR {
		// Path A: target exists as a known element type
		remediateKnownElement(db, specID, lsi, pkg, target, isADR)
	} else {
		// Path B: search for related elements and use proxy
		remediateUnknownElement(db, specID, lsi, pkg, target, location)
	}

	// Generate guidance based on drift type
	pkg.Guidance = generateGuidance(driftType, target, location, isInvariant || isADR)

	return pkg, nil
}

// pickTarget selects the highest-priority drift item.
// Priority: correctness (unimplemented) > coherence > depth (unspecified).
func pickTarget(report *DriftReport) (target, driftType, location string) {
	// Correctness first (unimplemented)
	for _, d := range report.ImplDrift.Details {
		if d.Type == "unimplemented" {
			return d.Element, "correctness", d.Location
		}
	}
	// Coherence second
	for _, d := range report.ImplDrift.Details {
		if d.Type == "coherence" {
			return d.Element, "coherence", d.Location
		}
	}
	// Depth last (unspecified)
	for _, d := range report.ImplDrift.Details {
		if d.Type == "unspecified" {
			return d.Element, "depth", d.Location
		}
	}
	return "", "", ""
}

// remediateKnownElement handles Path A: target is an invariant or ADR.
func remediateKnownElement(db *sql.DB, specID int64, lsi *search.LSIIndex, pkg *RemediationPackage, target string, isADR bool) {
	// Try to get element title
	if isADR {
		adr, err := storage.GetADR(db, specID, target)
		if err == nil && adr != nil {
			pkg.Title = adr.Title
		}
	} else {
		inv, err := storage.GetInvariant(db, specID, target)
		if err == nil && inv != nil {
			pkg.Title = inv.Title
		}
	}

	// Try exemplar analysis
	if lsi != nil {
		exemplarResult, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
			Target:   target,
			Limit:    3,
			MinScore: 0.3,
		})
		if err == nil {
			pkg.Exemplars = exemplarResult
		}
	}

	// Try context bundle
	if lsi != nil {
		ctx, err := search.BuildContext(db, specID, target, lsi, "", 2, 5)
		if err == nil {
			pkg.Context = ctx
		}
	}
}

// remediateUnknownElement handles Path B: target is not a formal element.
// Searches by name, finds a proxy element for exemplar/context.
func remediateUnknownElement(db *sql.DB, specID int64, lsi *search.LSIIndex, pkg *RemediationPackage, target, location string) {
	pkg.Title = target

	// Search for related elements by name
	results, err := storage.SearchFTS5(db, target, 5)
	if err != nil || len(results) == 0 {
		return
	}

	// Use the best match as a proxy for exemplar/context
	proxy := results[0]
	proxyID := proxy.ElementID

	if lsi != nil {
		exemplarResult, err := exemplar.Analyze(db, specID, lsi, exemplar.Options{
			Target:   proxyID,
			Limit:    3,
			MinScore: 0.3,
		})
		if err == nil {
			pkg.Exemplars = exemplarResult
		}

		ctx, err := search.BuildContext(db, specID, proxyID, lsi, "", 2, 5)
		if err == nil {
			pkg.Context = ctx
		}
	}
}

// generateGuidance produces step-by-step remediation instructions.
func generateGuidance(driftType, target, location string, isKnownElement bool) []string {
	var guidance []string

	switch driftType {
	case "correctness":
		if isKnownElement {
			guidance = append(guidance,
				fmt.Sprintf("Write the full definition for %s in module %s", target, location),
				"Include all required components (statement, semi-formal, violation, validation, why)",
				"Add cross-references to related invariants and ADRs",
			)
		} else {
			guidance = append(guidance,
				fmt.Sprintf("Implement %s as specified in the spec", target),
				"Verify the implementation matches all relevant invariants",
			)
		}
	case "coherence":
		guidance = append(guidance,
			fmt.Sprintf("Resolve the cross-reference to %s", target),
			"Either define the target element or update the reference to point to the correct element",
			"Run `ddis validate` to check for remaining unresolved references",
		)
	case "depth":
		guidance = append(guidance,
			fmt.Sprintf("Add %s to the spec's element specifications", target),
			fmt.Sprintf("Write an invariant following the pattern of existing invariants in %s", location),
			"Add cross-references to INV-003 (falsifiability) and INV-007 (signal-to-noise)",
		)
	}

	guidance = append(guidance,
		fmt.Sprintf("After writing: ddis parse && ddis drift (expect drift to decrease by 1)"),
	)

	return guidance
}
