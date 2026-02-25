package progress

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
	Done          string // comma-separated invariant IDs or domain names
	AsJSON        bool
	UseWitnesses  bool   // load persistent witnesses as done set
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
	ID         string   `json:"id"`
	Title      string   `json:"title"`
	Domain     string   `json:"domain"`
	WaitingOn  []string `json:"waiting_on"` // unsatisfied dependency IDs
}

// Analyze computes progress status by partitioning invariants into done/frontier/blocked.
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

	// 4. Build dependency graph (same pattern as implorder)
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

	moduleMainInvs := make(map[string][]string)
	moduleInterfaceInvs := make(map[string][]string)
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

	nodeSet := make(map[string]bool)
	for _, inv := range invs {
		nodeSet[inv.InvariantID] = true
	}

	// deps[X] = {Y: true} means X depends on Y
	deps := make(map[string]map[string]bool)
	for node := range nodeSet {
		deps[node] = make(map[string]bool)
	}

	// Note: interface relationships are soft dependencies (consumed, not produced).
	// Creating hard edges from interfaces causes circular dependencies when
	// two modules interface each other's maintained invariants.
	// Only hard implementation dependencies create DAG edges.
	_ = moduleInterfaceInvs // interfaces tracked but not used for hard deps

	// 4.5. Load persistent witness done set
	doneSet := make(map[string]bool)
	if opts.UseWitnesses {
		if wSet, err := witness.ValidDoneSet(db, specID); err == nil {
			for id := range wSet {
				doneSet[id] = true
			}
		}
	}

	// 5. Expand "done" set from --done flag (additive on top of witnesses)
	if opts.Done != "" {
		for _, token := range strings.Split(opts.Done, ",") {
			token = strings.TrimSpace(token)
			if token == "" {
				continue
			}
			// Check if it's a domain name
			if invIDs, ok := domainToInvs[token]; ok {
				for _, id := range invIDs {
					doneSet[id] = true
				}
			} else if nodeSet[token] {
				// It's an invariant ID
				doneSet[token] = true
			}
		}
	}

	// 6. Partition into done/frontier/blocked
	// Also compute reverse deps for unblocks count
	reverseDeps := make(map[string][]string) // Y → [X...] where X depends on Y
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

		// Check unsatisfied dependencies
		var waitingOn []string
		for dep := range deps[id] {
			if !doneSet[dep] {
				waitingOn = append(waitingOn, dep)
			}
		}
		sort.Strings(waitingOn)

		if len(waitingOn) == 0 {
			// Count how many blocked items this would unblock
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

	// Sort results for deterministic output
	sort.Slice(done, func(i, j int) bool { return done[i].ID < done[j].ID })
	sort.Slice(frontier, func(i, j int) bool {
		// Sort by authority descending, then ID ascending
		if frontier[i].Authority != frontier[j].Authority {
			return frontier[i].Authority > frontier[j].Authority
		}
		return frontier[i].ID < frontier[j].ID
	})
	sort.Slice(blocked, func(i, j int) bool { return blocked[i].ID < blocked[j].ID })

	// Ensure non-nil slices for JSON
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
