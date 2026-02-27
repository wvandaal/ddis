package validator

// ddis:implements APP-ADR-052
// ddis:maintains APP-INV-062

import (
	"database/sql"
	"fmt"
	"regexp"
	"strings"
)

// Check 20: Lifecycle reachability — every state in the spec's transition graph
// must be reachable from the initial state, and dead-end states (states with no
// forward path to a terminal/validated state) are reported as warnings.
// Governs APP-ADR-052 and APP-INV-062.
type checkLifecycleReachability struct{}

func (c *checkLifecycleReachability) ID() int                { return 20 }
func (c *checkLifecycleReachability) Name() string           { return "Lifecycle reachability" }
func (c *checkLifecycleReachability) Applicable(string) bool { return true }

// transition represents a single directed edge in the state machine graph.
type transition struct {
	command   string
	fromState string
	toState   string
}

// tableRowRe matches markdown table rows with at least 3 pipe-delimited cells.
// Captures: | cell1 | cell2 | cell3 | ...
var tableRowRe = regexp.MustCompile(`^\s*\|([^|]+)\|([^|]+)\|([^|]+)`)

// parseTransitions parses state transition table rows from raw section text.
// Expected format: | command | from_state | to_state | (optional extra columns)
// Header and separator rows (containing "---") are skipped.
func parseTransitions(rawText string) []transition {
	var result []transition
	lines := strings.Split(rawText, "\n")
	for _, line := range lines {
		m := tableRowRe.FindStringSubmatch(line)
		if m == nil {
			continue
		}
		cmd := strings.TrimSpace(m[1])
		from := strings.TrimSpace(m[2])
		to := strings.TrimSpace(m[3])

		// Skip header rows (contain "Command" or "State") and separator rows (contain "---").
		if strings.Contains(cmd, "---") || strings.Contains(from, "---") || strings.Contains(to, "---") {
			continue
		}
		if strings.EqualFold(cmd, "command") || strings.EqualFold(from, "from") ||
			strings.EqualFold(from, "from state") || strings.EqualFold(from, "state") {
			continue
		}

		// Skip empty cells — incomplete rows.
		if cmd == "" || from == "" || to == "" {
			continue
		}

		result = append(result, transition{
			command:   cmd,
			fromState: from,
			toState:   to,
		})
	}
	return result
}

// bfsReachable performs a breadth-first search from start over the transition
// graph and returns the set of all reachable state names.
func bfsReachable(transitions []transition, start string) map[string]bool {
	// Build adjacency list: fromState → []toState
	adj := make(map[string][]string)
	for _, t := range transitions {
		adj[t.fromState] = append(adj[t.fromState], t.toState)
	}

	visited := make(map[string]bool)
	queue := []string{start}
	visited[start] = true

	for len(queue) > 0 {
		cur := queue[0]
		queue = queue[1:]
		for _, next := range adj[cur] {
			if !visited[next] {
				visited[next] = true
				queue = append(queue, next)
			}
		}
	}
	return visited
}

// findDeadEnds returns all reachable states that have no forward path leading
// eventually to the terminal state. A state is a dead-end if it is reachable
// from the initial state but the terminal state is NOT reachable from it.
func findDeadEnds(transitions []transition, reachable map[string]bool, terminal string) []string {
	// Build adjacency list for forward reachability from each state.
	adj := make(map[string][]string)
	for _, t := range transitions {
		adj[t.fromState] = append(adj[t.fromState], t.toState)
	}

	// canReachTerminal[s] = true if terminal is reachable from s.
	canReachTerminal := make(map[string]bool)
	canReachTerminal[terminal] = true

	// Iteratively propagate: if any successor can reach terminal, so can the current state.
	// Repeat until no new states are added (fixed-point).
	changed := true
	for changed {
		changed = false
		for state, successors := range adj {
			if canReachTerminal[state] {
				continue
			}
			for _, succ := range successors {
				if canReachTerminal[succ] {
					canReachTerminal[state] = true
					changed = true
					break
				}
			}
		}
	}

	var deadEnds []string
	for state := range reachable {
		if state == terminal {
			continue
		}
		if !canReachTerminal[state] {
			deadEnds = append(deadEnds, state)
		}
	}
	return deadEnds
}

