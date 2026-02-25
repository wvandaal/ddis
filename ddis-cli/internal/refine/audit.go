package refine

// ddis:implements APP-ADR-022 (state monad architecture)
// ddis:maintains APP-INV-022 (refinement drift monotonicity)

import (
	"database/sql"
	"fmt"
	"strconv"
	"strings"

	"github.com/wvandaal/ddis/internal/autoprompt"
	"github.com/wvandaal/ddis/internal/drift"
	"github.com/wvandaal/ddis/internal/exemplar"
	"github.com/wvandaal/ddis/internal/state"
	"github.com/wvandaal/ddis/internal/storage"
)

// specInternalDrift holds computed drift metrics derived from direct DB queries
// against spec elements (invariants, ADRs, cross-references).
type specInternalDrift struct {
	UnresolvedRefs        int
	MissingViolation      int
	MissingSemiFormal     int
	MissingADRTests       int
	WeakChosenOption      int
	TotalRefs             int
	TotalInvariants       int
	TotalADRs             int
	CompleteInvariants    int // all 5 components present and non-empty
	CompleteADRs          int // all 5 components present and non-empty
	TotalElements         int // invariants + ADRs
	CompleteElements      int // those with all components filled
	InvariantsWithFormal  int // those with non-empty semi_formal
}

// finding captures a single quality gap for reporting.
type finding struct {
	ElementID string
	Component string
	Detail    string
}

// computeSpecInternalDrift queries the DB to compute per-dimension quality metrics.
func computeSpecInternalDrift(db *sql.DB, specID int64) (*specInternalDrift, []finding, error) {
	sid := &specInternalDrift{}
	var findings []finding

	// Unresolved cross-references
	err := db.QueryRow(
		`SELECT COUNT(*) FROM cross_references WHERE spec_id = ? AND resolved = 0`, specID,
	).Scan(&sid.UnresolvedRefs)
	if err != nil {
		return nil, nil, fmt.Errorf("count unresolved refs: %w", err)
	}

	// Total cross-references
	err = db.QueryRow(
		`SELECT COUNT(*) FROM cross_references WHERE spec_id = ?`, specID,
	).Scan(&sid.TotalRefs)
	if err != nil {
		return nil, nil, fmt.Errorf("count total refs: %w", err)
	}

	// Load all invariants for component analysis
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, nil, fmt.Errorf("list invariants: %w", err)
	}
	sid.TotalInvariants = len(invs)

	for _, inv := range invs {
		fields := exemplar.ExtractInvariantFields(inv)
		complete := true
		for _, comp := range exemplar.ComponentsForType("invariant") {
			if fields[comp] == "" {
				complete = false
				switch comp {
				case "violation_scenario":
					sid.MissingViolation++
					findings = append(findings, finding{
						ElementID: inv.InvariantID,
						Component: comp,
						Detail:    "missing violation scenario",
					})
				case "semi_formal":
					sid.MissingSemiFormal++
					findings = append(findings, finding{
						ElementID: inv.InvariantID,
						Component: comp,
						Detail:    "missing semi-formal predicate",
					})
				}
			}
		}
		if fields["semi_formal"] != "" {
			sid.InvariantsWithFormal++
		}
		if complete {
			sid.CompleteInvariants++
		}
	}

	// Load all ADRs for component analysis
	adrs, err := storage.ListADRs(db, specID)
	if err != nil {
		return nil, nil, fmt.Errorf("list adrs: %w", err)
	}
	sid.TotalADRs = len(adrs)

	for _, adr := range adrs {
		fields := exemplar.ExtractADRFields(adr)
		complete := true
		for _, comp := range exemplar.ComponentsForType("adr") {
			if fields[comp] == "" {
				complete = false
				switch comp {
				case "tests":
					sid.MissingADRTests++
					findings = append(findings, finding{
						ElementID: adr.ADRID,
						Component: comp,
						Detail:    "missing tests",
					})
				case "chosen_option":
					sid.WeakChosenOption++
					findings = append(findings, finding{
						ElementID: adr.ADRID,
						Component: comp,
						Detail:    "missing chosen option",
					})
				}
			}
		}
		if fields["chosen_option"] != "" && len(fields["chosen_option"]) < 20 {
			sid.WeakChosenOption++
			findings = append(findings, finding{
				ElementID: adr.ADRID,
				Component: "chosen_option",
				Detail:    fmt.Sprintf("chosen option too short (%d chars)", len(fields["chosen_option"])),
			})
		}
		if complete {
			sid.CompleteADRs++
		}
	}

	sid.TotalElements = sid.TotalInvariants + sid.TotalADRs
	sid.CompleteElements = sid.CompleteInvariants + sid.CompleteADRs

	return sid, findings, nil
}

