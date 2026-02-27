package triage

// ddis:implements APP-ADR-053 (issue lifecycle as event-sourced state machine — state derivation)
// ddis:maintains APP-INV-063 (issue-discovery linkage — thread_id extraction)
// ddis:maintains APP-INV-068 (fixpoint termination — state derivation is the fold homomorphism)

import (
	"encoding/json"
	"fmt"
	"sort"

	"github.com/wvandaal/ddis/internal/events"
)

// transitionTable encodes the state machine: transitionTable[currentState][eventType] = nextState.
// nil entries are invalid transitions (hard error, not silent discard).
var transitionTable = map[State]map[string]State{
	StateFiled: {
		events.TypeIssueTriaged: StateTriaged,
		events.TypeIssueWontfix: StateWontFix,
	},
	StateTriaged: {
		events.TypeIssueSpecified: StateSpecified,
		events.TypeIssueWontfix:   StateWontFix,
	},
	StateSpecified: {
		events.TypeIssueImplementing: StateImplementing,
		events.TypeIssueWontfix:      StateWontFix,
	},
	StateImplementing: {
		events.TypeIssueVerified: StateVerified,
		events.TypeIssueWontfix:  StateWontFix,
	},
	StateVerified: {
		events.TypeIssueClosed:  StateClosed,
		events.TypeIssueTriaged: StateTriaged, // regression path
		events.TypeIssueWontfix: StateWontFix,
	},
}

// triageEventTypes is the set of event types that are part of the triage lifecycle.
var triageEventTypes = map[string]bool{
	events.TypeIssueTriaged:      true,
	events.TypeIssueSpecified:    true,
	events.TypeIssueImplementing: true,
	events.TypeIssueVerified:     true,
	events.TypeIssueClosed:       true,
	events.TypeIssueWontfix:      true,
}

// DeriveIssueState replays Stream 3 events for a given issue and returns the
// current lifecycle state. This is the fold homomorphism from the event monoid
// to the state lattice (APP-ADR-053).
func DeriveIssueState(evts []events.Event, issueNumber int) (State, string, error) {
	state := StateFiled
	threadID := ""

	// Filter and sort by timestamp
	var relevant []events.Event
	for _, e := range evts {
		num := extractIssueNumber(e)
		if num == issueNumber && triageEventTypes[e.Type] {
			relevant = append(relevant, e)
		}
	}
	sort.Slice(relevant, func(i, j int) bool {
		return relevant[i].Timestamp < relevant[j].Timestamp
	})

	for _, e := range relevant {
		transitions, ok := transitionTable[state]
		if !ok {
			return state, threadID, fmt.Errorf("no transitions from terminal state %s", state)
		}
		next, valid := transitions[e.Type]
		if !valid {
			return state, threadID, fmt.Errorf("invalid transition from %s via %s for issue %d", state, e.Type, issueNumber)
		}
		state = next

		// Extract thread_id from issue_triaged events (APP-INV-063)
		if e.Type == events.TypeIssueTriaged {
			threadID = extractThreadID(e)
		}
	}

	return state, threadID, nil
}

// DeriveAllIssueStates derives state for every known issue in a single pass.
func DeriveAllIssueStates(evts []events.Event) map[int]*IssueInfo {
	// Collect all issue numbers
	issueNums := map[int]bool{}
	for _, e := range evts {
		if num := extractIssueNumber(e); num > 0 {
			issueNums[num] = true
		}
	}

	result := make(map[int]*IssueInfo, len(issueNums))
	for num := range issueNums {
		state, threadID, _ := DeriveIssueState(evts, num)
		info := &IssueInfo{
			Number:           num,
			State:            state,
			ThreadID:         threadID,
			ValidTransitions: NextValidTransitions(state),
		}
		info.AffectedInvariants = ExtractAffectedInvariants(evts, num)
		result[num] = info
	}
	return result
}

// NextValidTransitions returns the set of event types valid from the given state.
func NextValidTransitions(state State) []string {
	transitions, ok := transitionTable[state]
	if !ok {
		return nil
	}
	result := make([]string, 0, len(transitions))
	for eventType := range transitions {
		result = append(result, eventType)
	}
	sort.Strings(result)
	return result
}

// extractIssueNumber extracts the issue_number from an event's payload.
func extractIssueNumber(e events.Event) int {
	var payload map[string]interface{}
	if err := json.Unmarshal(e.Payload, &payload); err != nil {
		return 0
	}
	if num, ok := payload["issue_number"].(float64); ok {
		return int(num)
	}
	return 0
}

// extractThreadID extracts thread_id from an event's payload.
func extractThreadID(e events.Event) string {
	var payload map[string]interface{}
	if err := json.Unmarshal(e.Payload, &payload); err != nil {
		return ""
	}
	if tid, ok := payload["thread_id"].(string); ok {
		return tid
	}
	return ""
}

// ExtractAffectedInvariants collects invariant IDs from triage events for an issue.
func ExtractAffectedInvariants(evts []events.Event, issueNumber int) []string {
	seen := map[string]bool{}
	for _, e := range evts {
		if extractIssueNumber(e) != issueNumber {
			continue
		}
		var payload map[string]interface{}
		if err := json.Unmarshal(e.Payload, &payload); err != nil {
			continue
		}
		// Check for affected_invariants array
		if arr, ok := payload["affected_invariants"].([]interface{}); ok {
			for _, v := range arr {
				if s, ok := v.(string); ok {
					seen[s] = true
				}
			}
		}
		// Check for single invariant_id
		if id, ok := payload["invariant_id"].(string); ok && id != "" {
			seen[id] = true
		}
	}
	result := make([]string, 0, len(seen))
	for id := range seen {
		result = append(result, id)
	}
	sort.Strings(result)
	return result
}
