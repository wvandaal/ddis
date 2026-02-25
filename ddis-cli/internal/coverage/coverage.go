package coverage

// ddis:maintains APP-INV-040 (progressive validation monotonicity)

import (
	"database/sql"
	"fmt"

	"github.com/wvandaal/ddis/internal/exemplar"
	"github.com/wvandaal/ddis/internal/storage"
)

// Options configures a coverage analysis run.
type Options struct {
	Domain string
	Module string
	AsJSON bool
}

// CoverageResult is the top-level output of a coverage analysis.
type CoverageResult struct {
	Spec       string                      `json:"spec"`
	Summary    CoverageSummary             `json:"summary"`
	Invariants map[string]InvariantCoverage `json:"invariants"`
	ADRs       map[string]ADRCoverage      `json:"adrs,omitempty"`
	Modules    map[string]ModuleCoverage   `json:"modules,omitempty"`
	Domains    map[string]DomainCoverage   `json:"domains,omitempty"`
	Gaps       []string                    `json:"gaps"`
}

// CoverageSummary holds aggregate coverage statistics.
type CoverageSummary struct {
	Score              float64 `json:"score"`
	InvariantsComplete int     `json:"invariants_complete"`
	InvariantsTotal    int     `json:"invariants_total"`
	ADRsComplete       int     `json:"adrs_complete"`
	ADRsTotal          int     `json:"adrs_total"`
}

// InvariantCoverage holds per-invariant component coverage.
type InvariantCoverage struct {
	Title        string                     `json:"title"`
	Components   map[string]ComponentStatus `json:"components"`
	Completeness float64                    `json:"completeness"`
}

// ADRCoverage holds per-ADR component coverage.
type ADRCoverage struct {
	Title        string                     `json:"title"`
	Components   map[string]ComponentStatus `json:"components"`
	Completeness float64                    `json:"completeness"`
}

// ModuleCoverage holds per-module coverage aggregation.
type ModuleCoverage struct {
	InvariantsMaintained int     `json:"invariants_maintained"`
	Complete             int     `json:"complete"`
	Coverage             float64 `json:"coverage"`
}

// DomainCoverage holds per-domain coverage aggregation.
type DomainCoverage struct {
	Modules    int     `json:"modules"`
	Invariants int     `json:"invariants"`
	Coverage   float64 `json:"coverage"`
}

// ComponentStatus holds a single component's presence and quality score.
type ComponentStatus struct {
	Present bool    `json:"present"`
	Score   float64 `json:"score"`
}