// deriveConfidence computes the 5-dimensional confidence array from spec metrics.
// Each dimension scores 0-10.
func deriveConfidence(sid *specInternalDrift) [5]int {
	var conf [5]int

	// Coverage: based on component coverage % (complete elements / total elements)
	if sid.TotalElements > 0 {
		conf[autoprompt.ConfCoverage] = clampScore(10 * sid.CompleteElements / sid.TotalElements)
	} else {
		conf[autoprompt.ConfCoverage] = 10 // vacuously covered
	}

	// Depth: based on how many invariants have full components
	// (statement + semi_formal + violation + validation + why_this_matters)
	if sid.TotalInvariants > 0 {
		conf[autoprompt.ConfDepth] = clampScore(10 * sid.CompleteInvariants / sid.TotalInvariants)
	} else {
		conf[autoprompt.ConfDepth] = 10
	}

	// Coherence: based on ratio of resolved xrefs to total xrefs
	if sid.TotalRefs > 0 {
		resolved := sid.TotalRefs - sid.UnresolvedRefs
		conf[autoprompt.ConfCoherence] = clampScore(10 * resolved / sid.TotalRefs)
	} else {
		conf[autoprompt.ConfCoherence] = 10
	}

	// Completeness: inverse of missing components as fraction of total possible
	totalComponents := sid.TotalInvariants*5 + sid.TotalADRs*5
	missingComponents := sid.MissingViolation + sid.MissingSemiFormal + sid.MissingADRTests + sid.WeakChosenOption
	if totalComponents > 0 {
		presentRatio := float64(totalComponents-missingComponents) / float64(totalComponents)
		conf[autoprompt.ConfCompleteness] = clampScore(int(presentRatio * 10))
	} else {
		conf[autoprompt.ConfCompleteness] = 10
	}

	// Formality: based on ratio of invariants with semi_formal predicates
	if sid.TotalInvariants > 0 {
		conf[autoprompt.ConfFormality] = clampScore(10 * sid.InvariantsWithFormal / sid.TotalInvariants)
	} else {
		conf[autoprompt.ConfFormality] = 10
	}

	return conf
}

// selectLimitingFactor returns the dimension name with the lowest confidence score.
// Tie-break uses DimensionPriority: completeness > coherence > depth > coverage > formality.
func selectLimitingFactor(conf [5]int) string {
	minVal := 11 // sentinel above max
	minIdx := 0

	// Walk in DimensionPriority order so the first encountered minimum wins ties
	for _, idx := range autoprompt.DimensionPriority {
		if conf[idx] < minVal {
			minVal = conf[idx]
			minIdx = idx
		}
	}
	return autoprompt.DimensionNames[minIdx]
}

// clampScore constrains a score to [0,10].
func clampScore(v int) int {
	if v < 0 {
		return 0
	}
	if v > 10 {
		return 10
	}
	return v
}

// Audit generates a diagnostic report combining drift, validation, and coverage data.
// It identifies the limiting quality dimension and selects relevant exemplars.
func Audit(db *sql.DB, specID int64, iteration int) (*autoprompt.CommandResult, error) {
	// 1. Get drift score from drift.Analyze
	driftReport, err := drift.Analyze(db, specID, drift.Options{})
	if err != nil {
		return nil, fmt.Errorf("drift analysis: %w", err)
	}

	// 2. Compute spec-internal metrics from DB
	sid, findings, err := computeSpecInternalDrift(db, specID)
	if err != nil {
		return nil, fmt.Errorf("spec internal drift: %w", err)
	}

	// 3. Derive confidence array
	conf := deriveConfidence(sid)

	// 4. Identify limiting factor
	limiting := selectLimitingFactor(conf)

	// 5. Build human-readable audit report
	var report strings.Builder
	report.WriteString("=== RALPH Audit Report ===\n\n")
	fmt.Fprintf(&report, "Iteration: %d\n", iteration)
	fmt.Fprintf(&report, "Effective drift: %d\n", driftReport.EffectiveDrift)
	fmt.Fprintf(&report, "Quality breakdown: correctness=%d, depth=%d, coherence=%d\n",
		driftReport.QualityBreakdown.Correctness,
		driftReport.QualityBreakdown.Depth,
		driftReport.QualityBreakdown.Coherence)
	report.WriteString("\nConfidence scores (0-10):\n")
	for i, name := range autoprompt.DimensionNames {
		marker := ""
		if name == limiting {
			marker = " <-- LIMITING"
		}
		fmt.Fprintf(&report, "  %s: %d%s\n", name, conf[i], marker)
	}
	fmt.Fprintf(&report, "\nLimiting factor: %s\n", limiting)

	if len(findings) > 0 {
		report.WriteString("\nFindings:\n")
		cap := 20
		if len(findings) < cap {
			cap = len(findings)
		}
		for _, f := range findings[:cap] {
			fmt.Fprintf(&report, "  - %s: %s\n", f.ElementID, f.Detail)
		}
		if len(findings) > 20 {
			fmt.Fprintf(&report, "  ... and %d more\n", len(findings)-20)
		}
	}

	// 6. Collect relevant element IDs for context
	var contextIDs []string
	seen := make(map[string]bool)
	for _, f := range findings {
		if !seen[f.ElementID] {
			contextIDs = append(contextIDs, f.ElementID)
			seen[f.ElementID] = true
		}
		if len(contextIDs) >= 10 {
			break
		}
	}

	// 7. Store audit state for downstream stages
	driftScore := driftReport.EffectiveDrift
	_ = state.Set(db, specID, "refine_drift_"+strconv.Itoa(iteration), strconv.Itoa(driftScore))
	_ = state.Set(db, specID, "refine_limiting_"+strconv.Itoa(iteration), limiting)
	confParts := make([]string, 5)
	for i, c := range conf {
		confParts[i] = strconv.Itoa(c)
	}
	_ = state.Set(db, specID, "refine_confidence_"+strconv.Itoa(iteration), strings.Join(confParts, ","))

	// 8. Build CommandResult
	attenuation := autoprompt.Attenuation(iteration)

	return &autoprompt.CommandResult{
		Output: report.String(),
		State: autoprompt.StateSnapshot{
			ActiveThread:     "refine",
			Confidence:       conf,
			LimitingFactor:   limiting,
			SpecDrift:        float64(driftScore),
			Iteration:        iteration,
			ArtifactsWritten: 0,
			ModeObserved:     "convergent",
		},
		Guidance: autoprompt.Guidance{
			ObservedMode:    "convergent",
			DoFHint:         "low",
			SuggestedNext:   []string{fmt.Sprintf("Run 'ddis refine plan' to focus on %s dimension", limiting)},
			RelevantContext: contextIDs,
			Attenuation:     attenuation,
		},
	}, nil
}
