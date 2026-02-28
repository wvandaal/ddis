// Package causal implements the causal DAG, CRDT merge, bisect, and blame
// operations for event-sourced DDIS specs (APP-INV-074, APP-INV-081, APP-INV-082, APP-INV-084).
package causal

// ddis:implements APP-INV-074 (causal ordering — DAG construction and validation)
// ddis:implements APP-INV-081 (CRDT convergence — semilattice merge for independent events)
// ddis:implements APP-INV-082 (bisect correctness — binary search over event log)
// ddis:implements APP-INV-084 (causal provenance — element-to-event tracing)
// ddis:implements APP-ADR-060 (event references for causal metadata)
// ddis:implements APP-ADR-063 (semilattice merge for CRDT)

import (
	"encoding/json"
	"fmt"
	"sort"

	"github.com/wvandaal/ddis/internal/events"
)

// DAG represents the causal dependency graph of events.
type DAG struct {
	nodes map[string]*events.Event
	edges map[string][]string // cause -> effects
	back  map[string][]string // effect -> causes
}

// NewDAG constructs a causal DAG from a set of events (APP-INV-074).
func NewDAG(evts []*events.Event) (*DAG, error) {
	dag := &DAG{
		nodes: make(map[string]*events.Event, len(evts)),
		edges: make(map[string][]string),
		back:  make(map[string][]string),
	}

	for _, e := range evts {
		dag.nodes[e.ID] = e
	}

	for _, e := range evts {
		for _, causeID := range e.Causes {
			if _, ok := dag.nodes[causeID]; !ok {
				return nil, fmt.Errorf("event %s references unknown cause %s", e.ID, causeID)
			}
			dag.edges[causeID] = append(dag.edges[causeID], e.ID)
			dag.back[e.ID] = append(dag.back[e.ID], causeID)
		}
	}

	return dag, nil
}

// Reachable returns true if there is a causal path from source to target.
func (d *DAG) Reachable(source, target string) bool {
	visited := make(map[string]bool)
	queue := []string{source}
	for len(queue) > 0 {
		curr := queue[0]
		queue = queue[1:]
		if curr == target {
			return true
		}
		if visited[curr] {
			continue
		}
		visited[curr] = true
		queue = append(queue, d.edges[curr]...)
	}
	return false
}

// Independent returns true if two events have no causal relationship.
func (d *DAG) Independent(id1, id2 string) bool {
	return !d.Reachable(id1, id2) && !d.Reachable(id2, id1)
}

// Provenance traces an element back to all events that created or modified it (APP-INV-084).
func Provenance(evts []*events.Event, elementID string) []*events.Event {
	var chain []*events.Event
	for _, e := range evts {
		// Check if this event's payload references the element ID
		var payload map[string]interface{}
		if err := json.Unmarshal(e.Payload, &payload); err != nil {
			continue
		}
		if id, ok := payload["id"].(string); ok && id == elementID {
			chain = append(chain, e)
		}
		if id, ok := payload["invariant_id"].(string); ok && id == elementID {
			chain = append(chain, e)
		}
	}
	sort.Slice(chain, func(i, j int) bool {
		return chain[i].Timestamp < chain[j].Timestamp
	})
	return chain
}

// Merge combines two independent event streams using semilattice merge (APP-INV-081, APP-ADR-063).
// Independent events commute. Concurrent updates to the same element use LWW resolution.
func Merge(streamA, streamB []*events.Event) []*events.Event {
	seen := make(map[string]bool)
	byID := make(map[string]*events.Event)

	// Collect all events, deduplicating by ID
	for _, e := range streamA {
		byID[e.ID] = e
		seen[e.ID] = true
	}
	for _, e := range streamB {
		if _, exists := seen[e.ID]; !exists {
			byID[e.ID] = e
		}
	}

	// Detect LWW conflicts: concurrent updates to the same element
	type elementKey struct{ typ, id string }
	latestByElement := make(map[elementKey]*events.Event)

	for _, e := range byID {
		var payload map[string]interface{}
		if err := json.Unmarshal(e.Payload, &payload); err != nil {
			continue
		}
		var ek elementKey
		if id, ok := payload["id"].(string); ok {
			ek = elementKey{typ: e.Type, id: id}
		} else if id, ok := payload["invariant_id"].(string); ok {
			ek = elementKey{typ: e.Type, id: id}
		} else {
			continue
		}

		if existing, ok := latestByElement[ek]; ok {
			// LWW: latest timestamp wins, agent ID (from event ID) as tiebreaker
			if e.Timestamp > existing.Timestamp || (e.Timestamp == existing.Timestamp && e.ID > existing.ID) {
				// Remove the loser
				delete(byID, existing.ID)
				latestByElement[ek] = e
			} else {
				delete(byID, e.ID)
			}
		} else {
			latestByElement[ek] = e
		}
	}

	// Collect result and sort by timestamp for stable ordering
	result := make([]*events.Event, 0, len(byID))
	for _, e := range byID {
		result = append(result, e)
	}
	sort.Slice(result, func(i, j int) bool {
		if result[i].Timestamp == result[j].Timestamp {
			return result[i].ID < result[j].ID
		}
		return result[i].Timestamp < result[j].Timestamp
	})

	return result
}

// BisectPredicate is a function that returns true if a defect is present in the given state.
type BisectPredicate func(evts []*events.Event) (bool, error)

// Bisect finds the earliest defect-introducing event via binary search (APP-INV-082).
// The predicate should return true if the defect is present when the given events are materialized.
func Bisect(evts []*events.Event, predicate BisectPredicate) (*events.Event, error) {
	if len(evts) == 0 {
		return nil, fmt.Errorf("empty event log")
	}

	// First check if defect exists at all
	hasDefect, err := predicate(evts)
	if err != nil {
		return nil, fmt.Errorf("predicate check on full log: %w", err)
	}
	if !hasDefect {
		return nil, fmt.Errorf("no defect found in event log")
	}

	lo, hi := 0, len(evts)-1
	for lo < hi {
		mid := (lo + hi) / 2
		defect, err := predicate(evts[:mid+1])
		if err != nil {
			return nil, fmt.Errorf("predicate check at position %d: %w", mid, err)
		}
		if defect {
			hi = mid
		} else {
			lo = mid + 1
		}
	}

	return evts[lo], nil
}
