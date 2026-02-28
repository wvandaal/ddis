package implorder

// ddis:implements APP-INV-112 (module-level dependency ordering)
// ddis:implements APP-ADR-080 (module-level DAG with SCC cycle breaking)

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

// Analyze computes optimal implementation order using module-level DAG.
// Modules are nodes; "adjacent" declarations form directed edges (A lists B
// means A depends on B). SCCs are condensed to eliminate cycles from
// bidirectional interfaces. Invariants are assigned to phases based on which
// module maintains them. Within each phase, elements are sorted by authority.
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
		authorityMap = make(map[string]float64)
	}

	// 4. Build module-level dependency graph from adjacent relationships
	modules, err := storage.ListModules(db, specID)
	if err != nil {
		return nil, err
	}
	moduleByID := make(map[int64]string) // module DB ID → module name
	moduleNames := make(map[string]bool)
	for _, m := range modules {
		moduleByID[m.ID] = m.ModuleName
		moduleNames[m.ModuleName] = true
	}

	rels, err := storage.GetModuleRelationships(db, specID)
	if err != nil {
		return nil, err
	}

	// Group: module → maintained invariants, module → adjacent modules
	moduleMainInvs := make(map[string][]string)
	moduleAdjacentTo := make(map[string][]string) // module → list of modules it depends on

	for _, r := range rels {
		modName := moduleByID[r.ModuleID]
		if modName == "" {
			continue
		}
		switch r.RelType {
		case "maintains":
			moduleMainInvs[modName] = append(moduleMainInvs[modName], r.Target)
		case "adjacent":
			if moduleNames[r.Target] {
				moduleAdjacentTo[modName] = append(moduleAdjacentTo[modName], r.Target)
			}
		}
	}

	// 5. Compute SCCs via Tarjan's algorithm on module graph
	sccIndex := computeModuleSCCs(moduleNames, moduleAdjacentTo)
	// sccIndex maps module name → SCC index

	// 6. Build condensed DAG: SCC → SCC edges
	numSCCs := 0
	for _, idx := range sccIndex {
		if idx+1 > numSCCs {
			numSCCs = idx + 1
		}
	}
	if numSCCs == 0 && len(moduleNames) > 0 {
		numSCCs = 1
	}

	// Build SCC adjacency for condensation DAG
	sccEdges := make(map[[2]int]bool)
	sccInDegree := make(map[int]int)
	for i := 0; i < numSCCs; i++ {
		sccInDegree[i] = 0
	}
	for modA, deps := range moduleAdjacentTo {
		sccA := sccIndex[modA]
		for _, modB := range deps {
			sccB := sccIndex[modB]
			if sccA != sccB {
				edge := [2]int{sccA, sccB}
				if !sccEdges[edge] {
					sccEdges[edge] = true
					sccInDegree[sccA]++ // A depends on B, so A has incoming from B
				}
			}
		}
	}

	// 7. Topological sort of condensed DAG (Kahn's)
	sccPhase := make(map[int]int) // SCC index → phase number
	var sccQueue []int
	for i := 0; i < numSCCs; i++ {
		if sccInDegree[i] == 0 {
			sccQueue = append(sccQueue, i)
		}
	}
	sccDependedBy := make(map[int][]int) // B → [A...] where A depends on B
	for edge := range sccEdges {
		sccDependedBy[edge[1]] = append(sccDependedBy[edge[1]], edge[0])
	}

	phaseNum := 0
	for len(sccQueue) > 0 {
		sort.Ints(sccQueue)
		var nextQueue []int
		for _, scc := range sccQueue {
			sccPhase[scc] = phaseNum
			for _, dep := range sccDependedBy[scc] {
				sccInDegree[dep]--
				if sccInDegree[dep] == 0 {
					nextQueue = append(nextQueue, dep)
				}
			}
		}
		sccQueue = nextQueue
		phaseNum++
	}

	// 8. Map invariants → phase via module → SCC → phase
	invPhase := make(map[string]int) // invariant ID → phase
	for modName, maintained := range moduleMainInvs {
		scc := sccIndex[modName]
		phase := sccPhase[scc]
		for _, invID := range maintained {
			invPhase[invID] = phase
		}
	}

	// 9. Build node set with domain filter
	nodeSet := make(map[string]bool)
	for _, inv := range invs {
		if opts.Domain == "" || domainMap[inv.InvariantID] == opts.Domain {
			nodeSet[inv.InvariantID] = true
		}
	}

	// 10. Group invariants by phase
	phaseElements := make(map[int][]Element)
	for id := range nodeSet {
		inv := invMap[id]
		phase := invPhase[id] // 0 for unmapped invariants (no module maintains them)
		phaseElements[phase] = append(phaseElements[phase], Element{
			ID:        id,
			Type:      "invariant",
			Title:     inv.Title,
			Authority: authorityMap[id],
			Domain:    domainMap[id],
		})
	}

	// 11. Sort elements within each phase by authority (descending)
	var phases []Phase
	maxPhase := 0
	for p := range phaseElements {
		if p > maxPhase {
			maxPhase = p
		}
	}
	processed := 0
	for p := 0; p <= maxPhase; p++ {
		elems := phaseElements[p]
		if len(elems) == 0 {
			continue
		}
		sort.Slice(elems, func(i, j int) bool {
			if elems[i].Authority != elems[j].Authority {
				return elems[i].Authority > elems[j].Authority
			}
			return elems[i].ID < elems[j].ID
		})
		label := "Foundation — no dependencies"
		if p > 0 {
			label = "Depends on earlier phases"
		}
		phases = append(phases, Phase{
			PhaseNum: p,
			Label:    label,
			Elements: elems,
		})
		processed += len(elems)
	}

	// 12. Detect cycles (SCCs with in-degree > 0 after Kahn's)
	var cycles []string
	for i := 0; i < numSCCs; i++ {
		if sccInDegree[i] > 0 {
			// Collect module names in this unresolved SCC
			for modName := range moduleNames {
				if sccIndex[modName] == i {
					cycles = append(cycles, modName)
				}
			}
		}
	}
	sort.Strings(cycles)

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

