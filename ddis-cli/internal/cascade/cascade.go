package cascade

import (
	"database/sql"
	"fmt"
	"sort"

	"github.com/wvandaal/ddis/internal/storage"
)

// Options controls cascade analysis behavior.
type Options struct {
	Depth  int
	AsJSON bool
}

// CascadeResult holds the complete cascade analysis output.
type CascadeResult struct {
	ChangedElement  string           `json:"changed_element"`
	ElementType     string           `json:"element_type"`
	Title           string           `json:"title"`
	OwnerModule     string           `json:"owner_module"`
	OwnerDomain     string           `json:"owner_domain"`
	AffectedModules []AffectedModule `json:"affected_modules"`
	AffectedDomains []string         `json:"affected_domains"`
	TotalReferences int              `json:"total_references"`
	Summary         string           `json:"summary"`
}

// AffectedModule represents a module affected by the cascade.
type AffectedModule struct {
	Module       string `json:"module"`
	Domain       string `json:"domain"`
	Relationship string `json:"relationship"`
}

// Analyze performs cascade analysis: given an element ID, find all modules and
// domains that would be affected if that element changes.
func Analyze(db *sql.DB, specID int64, elementID string, opts Options) (*CascadeResult, error) {
	depth := opts.Depth
	if depth <= 0 {
		depth = 3
	}
	if depth > 5 {
		depth = 5
	}
	_ = depth // reserved for future multi-hop cascade

	result := &CascadeResult{ChangedElement: elementID}

	// 1. Determine element type and title
	inv, err := storage.GetInvariant(db, specID, elementID)
	if err == nil && inv != nil {
		result.ElementType = "invariant"
		result.Title = inv.Title
	} else {
		adr, err := storage.GetADR(db, specID, elementID)
		if err == nil && adr != nil {
			result.ElementType = "adr"
			result.Title = adr.Title
		} else {
			gate, err := storage.GetQualityGate(db, specID, elementID)
			if err == nil && gate != nil {
				result.ElementType = "gate"
				result.Title = gate.Title
			} else {
				return nil, fmt.Errorf("element %q not found as invariant, ADR, or gate in spec", elementID)
			}
		}
	}

	// 2. Find owner from invariant_registry
	registry, _ := storage.GetInvariantRegistryEntries(db, specID)
	for _, r := range registry {
		if r.InvariantID == elementID {
			result.OwnerModule = r.Owner
			result.OwnerDomain = r.Domain
			break
		}
	}

	// 3. Build module name → domain map
	modules, _ := storage.ListModules(db, specID)
	moduleDomain := make(map[string]string)
	moduleByID := make(map[int64]string) // module DB id → name
	for _, m := range modules {
		moduleDomain[m.ModuleName] = m.Domain
		moduleByID[m.ID] = m.ModuleName
	}

	// 4. Find affected modules via module_relationships
	rels, _ := storage.GetModuleRelationships(db, specID)
	domainSet := make(map[string]bool)
	seen := make(map[string]bool) // dedup by module name
	for _, r := range rels {
		if r.Target != elementID {
			continue
		}
		moduleName := moduleByID[r.ModuleID]
		if moduleName == "" || r.RelType == "maintains" {
			continue
		}
		if seen[moduleName] {
			continue
		}
		seen[moduleName] = true
		domain := moduleDomain[moduleName]
		result.AffectedModules = append(result.AffectedModules, AffectedModule{
			Module:       moduleName,
			Domain:       domain,
			Relationship: r.RelType + " with " + elementID,
		})
		domainSet[domain] = true
	}

	// 5. Count cross-references (backlinks) for total reference count
	backlinks, _ := storage.GetBacklinks(db, specID, elementID)
	result.TotalReferences = len(backlinks)

	// 6. Build affected domains list
	for d := range domainSet {
		if d != "" {
			result.AffectedDomains = append(result.AffectedDomains, d)
		}
	}
	sort.Strings(result.AffectedDomains)

	// 7. Build summary
	allDomains, _ := storage.GetModuleDomains(db, specID)
	totalDomainCount := len(allDomains)
	domainSuffix := ""
	if totalDomainCount > 0 {
		domainSuffix = fmt.Sprintf(" of %d", totalDomainCount)
	}
	result.Summary = fmt.Sprintf("%d total references. %d module(s) across %d%s domain(s) need revalidation.",
		result.TotalReferences, len(result.AffectedModules), len(result.AffectedDomains), domainSuffix)

	// Ensure non-nil slices for JSON
	if result.AffectedModules == nil {
		result.AffectedModules = []AffectedModule{}
	}
	if result.AffectedDomains == nil {
		result.AffectedDomains = []string{}
	}

	return result, nil
}
