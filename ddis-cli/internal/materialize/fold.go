// Package materialize implements the deterministic fold engine that replays
// JSONL event logs into SQLite materialized state (APP-INV-073, APP-ADR-059).
package materialize

// ddis:implements APP-INV-071 (log canonicality — materialize derives SQL from events)
// ddis:implements APP-INV-073 (fold determinism — pure apply function)
// ddis:implements APP-INV-075 (materialization idempotency — replay produces identical state)
// ddis:implements APP-ADR-059 (deterministic fold over incremental mutation)

import (
	"encoding/json"
	"fmt"
	"sort"

	"github.com/wvandaal/ddis/internal/events"
)

// Processor is a post-fold observer that fires after content events.
// ddis:implements APP-INV-080 (stream processor reactivity — content events trigger processors)
type Processor struct {
	Name       string
	EventTypes map[string]bool
	Handle     func(evt *events.Event, db interface{}) ([]*events.Event, error)
}

// Engine is the materialize engine that folds events into SQL state.
type Engine struct {
	processors []Processor
}

// New creates a new materialize engine.
func New() *Engine {
	return &Engine{}
}

// RegisterProcessor adds a stream processor.
// ddis:implements APP-ADR-065 (stream processors as fold observers)
func (e *Engine) RegisterProcessor(p Processor) {
	e.processors = append(e.processors, p)
}

// CausalSort sorts events respecting causal ordering (APP-INV-074).
// Events without causes preserve their original order.
// Events with causes are topologically sorted, with timestamp as tiebreaker.
func CausalSort(evts []*events.Event) ([]*events.Event, error) {
	if len(evts) == 0 {
		return evts, nil
	}

	// Build index: ID -> event
	byID := make(map[string]*events.Event, len(evts))
	for _, e := range evts {
		byID[e.ID] = e
	}

	// Build dependency graph: event ID -> set of events that depend on it
	dependents := make(map[string][]string)
	inDegree := make(map[string]int)
	for _, e := range evts {
		if _, ok := inDegree[e.ID]; !ok {
			inDegree[e.ID] = 0
		}
		for _, causeID := range e.Causes {
			dependents[causeID] = append(dependents[causeID], e.ID)
			inDegree[e.ID]++
		}
	}

	// Kahn's algorithm: topological sort
	var queue []string
	for _, e := range evts {
		if inDegree[e.ID] == 0 {
			queue = append(queue, e.ID)
		}
	}

	// Sort queue by timestamp for stable ordering within each level
	sort.Slice(queue, func(i, j int) bool {
		return byID[queue[i]].Timestamp < byID[queue[j]].Timestamp
	})

	var result []*events.Event
	for len(queue) > 0 {
		id := queue[0]
		queue = queue[1:]
		result = append(result, byID[id])

		var nextLevel []string
		for _, depID := range dependents[id] {
			inDegree[depID]--
			if inDegree[depID] == 0 {
				nextLevel = append(nextLevel, depID)
			}
		}
		// Sort within level by timestamp
		sort.Slice(nextLevel, func(i, j int) bool {
			return byID[nextLevel[i]].Timestamp < byID[nextLevel[j]].Timestamp
		})
		queue = append(queue, nextLevel...)
	}

	if len(result) != len(evts) {
		return nil, fmt.Errorf("causal cycle detected: %d events could not be sorted (expected %d)", len(evts)-len(result), len(evts))
	}

	return result, nil
}