// computeModuleSCCs runs Tarjan's algorithm on the module graph
// and returns a map from module name to SCC index.
func computeModuleSCCs(moduleNames map[string]bool, adjacent map[string][]string) map[string]int {
	index := 0
	sccCount := 0
	nodeIndex := make(map[string]int)
	nodeLowlink := make(map[string]int)
	onStack := make(map[string]bool)
	var stack []string
	result := make(map[string]int) // module → SCC index

	var strongConnect func(v string)
	strongConnect = func(v string) {
		nodeIndex[v] = index
		nodeLowlink[v] = index
		index++
		stack = append(stack, v)
		onStack[v] = true

		for _, w := range adjacent[v] {
			if !moduleNames[w] {
				continue
			}
			if _, visited := nodeIndex[w]; !visited {
				strongConnect(w)
				if nodeLowlink[w] < nodeLowlink[v] {
					nodeLowlink[v] = nodeLowlink[w]
				}
			} else if onStack[w] {
				if nodeIndex[w] < nodeLowlink[v] {
					nodeLowlink[v] = nodeIndex[w]
				}
			}
		}

		if nodeLowlink[v] == nodeIndex[v] {
			for {
				w := stack[len(stack)-1]
				stack = stack[:len(stack)-1]
				onStack[w] = false
				result[w] = sccCount
				if w == v {
					break
				}
			}
			sccCount++
		}
	}

	// Process all modules (sorted for determinism)
	sortedModules := make([]string, 0, len(moduleNames))
	for m := range moduleNames {
		sortedModules = append(sortedModules, m)
	}
	sort.Strings(sortedModules)

	for _, m := range sortedModules {
		if _, visited := nodeIndex[m]; !visited {
			strongConnect(m)
		}
	}

	return result
}
