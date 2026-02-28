package progress

// ddis:implements APP-INV-112 (module-level dependency ordering)
// ddis:implements APP-ADR-080 (module-level DAG with SCC cycle breaking)

import (
	"database/sql"
	"fmt"
	"sort"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/witness"
)

// Options controls progress analysis behavior.
type Options struct {
	Done         string // comma-separated invariant IDs or domain names
	AsJSON       bool
	UseWitnesses bool // load persistent witnesses as done set
}

// ProgressResult holds the complete progress analysis output.
type ProgressResult struct {
	Done            []DoneItem     `json:"done"`
	Frontier        []FrontierItem `json:"frontier"`
	Blocked         []BlockedItem  `json:"blocked"`
	Progress        string         `json:"progress"`
	NextRecommended string         `json:"next_recommended"`
}

// DoneItem is an invariant marked as completed.
type DoneItem struct {
	ID     string `json:"id"`
	Title  string `json:"title"`
	Domain string `json:"domain"`
}

// FrontierItem is an invariant ready to work on (all deps satisfied).
type FrontierItem struct {
	ID        string  `json:"id"`
	Title     string  `json:"title"`
	Domain    string  `json:"domain"`
	Authority float64 `json:"authority"`
	Unblocks  int     `json:"unblocks"` // how many blocked items this unblocks
}

// BlockedItem is an invariant waiting on unsatisfied dependencies.
type BlockedItem struct {
	ID        string   `json:"id"`
	Title     string   `json:"title"`
	Domain    string   `json:"domain"`
	WaitingOn []string `json:"waiting_on"` // unsatisfied dependency IDs
}

