package drift

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"sort"
	"strings"

	"github.com/wvandaal/ddis/internal/consistency"
	"github.com/wvandaal/ddis/internal/state"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:implements APP-ADR-012 (annotations over code manifest)

// Options controls drift analysis behavior.
type Options struct {
	AsJSON  bool
	Report  bool // --report flag: full summary mode
	Intent  bool // --intent flag: include intent drift measurement
}

// DriftReport holds the complete drift analysis output.
type DriftReport struct {
	ImplDrift          ImplDrift         `json:"impl_drift"`
	IntentDrift        IntentDrift       `json:"intent_drift"`
	PlannedDivergences int               `json:"planned_divergences"`
	EffectiveDrift     int               `json:"effective_drift"`
	Classification     Classification    `json:"classification"`
	QualityBreakdown   QualityBreakdown  `json:"quality_breakdown"`
	StaleWitnesses     int               `json:"stale_witnesses"`
	ProcessDrift       float64           `json:"process_drift,omitempty"`
	ProcessDetails     *ProcessDriftInfo `json:"process_details,omitempty"`
}

// ProcessDriftInfo captures methodology compliance metrics.
type ProcessDriftInfo struct {
	SpecFirstRatio     float64  `json:"spec_first_ratio"`
	ToolIntermediation float64  `json:"tool_intermediation"`
	WitnessCoverage    float64  `json:"witness_coverage"`
	ValidateGating     float64  `json:"validate_gating"`
	Degraded           []string `json:"degraded,omitempty"`
}

// QualityBreakdown decomposes drift by quality dimension.
// Guides remediation: correctness -> fix impl, depth -> formalize spec, coherence -> repair spec.
type QualityBreakdown struct {
	Correctness int `json:"correctness"` // unimplemented + contradictions
	Depth       int `json:"depth"`       // unspecified
	Coherence   int `json:"coherence"`   // orphan cross-refs, declaration gaps
}

// ImplDrift measures spec-implementation drift.
type ImplDrift struct {
	Unspecified    int           `json:"unspecified"`
	Unimplemented  int           `json:"unimplemented"`
	Contradictions int           `json:"contradictions"`
	Total          int           `json:"total"`
	Details        []DriftDetail `json:"details"`
}

// IntentDrift measures intent-specification drift.
type IntentDrift struct {
	UncoveredNonnegotiables int            `json:"uncovered_nonnegotiables"`
	PurposelessElements     int            `json:"purposeless_elements"`
	Total                   int            `json:"total"`
	Details                 []IntentDetail `json:"details"`
}

// Classification categorizes drift along three dimensions.
type Classification struct {
	Direction      string `json:"direction"`      // impl-ahead | spec-ahead | contradictory | mutual
	Severity       string `json:"severity"`       // additive | contradictory | structural
	Intentionality string `json:"intentionality"` // planned | accidental | organic
}

// DriftDetail describes a single drift instance.
type DriftDetail struct {
	Element  string `json:"element"`
	Type     string `json:"type"`     // unspecified | unimplemented | contradiction
	Location string `json:"location"` // module or file where it was found
}

// IntentDetail describes a single intent drift instance.
type IntentDetail struct {
	Nonnegotiable string `json:"nonnegotiable"`
	Status        string `json:"status"` // uncovered | weak
}

// PlannedDivergence represents a tracked intentional divergence.
type PlannedDivergence struct {
	Element string `json:"element"`
	Type    string `json:"type"`
	Reason  string `json:"reason"`
	Expiry  string `json:"expiry"`
}

// RemediationPackage is the actionable output of drift's default mode.
type RemediationPackage struct {
	Target        string      `json:"target"`
	Title         string      `json:"title"`
	DriftType     string      `json:"drift_type"`
	Priority      int         `json:"priority"`
	TotalDrift    int         `json:"total_drift"`
	Context       any `json:"context,omitempty"`
	Exemplars     any `json:"exemplars,omitempty"`
	Guidance      []string    `json:"guidance"`
	ExpectedDrift int         `json:"expected_drift"`
}