// Apply applies a single event to the state. This is a pure function:
// no system clock, no RNG, no environment access (APP-INV-073).
// The applier argument handles the actual SQL mutation.
func Apply(applier Applier, evt *events.Event) error {
	switch evt.Type {
	case events.TypeSpecSectionDefined:
		var p events.SectionPayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal section payload: %w", err)
		}
		return applier.InsertSection(p)
	case events.TypeSpecSectionUpdated:
		var p events.SectionUpdatePayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal section update payload: %w", err)
		}
		return applier.UpdateSection(p)
	case events.TypeSpecSectionRemoved:
		var p events.SectionRemovePayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal section remove payload: %w", err)
		}
		return applier.RemoveSection(p)
	case events.TypeInvariantCrystallized:
		var p events.InvariantPayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal invariant payload: %w", err)
		}
		return applier.InsertInvariant(p)
	case events.TypeInvariantUpdated:
		var p events.InvariantUpdatePayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal invariant update payload: %w", err)
		}
		return applier.UpdateInvariant(p)
	case events.TypeInvariantRemoved:
		var p events.InvariantRemovePayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal invariant remove payload: %w", err)
		}
		return applier.RemoveInvariant(p)
	case events.TypeADRCrystallized:
		var p events.ADRPayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal ADR payload: %w", err)
		}
		return applier.InsertADR(p)
	case events.TypeADRUpdated:
		var p events.ADRUpdatePayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal ADR update payload: %w", err)
		}
		return applier.UpdateADR(p)
	case events.TypeADRSuperseded:
		var p events.ADRSupersededPayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal ADR superseded payload: %w", err)
		}
		return applier.SupersedeADR(p)
	case events.TypeWitnessRecorded:
		var p events.WitnessPayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal witness payload: %w", err)
		}
		return applier.InsertWitness(p)
	case events.TypeWitnessRevoked, events.TypeWitnessInvalidated:
		var p events.WitnessRevokePayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal witness revoke payload: %w", err)
		}
		return applier.RevokeWitness(p)
	case events.TypeChallengeCompleted:
		var p events.ChallengePayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal challenge payload: %w", err)
		}
		return applier.InsertChallenge(p)
	case events.TypeModuleRegistered:
		var p events.ModulePayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal module payload: %w", err)
		}
		return applier.InsertModule(p)
	case events.TypeGlossaryTermDefined:
		var p events.GlossaryTermPayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal glossary payload: %w", err)
		}
		return applier.InsertGlossaryTerm(p)
	case events.TypeCrossRefAdded:
		var p events.CrossRefPayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal cross-ref payload: %w", err)
		}
		return applier.InsertCrossRef(p)
	case events.TypeNegativeSpecAdded:
		var p events.NegativeSpecPayload
		if err := json.Unmarshal(evt.Payload, &p); err != nil {
			return fmt.Errorf("unmarshal negative spec payload: %w", err)
		}
		return applier.InsertNegativeSpec(p)
	default:
		// Unknown types are no-ops for forward compatibility
		return nil
	}
}

// Applier is the interface for state mutations. Each method is a pure
// SQL mutation (INSERT, UPDATE, or DELETE) with no side effects.
type Applier interface {
	InsertSection(events.SectionPayload) error
	UpdateSection(events.SectionUpdatePayload) error
	RemoveSection(events.SectionRemovePayload) error
	InsertInvariant(events.InvariantPayload) error
	UpdateInvariant(events.InvariantUpdatePayload) error
	RemoveInvariant(events.InvariantRemovePayload) error
	InsertADR(events.ADRPayload) error
	UpdateADR(events.ADRUpdatePayload) error
	SupersedeADR(events.ADRSupersededPayload) error
	InsertWitness(events.WitnessPayload) error
	RevokeWitness(events.WitnessRevokePayload) error
	InsertChallenge(events.ChallengePayload) error
	InsertModule(events.ModulePayload) error
	InsertGlossaryTerm(events.GlossaryTermPayload) error
	InsertCrossRef(events.CrossRefPayload) error
	InsertNegativeSpec(events.NegativeSpecPayload) error
}

// FoldResult captures the outcome of a materialize fold.
type FoldResult struct {
	EventsProcessed int
	EventsSkipped   int
	Errors          []FoldError
}

// FoldError captures a non-fatal error during fold.
type FoldError struct {
	EventID   string
	EventType string
	Err       error
}

// Fold replays a sequence of events through the apply function,
// collecting errors without halting (APP-INV-073, APP-INV-075).
func Fold(applier Applier, evts []*events.Event) (*FoldResult, error) {
	sorted, err := CausalSort(evts)
	if err != nil {
		return nil, fmt.Errorf("causal sort: %w", err)
	}

	result := &FoldResult{}
	for _, evt := range sorted {
		if err := Apply(applier, evt); err != nil {
			result.Errors = append(result.Errors, FoldError{
				EventID:   evt.ID,
				EventType: evt.Type,
				Err:       err,
			})
			result.EventsSkipped++
		} else {
			result.EventsProcessed++
		}
	}

	return result, nil
}