func (c *checkLifecycleReachability) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	// Locate the state transitions section: §0.2 or §0.2.1.
	// Try both section paths — different specs use different numbering.
	candidatePaths := []string{"§0.2", "§0.2.1", "§0.2.0"}
	var rawText string
	found := false
	for _, path := range candidatePaths {
		var rt string
		err := db.QueryRow(
			`SELECT raw_text FROM sections WHERE spec_id = ? AND section_path = ? LIMIT 1`,
			specID, path,
		).Scan(&rt)
		if err == nil && rt != "" {
			rawText = rt
			found = true
			break
		}
	}

	// Also try a title-based fallback: search for sections titled "State Transitions".
	if !found {
		rows, err := db.Query(
			`SELECT raw_text FROM sections WHERE spec_id = ? AND (
				title LIKE '%State Transition%' OR
				title LIKE '%state transition%' OR
				title LIKE '%Transitions%'
			) ORDER BY id LIMIT 1`,
			specID,
		)
		if err == nil {
			defer rows.Close()
			if rows.Next() {
				var rt string
				if scanErr := rows.Scan(&rt); scanErr == nil && rt != "" {
					rawText = rt
					found = true
				}
			}
		}
	}

	if !found || rawText == "" {
		// Graceful degradation: no transitions section found — this is not an error.
		result.Summary = "no transitions found — lifecycle reachability check skipped"
		return result
	}

	transitions := parseTransitions(rawText)
	if len(transitions) == 0 {
		result.Summary = "no parseable transitions found — lifecycle reachability check skipped"
		return result
	}

	// Determine the initial state: use the first fromState seen in the transition list.
	initialState := transitions[0].fromState

	// Determine the terminal state: the conventional name is "ValidatedSpec" or the
	// last toState that appears in no fromState (a sink node).
	terminalState := findTerminalState(transitions)

	// BFS from initial state to find all reachable states.
	reachable := bfsReachable(transitions, initialState)

	// Collect all states mentioned in the transition table.
	allStates := collectAllStates(transitions)

	// Find states present in the graph but unreachable from the initial state.
	unreachableStates := []string{}
	for state := range allStates {
		if !reachable[state] {
			unreachableStates = append(unreachableStates, state)
		}
	}

	// Find dead-end states: reachable but with no forward path to terminal.
	deadEnds := []string{}
	if terminalState != "" {
		deadEnds = findDeadEnds(transitions, reachable, terminalState)
	}

	// Report unreachable states as warnings (not failures — specs evolve).
	for _, state := range unreachableStates {
		result.Findings = append(result.Findings, Finding{
			CheckID:   c.ID(),
			CheckName: c.Name(),
			Severity:  SeverityWarning,
			Message:   fmt.Sprintf("state %q is unreachable from initial state %q", state, initialState),
			Location:  state,
		})
	}

	// Report dead-end states as warnings.
	for _, state := range deadEnds {
		result.Findings = append(result.Findings, Finding{
			CheckID:   c.ID(),
			CheckName: c.Name(),
			Severity:  SeverityWarning,
			Message:   fmt.Sprintf("state %q is a dead-end: reachable from %q but has no path to terminal state %q", state, initialState, terminalState),
			Location:  state,
		})
	}

	totalIssues := len(unreachableStates) + len(deadEnds)
	if terminalState == "" {
		result.Summary = fmt.Sprintf("%d states, %d reachable from %q, %d unreachable (no terminal state identified)",
			len(allStates), len(reachable), initialState, len(unreachableStates))
	} else {
		result.Summary = fmt.Sprintf("%d states, %d reachable from %q → %q, %d issue(s)",
			len(allStates), len(reachable), initialState, terminalState, totalIssues)
	}

	return result
}

// collectAllStates returns the set of all state names mentioned in the transition table.
func collectAllStates(transitions []transition) map[string]bool {
	states := make(map[string]bool)
	for _, t := range transitions {
		states[t.fromState] = true
		states[t.toState] = true
	}
	return states
}

// findTerminalState identifies the sink node in the transition graph: a state
// that appears as a toState but never as a fromState. If multiple sinks exist,
// returns the one whose name contains "Validated" or "Terminal" or "Final".
// Falls back to the last toState in the table if no preferred sink is found.
func findTerminalState(transitions []transition) string {
	fromStates := make(map[string]bool)
	for _, t := range transitions {
		fromStates[t.fromState] = true
	}

	var sinks []string
	seen := make(map[string]bool)
	for _, t := range transitions {
		if !fromStates[t.toState] && !seen[t.toState] {
			sinks = append(sinks, t.toState)
			seen[t.toState] = true
		}
	}

	if len(sinks) == 0 {
		return ""
	}
	if len(sinks) == 1 {
		return sinks[0]
	}

	// Prefer a sink whose name suggests validation/finality.
	preferred := []string{"Validated", "Terminal", "Final", "Complete", "Done"}
	for _, pref := range preferred {
		for _, s := range sinks {
			if strings.Contains(s, pref) {
				return s
			}
		}
	}

	// Fall back to the first sink found.
	return sinks[0]
}