// Analyze computes the drift report for a spec.
func Analyze(db *sql.DB, specID int64, opts Options) (*DriftReport, error) {
	// 1. Load declared elements from invariant registry
	registry, err := storage.GetInvariantRegistryEntries(db, specID)
	if err != nil {
		return nil, fmt.Errorf("get registry: %w", err)
	}
	registeredIDs := make(map[string]storage.InvariantRegistryEntry)
	for _, r := range registry {
		registeredIDs[r.InvariantID] = r
	}

	// 2. Load actual invariant definitions
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, fmt.Errorf("list invariants: %w", err)
	}
	definedInvIDs := make(map[string]bool)
	for _, inv := range invs {
		definedInvIDs[inv.InvariantID] = true
	}

	// 3. Load ADR registry and definitions
	adrs, err := storage.ListADRs(db, specID)
	if err != nil {
		return nil, fmt.Errorf("list adrs: %w", err)
	}
	definedADRIDs := make(map[string]bool)
	for _, adr := range adrs {
		definedADRIDs[adr.ADRID] = true
	}

	// 4. Load module relationships to find expected elements
	modules, err := storage.ListModules(db, specID)
	if err != nil {
		return nil, fmt.Errorf("list modules: %w", err)
	}
	moduleByID := make(map[int64]string)
	for _, m := range modules {
		moduleByID[m.ID] = m.ModuleName
	}

	rels, err := storage.GetModuleRelationships(db, specID)
	if err != nil {
		return nil, fmt.Errorf("get relationships: %w", err)
	}

	// Build sets of what modules claim to maintain/implement
	maintainedInvs := make(map[string]string)  // inv ID -> module name
	implementedADRs := make(map[string]string) // adr ID -> module name
	interfacedInvs := make(map[string]string)  // inv ID -> module name
	for _, r := range rels {
		modName := moduleByID[r.ModuleID]
		if modName == "" {
			continue
		}
		switch r.RelType {
		case "maintains":
			if strings.HasPrefix(r.Target, "INV-") || strings.Contains(r.Target, "-INV-") {
				maintainedInvs[r.Target] = modName
			}
		case "implements":
			if strings.HasPrefix(r.Target, "ADR-") || strings.Contains(r.Target, "-ADR-") {
				implementedADRs[r.Target] = modName
			}
		case "interfaces":
			if strings.HasPrefix(r.Target, "INV-") || strings.Contains(r.Target, "-INV-") {
				interfacedInvs[r.Target] = modName
			}
		}
	}

	// 5. Detect drift
	var details []DriftDetail
	unspecified := 0
	unimplemented := 0
	contradictions := 0

	// 5a. Unimplemented: registry entries without definitions
	for invID, regEntry := range registeredIDs {
		if !definedInvIDs[invID] {
			unimplemented++
			details = append(details, DriftDetail{
				Element:  invID,
				Type:     "unimplemented",
				Location: regEntry.Owner,
			})
		}
	}

	// 5b. Unspecified: maintained invariants not in registry
	for invID, modName := range maintainedInvs {
		if _, inRegistry := registeredIDs[invID]; !inRegistry {
			unspecified++
			details = append(details, DriftDetail{
				Element:  invID,
				Type:     "unspecified",
				Location: modName,
			})
		}
	}

	// 5c. Cross-spec: if parent spec exists, check interface requirements
	parentSpecID, err := storage.GetParentSpecID(db, specID)
	if err == nil && parentSpecID != nil {
		parentRegistry, err := storage.GetInvariantRegistryEntries(db, *parentSpecID)
		if err == nil {
			// Parent interface invariants that child should address
			for _, parentEntry := range parentRegistry {
				if _, interfaced := interfacedInvs[parentEntry.InvariantID]; interfaced {
					continue // child addresses it
				}
				if _, maintained := maintainedInvs[parentEntry.InvariantID]; maintained {
					continue // child maintains it
				}
				if definedInvIDs[parentEntry.InvariantID] {
					continue // child defines it
				}
				// Skip — parent elements aren't automatically required in child
				// Only flag if child's manifest references them
			}
		}
	}

	// 5d. Contradictions: integrate consistency checker (APP-INV-106)
	// ddis:implements APP-INV-106 (drift contradiction integration)
	// Run tiers 2-4 (graph, SAT, heuristic) — skip LLM tiers for performance.
	conResult, conErr := consistency.Analyze(db, specID, consistency.Options{MaxTier: consistency.TierHeuristic})
	if conErr == nil && conResult != nil {
		contradictions = len(conResult.Contradictions)
		for i, c := range conResult.Contradictions {
			if i >= 10 {
				break // Cap at 10 contradiction details
			}
			details = append(details, DriftDetail{
				Element:  c.ElementA + " vs " + c.ElementB,
				Type:     "contradiction",
				Location: string(c.Type),
			})
		}
	}

	// 5e. Coherence: unresolved cross-references
	unresolvedRefs, err := storage.GetUnresolvedRefs(db, specID)
	if err != nil {
		unresolvedRefs = nil
	}
	coherenceGaps := len(unresolvedRefs)

	// Add coherence details (cap at 20 to avoid noise)
	for i, ref := range unresolvedRefs {
		if i >= 20 {
			break
		}
		details = append(details, DriftDetail{
			Element:  ref.RefTarget,
			Type:     "coherence",
			Location: fmt.Sprintf("line %d", ref.SourceLine),
		})
	}

	// Sort details deterministically
	sort.Slice(details, func(i, j int) bool {
		if details[i].Type != details[j].Type {
			return details[i].Type < details[j].Type
		}
		return details[i].Element < details[j].Element
	})

	implDrift := ImplDrift{
		Unspecified:    unspecified,
		Unimplemented:  unimplemented,
		Contradictions: contradictions,
		Total:          unspecified + unimplemented + 2*contradictions,
		Details:        details,
	}
	if implDrift.Details == nil {
		implDrift.Details = []DriftDetail{}
	}

	// 6. Intent drift (only if --intent flag)
	intentDrift := IntentDrift{
		Details: []IntentDetail{},
	}
	if opts.Intent {
		intentDrift, err = analyzeIntentDrift(db, specID)
		if err != nil {
			// Non-fatal: intent drift is best-effort
			intentDrift = IntentDrift{Details: []IntentDetail{}}
		}
	}

	// 7. Read planned divergences from session state
	plannedCount := 0
	plannedDivVal, err := state.Get(db, specID, "planned_divergences")
	if err == nil && plannedDivVal != "" {
		var planned []PlannedDivergence
		if json.Unmarshal([]byte(plannedDivVal), &planned) == nil {
			plannedCount = len(planned)
		}
	}

	// 7.5 Stale witnesses: query witness table for non-valid witnesses
	staleWitnessCount := 0
	if witnessRows, err := db.Query(
		`SELECT invariant_id FROM invariant_witnesses WHERE spec_id = ? AND status != 'valid'`, specID,
	); err == nil {
		defer witnessRows.Close()
		for witnessRows.Next() {
			var invID string
			if err := witnessRows.Scan(&invID); err == nil {
				staleWitnessCount++
				details = append(details, DriftDetail{
					Element:  invID,
					Type:     "stale_witness",
					Location: "invariant_witnesses",
				})
			}
		}
	}

	// Re-sort details after adding stale witnesses
	sort.Slice(details, func(i, j int) bool {
		if details[i].Type != details[j].Type {
			return details[i].Type < details[j].Type
		}
		return details[i].Element < details[j].Element
	})

	// 7.6 Process drift: compute witness coverage as process quality signal
	processDrift := 0.0
	var processDetails *ProcessDriftInfo
	{
		totalInvs, _ := storage.ListInvariants(db, specID)
		validIDs, _ := storage.ListValidWitnessIDs(db, specID)
		if len(totalInvs) > 0 {
			witCov := float64(len(validIDs)) / float64(len(totalInvs))
			processDrift = 1.0 - witCov
			processDetails = &ProcessDriftInfo{
				SpecFirstRatio:     0.5, // degraded: no git analysis in drift context
				ToolIntermediation: 0.5, // degraded: no oplog analysis in drift context
				WitnessCoverage:    witCov,
				ValidateGating:     0.5, // degraded: no oplog analysis in drift context
				Degraded:           []string{"spec_first_ratio", "tool_intermediation", "validate_gating"},
			}
		}
	}

	// 8. Compute effective drift and quality breakdown
	totalDrift := implDrift.Total + intentDrift.Total + staleWitnessCount
	effectiveDrift := totalDrift - plannedCount
	if effectiveDrift < 0 {
		effectiveDrift = 0
	}

	quality := QualityBreakdown{
		Correctness: unimplemented + contradictions + staleWitnessCount,
		Depth:       unspecified,
		Coherence:   coherenceGaps,
	}

	// 9. Classify
	report := &DriftReport{
		ImplDrift:          implDrift,
		IntentDrift:        intentDrift,
		PlannedDivergences: plannedCount,
		EffectiveDrift:     effectiveDrift,
		QualityBreakdown:   quality,
		StaleWitnesses:     staleWitnessCount,
		ProcessDrift:       processDrift,
		ProcessDetails:     processDetails,
	}
	report.Classification = Classify(report)

	return report, nil
}