// Analyze computes progress status by partitioning invariants into done/frontier/blocked.
// Uses module-level DAG from adjacent declarations to determine ordering.
func Analyze(db *sql.DB, specID int64, opts Options) (*ProgressResult, error) {
	// 1. Load invariants
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, fmt.Errorf("list invariants: %w", err)
	}
	invMap := make(map[string]storage.Invariant)
	for _, inv := range invs {
		invMap[inv.InvariantID] = inv
	}

	// 2. Load registry for domain info
	registry, err := storage.GetInvariantRegistryEntries(db, specID)
	if err != nil {
		return nil, fmt.Errorf("get registry: %w", err)
	}
	domainMap := make(map[string]string)
	domainToInvs := make(map[string][]string) // domain → invariant IDs
	for _, r := range registry {
		domainMap[r.InvariantID] = r.Domain
		domainToInvs[r.Domain] = append(domainToInvs[r.Domain], r.InvariantID)
	}

	// 3. Load authority scores
	authorityMap, err := storage.GetAuthorityScores(db, specID)
	if err != nil {
		authorityMap = make(map[string]float64)
	}

	// 4. Build module-level dependency graph from adjacent relationships
	modules, err := storage.ListModules(db, specID)
	if err != nil {
		return nil, fmt.Errorf("list modules: %w", err)
	}
	moduleByID := make(map[int64]string)
	moduleNames := make(map[string]bool)
	for _, m := range modules {
		moduleByID[m.ID] = m.ModuleName
		moduleNames[m.ModuleName] = true
	}

	rels, err := storage.GetModuleRelationships(db, specID)
	if err != nil {
		return nil, fmt.Errorf("get relationships: %w", err)
	}

	moduleMainInvs := make(map[string][]string)
	moduleAdjacentTo := make(map[string][]string)
	invToModule := make(map[string]string) // invariant → maintaining module

	for _, r := range rels {
		modName := moduleByID[r.ModuleID]
		if modName == "" {
			continue
		}
		switch r.RelType {
		case "maintains":
			moduleMainInvs[modName] = append(moduleMainInvs[modName], r.Target)
			invToModule[r.Target] = modName
		case "adjacent":
			if moduleNames[r.Target] {
				moduleAdjacentTo[modName] = append(moduleAdjacentTo[modName], r.Target)
			}
		}
	}

	// 5. Compute module-level phases via SCC condensation
	sccIndex := computeModuleSCCs(moduleNames, moduleAdjacentTo)
	numSCCs := 0
	for _, idx := range sccIndex {
		if idx+1 > numSCCs {
			numSCCs = idx + 1
		}
	}

	// Build condensed DAG
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
					sccInDegree[sccA]++
				}
			}
		}
	}

	// Topological sort of condensed DAG
	sccPhase := make(map[int]int)
	var sccQueue []int
	for i := 0; i < numSCCs; i++ {
		if sccInDegree[i] == 0 {
			sccQueue = append(sccQueue, i)
		}
	}
	sccDependedBy := make(map[int][]int)
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

	// Map invariants to phases
	invPhase := make(map[string]int)
	for modName, maintained := range moduleMainInvs {
		scc := sccIndex[modName]
		phase := sccPhase[scc]
		for _, invID := range maintained {
			invPhase[invID] = phase
		}
	}

	// 6. Build invariant-level dependencies from module phases:
	// An invariant in phase P depends on ALL invariants maintained by
	// predecessor modules (modules in phases < P).
	nodeSet := make(map[string]bool)
	for _, inv := range invs {
		nodeSet[inv.InvariantID] = true
	}

	deps := make(map[string]map[string]bool) // X depends on Y
	for id := range nodeSet {
		deps[id] = make(map[string]bool)
	}

	// For each invariant I in phase P > 0, it depends on all invariants in earlier phases
	// maintained by predecessor modules. We track these as module-level deps for efficiency.
	predecessorInvs := make(map[int][]string) // phase → all invariants in that phase
	for id := range nodeSet {
		p := invPhase[id]
		predecessorInvs[p] = append(predecessorInvs[p], id)
	}

	for id := range nodeSet {
		myPhase := invPhase[id]
		if myPhase == 0 {
			continue
		}
		// Depend on all invariants in strictly earlier phases
		for p := 0; p < myPhase; p++ {
			for _, predID := range predecessorInvs[p] {
				deps[id][predID] = true
			}
		}
	}

	// 7. Load persistent witness done set
	doneSet := make(map[string]bool)
	if opts.UseWitnesses {
		if wSet, err := witness.ValidDoneSet(db, specID); err == nil {
			for id := range wSet {
				doneSet[id] = true
			}
		}
	}

	// 8. Expand "done" set from --done flag (additive on top of witnesses)
	if opts.Done != "" {
		for _, token := range strings.Split(opts.Done, ",") {
			token = strings.TrimSpace(token)
			if token == "" {
				continue
			}
			if invIDs, ok := domainToInvs[token]; ok {
				for _, id := range invIDs {
					doneSet[id] = true
				}
			} else if nodeSet[token] {
				doneSet[token] = true
			}
		}
	}

	// 9. Partition into done/frontier/blocked
	reverseDeps := make(map[string][]string)
	for x, depSet := range deps {
		for y := range depSet {
			reverseDeps[y] = append(reverseDeps[y], x)
		}
	}

	var done []DoneItem
	var frontier []FrontierItem
	var blocked []BlockedItem

	for id := range nodeSet {
		inv := invMap[id]
		domain := domainMap[id]

		if doneSet[id] {
			done = append(done, DoneItem{
				ID:     id,
				Title:  inv.Title,
				Domain: domain,
			})
			continue
		}

		var waitingOn []string
		for dep := range deps[id] {
			if !doneSet[dep] {
				waitingOn = append(waitingOn, dep)
			}
		}
		sort.Strings(waitingOn)

		if len(waitingOn) == 0 {
			unblocks := 0
			for _, dependent := range reverseDeps[id] {
				if !doneSet[dependent] {
					unblocks++
				}
			}
			frontier = append(frontier, FrontierItem{
				ID:        id,
				Title:     inv.Title,
				Domain:    domain,
				Authority: authorityMap[id],
				Unblocks:  unblocks,
			})
		} else {
			blocked = append(blocked, BlockedItem{
				ID:        id,
				Title:     inv.Title,
				Domain:    domain,
				WaitingOn: waitingOn,
			})
		}
	}

	// Sort results
	sort.Slice(done, func(i, j int) bool { return done[i].ID < done[j].ID })
	sort.Slice(frontier, func(i, j int) bool {
		if frontier[i].Authority != frontier[j].Authority {
			return frontier[i].Authority > frontier[j].Authority
		}
		return frontier[i].ID < frontier[j].ID
	})
	sort.Slice(blocked, func(i, j int) bool { return blocked[i].ID < blocked[j].ID })

	if done == nil {
		done = []DoneItem{}
	}
	if frontier == nil {
		frontier = []FrontierItem{}
	}
	if blocked == nil {
		blocked = []BlockedItem{}
	}

	total := len(nodeSet)
	progress := fmt.Sprintf("%d/%d done (%.0f%%)", len(done), total,
		float64(len(done))/float64(max(total, 1))*100)

	nextRecommended := ""
	if len(frontier) > 0 {
		nextRecommended = frontier[0].ID
	}

	return &ProgressResult{
		Done:            done,
		Frontier:        frontier,
		Blocked:         blocked,
		Progress:        progress,
		NextRecommended: nextRecommended,
	}, nil
}

// computeModuleSCCs runs Tarjan's algorithm on the module graph.
func computeModuleSCCs(moduleNames map[string]bool, adjacent map[string][]string) map[string]int {
	index := 0
	sccCount := 0
	nodeIndex := make(map[string]int)
	nodeLowlink := make(map[string]int)
	onStack := make(map[string]bool)
	var stack []string
	result := make(map[string]int)

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