// Analyze computes coverage for a spec.
func Analyze(db *sql.DB, specID int64, opts Options) (*CoverageResult, error) {
	spec, err := storage.GetSpecIndex(db, specID)
	if err != nil {
		return nil, fmt.Errorf("get spec: %w", err)
	}

	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, fmt.Errorf("list invariants: %w", err)
	}

	adrs, err := storage.ListADRs(db, specID)
	if err != nil {
		return nil, fmt.Errorf("list adrs: %w", err)
	}

	result := &CoverageResult{
		Spec:       spec.SpecName,
		Invariants: make(map[string]InvariantCoverage),
		ADRs:       make(map[string]ADRCoverage),
		Modules:    make(map[string]ModuleCoverage),
		Domains:    make(map[string]DomainCoverage),
		Gaps:       []string{},
	}

	// Build domain filter set from registry if domain filter is specified
	registry, _ := storage.GetInvariantRegistryEntries(db, specID)
	domainForInv := make(map[string]string) // invariantID → domain
	for _, r := range registry {
		domainForInv[r.InvariantID] = r.Domain
	}

	invComplete := 0
	for _, inv := range invs {
		// Apply domain filter
		if opts.Domain != "" {
			if domainForInv[inv.InvariantID] != opts.Domain {
				continue
			}
		}

		fields := exemplar.ExtractInvariantFields(inv)
		components := exemplar.ComponentsForType("invariant")
		ic := InvariantCoverage{
			Title:      inv.Title,
			Components: make(map[string]ComponentStatus),
		}
		presentCount := 0
		for _, comp := range components {
			val := fields[comp]
			score := 0.0
			present := false
			if val != "" {
				score = exemplar.WeakScore(val, comp, "invariant")
				present = score > 0
			}
			if present {
				presentCount++
			}
			ic.Components[comp] = ComponentStatus{Present: present, Score: score}
			if !present {
				result.Gaps = append(result.Gaps, inv.InvariantID+":"+comp+" MISSING")
			} else if score < 0.6 {
				result.Gaps = append(result.Gaps, fmt.Sprintf("%s:%s WEAK (score: %.2f)", inv.InvariantID, comp, score))
			}
		}
		if len(components) > 0 {
			ic.Completeness = float64(presentCount) / float64(len(components))
		}
		if ic.Completeness >= 1.0 {
			invComplete++
		}
		result.Invariants[inv.InvariantID] = ic
	}

	adrComplete := 0
	for _, adr := range adrs {
		fields := exemplar.ExtractADRFields(adr)
		components := exemplar.ComponentsForType("adr")
		ac := ADRCoverage{
			Title:      adr.Title,
			Components: make(map[string]ComponentStatus),
		}
		presentCount := 0
		for _, comp := range components {
			val := fields[comp]
			score := 0.0
			present := false
			if val != "" {
				score = exemplar.WeakScore(val, comp, "adr")
				present = score > 0
			}
			if present {
				presentCount++
			}
			ac.Components[comp] = ComponentStatus{Present: present, Score: score}
			if !present {
				result.Gaps = append(result.Gaps, adr.ADRID+":"+comp+" MISSING")
			} else if score < 0.6 {
				result.Gaps = append(result.Gaps, fmt.Sprintf("%s:%s WEAK (score: %.2f)", adr.ADRID, comp, score))
			}
		}
		if len(components) > 0 {
			ac.Completeness = float64(presentCount) / float64(len(components))
		}
		if ac.Completeness >= 1.0 {
			adrComplete++
		}
		result.ADRs[adr.ADRID] = ac
	}

	// Summary
	total := len(result.Invariants) + len(result.ADRs)
	complete := invComplete + adrComplete
	score := 0.0
	if total > 0 {
		score = float64(complete) / float64(total)
	}
	result.Summary = CoverageSummary{
		Score:              score,
		InvariantsComplete: invComplete,
		InvariantsTotal:    len(result.Invariants),
		ADRsComplete:       adrComplete,
		ADRsTotal:          len(result.ADRs),
	}

	// Module aggregation
	modules, _ := storage.ListModules(db, specID)
	rels, _ := storage.GetModuleRelationships(db, specID)

	// Build moduleID → moduleName map
	moduleNames := make(map[int64]string)
	for _, m := range modules {
		moduleNames[m.ID] = m.ModuleName
	}

	moduleInvCount := make(map[string]int)
	moduleCompleteCount := make(map[string]int)
	for _, r := range rels {
		if r.RelType == "maintains" {
			modName := moduleNames[r.ModuleID]
			if modName == "" {
				continue
			}
			// Apply module filter
			if opts.Module != "" && modName != opts.Module {
				continue
			}
			moduleInvCount[modName]++
			if ic, ok := result.Invariants[r.Target]; ok && ic.Completeness >= 1.0 {
				moduleCompleteCount[modName]++
			}
		}
	}

	for _, m := range modules {
		if opts.Module != "" && m.ModuleName != opts.Module {
			continue
		}
		total := moduleInvCount[m.ModuleName]
		complete := moduleCompleteCount[m.ModuleName]
		cov := 0.0
		if total > 0 {
			cov = float64(complete) / float64(total)
		}
		result.Modules[m.ModuleName] = ModuleCoverage{
			InvariantsMaintained: total,
			Complete:             complete,
			Coverage:             cov,
		}
	}

	// Domain aggregation from registry
	domainInvs := make(map[string][]string)
	for _, r := range registry {
		domainInvs[r.Domain] = append(domainInvs[r.Domain], r.InvariantID)
	}

	for domain, invIDs := range domainInvs {
		if opts.Domain != "" && domain != opts.Domain {
			continue
		}
		complete := 0
		for _, id := range invIDs {
			if ic, ok := result.Invariants[id]; ok && ic.Completeness >= 1.0 {
				complete++
			}
		}
		cov := 0.0
		if len(invIDs) > 0 {
			cov = float64(complete) / float64(len(invIDs))
		}
		modCount := 0
		for _, m := range modules {
			if m.Domain == domain {
				modCount++
			}
		}
		result.Domains[domain] = DomainCoverage{
			Modules:    modCount,
			Invariants: len(invIDs),
			Coverage:   cov,
		}
	}

	return result, nil
}
