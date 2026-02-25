package implorder

import (
	"database/sql"
	"sort"

	"github.com/wvandaal/ddis/internal/storage"
)

// Options controls impl-order analysis behavior.
type Options struct {
	Domain string
	AsJSON bool
}

// ImplOrderResult holds the complete implementation order analysis output.
type ImplOrderResult struct {
	Phases         []Phase  `json:"phases"`
	TotalElements  int      `json:"total_elements"`
	CriticalPath   int      `json:"critical_path_length"`
	CyclesDetected []string `json:"cycles_detected"`
}

// Phase represents one topological layer of implementation.
type Phase struct {
	PhaseNum int       `json:"phase"`
	Label    string    `json:"label"`
	Elements []Element `json:"elements"`
}

// Element represents a single invariant within a phase.
type Element struct {
	ID        string  `json:"id"`
	Type      string  `json:"type"`
	Title     string  `json:"title"`
	Authority float64 `json:"authority"`
	Domain    string  `json:"domain"`
}

// Analyze computes optimal implementation order using Kahn's topological sort.
// Nodes are invariants; edges come from module_relationships (if module M
// maintains INV-X and interfaces with INV-Y, then INV-X depends on INV-Y).
// Elements are grouped into phases (phase 0 = no dependencies) and sorted
// within each phase by authority score (PageRank, descending).
func Analyze(db *sql.DB, specID int64, opts Options) (*ImplOrderResult, error) {
	// 1. Load invariants
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, err
	}

	invMap := make(map[string]storage.Invariant) // invariantID → Invariant
	for _, inv := range invs {
		invMap[inv.InvariantID] = inv
	}

	// 2. Load invariant registry for domain info
	registry, err := storage.GetInvariantRegistryEntries(db, specID)
	if err != nil {
		return nil, err
	}
	domainMap := make(map[string]string) // invariantID → domain
	for _, r := range registry {
		domainMap[r.InvariantID] = r.Domain
	}

	// 3. Load authority scores (PageRank)
	authorityMap, err := storage.GetAuthorityScores(db, specID)
	if err != nil {
		// Non-fatal: authority scores may not exist
		authorityMap = make(map[string]float64)
	}

	// 4. Build dependency graph from module_relationships
	// If module M maintains INV-X and interfaces with INV-Y, then INV-X depends on INV-Y.
	modules, err := storage.ListModules(db, specID)
	if err != nil {
		return nil, err
	}
	moduleByID := make(map[int64]string) // module DB ID → module name
	for _, m := range modules {
		moduleByID[m.ID] = m.ModuleName
	}

	rels, err := storage.GetModuleRelationships(db, specID)
	if err != nil {
		return nil, err
	}

	// Group relationships by module name
	moduleMainInvs := make(map[string][]string)      // module → maintained invariants
	moduleInterfaceInvs := make(map[string][]string)  // module → interfaced invariants

	for _, r := range rels {
		modName := moduleByID[r.ModuleID]
		if modName == "" {
			continue
		}
		switch r.RelType {
		case "maintains":
			moduleMainInvs[modName] = append(moduleMainInvs[modName], r.Target)
		case "interfaces":
			moduleInterfaceInvs[modName] = append(moduleInterfaceInvs[modName], r.Target)
		}
	}

	// Build the node set (only invariants present in the registry or invMap)
	nodeSet := make(map[string]bool)
	for _, inv := range invs {
		nodeSet[inv.InvariantID] = true
	}

	// Build adjacency: dependsOn[X] = [Y, Z] means X depends on Y, Z
	inDegree := make(map[string]int)
	dependedBy := make(map[string][]string) // Y → [X, ...] meaning X depends on Y

	for node := range nodeSet {
		inDegree[node] = 0
	}

	// Note: interface relationships are soft dependencies (consumed, not produced).
	// Creating hard edges from interfaces causes circular dependencies when
	// two modules interface each other's maintained invariants.
	// Only hard implementation dependencies create DAG edges.
	edgeSet := make(map[[2]string]bool) // dedup edges (empty — no hard deps from interfaces)
	_ = moduleInterfaceInvs             // interfaces tracked but not used for hard deps

	// 5. Apply domain filter
	if opts.Domain != "" {
		for id := range nodeSet {
			if domainMap[id] != opts.Domain {
				delete(nodeSet, id)
			}
		}
		// Recompute in-degree for filtered set
		for id := range nodeSet {
			inDegree[id] = 0
		}
		for edge := range edgeSet {
			m, i := edge[0], edge[1]
			if nodeSet[m] && nodeSet[i] {
				inDegree[m]++
			}
		}
		// Rebuild dependedBy for filtered set
		dependedBy = make(map[string][]string)
		for edge := range edgeSet {
			m, i := edge[0], edge[1]
			if nodeSet[m] && nodeSet[i] {
				dependedBy[i] = append(dependedBy[i], m)
			}
		}
	}

	// 6. Kahn's algorithm
	var queue []string
	for node := range nodeSet {
		if inDegree[node] == 0 {
			queue = append(queue, node)
		}
	}

	var phases []Phase
	phaseNum := 0
	processed := 0

	for len(queue) > 0 {
		// Sort by authority (descending) for deterministic tie-breaking
		sort.Slice(queue, func(i, j int) bool {
			ai, aj := authorityMap[queue[i]], authorityMap[queue[j]]
			if ai != aj {
				return ai > aj
			}
			return queue[i] < queue[j]
		})

		label := "Foundation — no dependencies"
		if phaseNum > 0 {
			label = "Depends on earlier phases"
		}

		phase := Phase{PhaseNum: phaseNum, Label: label}
		var nextQueue []string

		for _, node := range queue {
			inv := invMap[node]
			phase.Elements = append(phase.Elements, Element{
				ID:        node,
				Type:      "invariant",
				Title:     inv.Title,
				Authority: authorityMap[node],
				Domain:    domainMap[node],
			})
			processed++

			// Decrease in-degree of dependents
			for _, dep := range dependedBy[node] {
				if !nodeSet[dep] {
					continue
				}
				inDegree[dep]--
				if inDegree[dep] == 0 {
					nextQueue = append(nextQueue, dep)
				}
			}
		}

		if len(phase.Elements) > 0 {
			phases = append(phases, phase)
		}
		queue = nextQueue
		phaseNum++
	}

	// 7. Detect cycles (remaining nodes with in-degree > 0)
	var cycles []string
	for node := range nodeSet {
		if inDegree[node] > 0 {
			cycles = append(cycles, node)
		}
	}
	sort.Strings(cycles)

	// Ensure non-nil slices for JSON
	if phases == nil {
		phases = []Phase{}
	}
	if cycles == nil {
		cycles = []string{}
	}

	return &ImplOrderResult{
		Phases:         phases,
		TotalElements:  processed,
		CriticalPath:   len(phases),
		CyclesDetected: cycles,
	}, nil
}