// analyzeIntentDrift measures the gap between intent and specification.
func analyzeIntentDrift(db *sql.DB, specID int64) (IntentDrift, error) {
	result := IntentDrift{
		Details: []IntentDetail{},
	}

	// Non-negotiables are listed in section §0.1.2 of the system constitution.
	// We approximate by checking: for each section with "non-negotiable" in its path,
	// count how many of its cross-references are resolved.
	sections, err := storage.ListSections(db, specID)
	if err != nil {
		return result, err
	}

	// Find the non-negotiables section
	var nonNegSection *storage.Section
	for i, s := range sections {
		lower := strings.ToLower(s.Title)
		if strings.Contains(lower, "non-negotiable") {
			nonNegSection = &sections[i]
			break
		}
	}

	if nonNegSection == nil {
		return result, nil
	}

	// Extract non-negotiable items from the section text
	nonnegotiables := extractNonnegotiables(nonNegSection.RawText)

	// For each non-negotiable, check if at least one invariant addresses it
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return result, err
	}

	for _, nn := range nonnegotiables {
		covered := false
		nnLower := strings.ToLower(nn)
		for _, inv := range invs {
			invText := strings.ToLower(inv.Statement + " " + inv.Title)
			// Check for keyword overlap
			if hasSignificantOverlap(nnLower, invText) {
				covered = true
				break
			}
		}
		if !covered {
			result.UncoveredNonnegotiables++
			result.Details = append(result.Details, IntentDetail{
				Nonnegotiable: nn,
				Status:        "uncovered",
			})
		}
	}

	result.Total = result.UncoveredNonnegotiables + result.PurposelessElements
	return result, nil
}

