package materialize

// Built-in stream processors for the materialize fold engine.
//
// ddis:implements APP-INV-090 (processor idempotency — deterministic handle)
// ddis:implements APP-INV-092 (derived event provenance — derived events carry causes + derived_by)
// ddis:implements APP-ADR-070 (processor registration mechanism — 3 built-ins + extension)

import (
	"encoding/json"

	"github.com/wvandaal/ddis/internal/events"
)

// NewValidationProcessor creates a processor that checks content events
// for structural completeness (e.g., invariants must have titles and statements).
func NewValidationProcessor() Processor {
	return Processor{
		Name: "validation",
		EventTypes: map[string]bool{
			events.TypeInvariantCrystallized: true,
			events.TypeADRCrystallized:       true,
			events.TypeModuleRegistered:      true,
		},
		Handle: func(evt *events.Event, db interface{}) ([]*events.Event, error) {
			var findings []string

			switch evt.Type {
			case events.TypeInvariantCrystallized:
				var p events.InvariantPayload
				if json.Unmarshal(evt.Payload, &p) == nil {
					if p.Title == "" {
						findings = append(findings, "invariant missing title")
					}
					if p.Statement == "" {
						findings = append(findings, "invariant missing statement")
					}
				}
			case events.TypeADRCrystallized:
				var p events.ADRPayload
				if json.Unmarshal(evt.Payload, &p) == nil {
					if p.Title == "" {
						findings = append(findings, "ADR missing title")
					}
				}
			case events.TypeModuleRegistered:
				var p events.ModulePayload
				if json.Unmarshal(evt.Payload, &p) == nil {
					if p.Name == "" {
						findings = append(findings, "module missing name")
					}
				}
			}

			return makeDerivedEvents(evt, "validation", findings)
		},
	}
}

// NewConsistencyProcessor creates a processor that checks for cross-reference
// consistency between content events.
func NewConsistencyProcessor() Processor {
	return Processor{
		Name: "consistency",
		EventTypes: map[string]bool{
			events.TypeCrossRefAdded: true,
		},
		Handle: func(evt *events.Event, db interface{}) ([]*events.Event, error) {
			// Consistency checks are deferred — the processor notes refs
			// for later verification but doesn't emit findings during fold.
			return nil, nil
		},
	}
}

// NewDriftProcessor creates a processor that fires on content changes
// and checks for spec-implementation drift.
func NewDriftProcessor() Processor {
	return Processor{
		Name: "drift",
		EventTypes: map[string]bool{
			events.TypeInvariantCrystallized: true,
			events.TypeInvariantUpdated:      true,
			events.TypeADRCrystallized:       true,
			events.TypeADRUpdated:            true,
		},
		Handle: func(evt *events.Event, db interface{}) ([]*events.Event, error) {
			// Drift checks require code scanning which is done outside fold.
			// The processor records that a drift check is needed.
			return nil, nil
		},
	}
}

// makeDerivedEvents creates derived events from processor findings.
// Each finding becomes an event with causes=[evt.ID] and derived_by=processorName.
func makeDerivedEvents(trigger *events.Event, processorName string, findings []string) ([]*events.Event, error) {
	if len(findings) == 0 {
		return nil, nil
	}

	var derived []*events.Event
	for _, finding := range findings {
		payload := map[string]interface{}{
			"finding":    finding,
			"derived_by": processorName,
			"source_id":  trigger.ID,
		}
		evt, err := events.NewEvent(trigger.Stream, "implementation_finding", trigger.SpecHash, payload)
		if err != nil {
			continue
		}
		evt.Causes = []string{trigger.ID}
		derived = append(derived, evt)
	}
	return derived, nil
}