// extractNonnegotiables parses non-negotiable items from section text.
func extractNonnegotiables(text string) []string {
	var items []string
	lines := strings.Split(text, "\n")
	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		// Non-negotiables are typically bold items starting with **
		if strings.HasPrefix(trimmed, "- **") || strings.HasPrefix(trimmed, "**") {
			// Extract the bold text
			start := strings.Index(trimmed, "**")
			if start >= 0 {
				rest := trimmed[start+2:]
				end := strings.Index(rest, "**")
				if end > 0 {
					items = append(items, rest[:end])
				}
			}
		}
	}
	return items
}

// hasSignificantOverlap checks if two strings share meaningful keywords.
func hasSignificantOverlap(a, b string) bool {
	stopWords := map[string]bool{
		"the": true, "a": true, "an": true, "is": true, "are": true,
		"was": true, "be": true, "to": true, "of": true, "and": true,
		"in": true, "that": true, "it": true, "for": true, "on": true,
		"with": true, "as": true, "at": true, "by": true, "or": true,
		"not": true, "must": true, "every": true, "all": true, "no": true,
	}

	wordsA := strings.Fields(a)
	keywordsA := make(map[string]bool)
	for _, w := range wordsA {
		w = strings.Trim(w, ".,;:!?()[]{}\"'")
		if len(w) > 2 && !stopWords[w] {
			keywordsA[w] = true
		}
	}

	matches := 0
	wordsB := strings.Fields(b)
	for _, w := range wordsB {
		w = strings.Trim(w, ".,;:!?()[]{}\"'")
		if keywordsA[w] {
			matches++
		}
	}

	// Require at least 2 keyword matches or 30% overlap
	threshold := len(keywordsA) * 3 / 10
	if threshold < 2 {
		threshold = 2
	}
	return matches >= threshold
}
