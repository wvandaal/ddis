package triage

// ddis:implements APP-ADR-053 (issue lifecycle as event-sourced state machine — state types)
// ddis:maintains APP-INV-063 (issue-discovery linkage — thread_id in triaged state)

// State represents the lifecycle state of a triaged issue.
type State string

const (
	StateFiled        State = "filed"
	StateTriaged      State = "triaged"
	StateSpecified    State = "specified"
	StateImplementing State = "implementing"
	StateVerified     State = "verified"
	StateClosed       State = "closed"
	StateWontFix      State = "wont_fix"
)

// IsTerminal returns true if the state is a terminal state (closed or wont_fix).
func (s State) IsTerminal() bool {
	return s == StateClosed || s == StateWontFix
}

// stateOrder maps lifecycle states to their progression order.
var stateOrder = map[State]int{
	StateFiled:        0,
	StateTriaged:      1,
	StateSpecified:    2,
	StateImplementing: 3,
	StateVerified:     4,
	StateClosed:       5,
	StateWontFix:      5,
}

// Order returns the lifecycle progression index (0=filed, 4=verified, 5=closed).
func (s State) Order() int {
	if o, ok := stateOrder[s]; ok {
		return o
	}
	return -1
}

// Measure is the triage measure μ(S) = (open_issues, unspecified, drift) ∈ ℕ³.
// The lexicographic ordering on ℕ³ is well-founded (APP-INV-068).
type Measure struct {
	OpenIssues  int `json:"open_issues"`
	Unspecified int `json:"unspecified"`
	DriftScore  int `json:"drift_score"`
}

// LexLess returns true if a <_lex b in the lexicographic ordering on ℕ³.
func LexLess(a, b Measure) bool {
	if a.OpenIssues != b.OpenIssues {
		return a.OpenIssues < b.OpenIssues
	}
	if a.Unspecified != b.Unspecified {
		return a.Unspecified < b.Unspecified
	}
	return a.DriftScore < b.DriftScore
}

// IsFixpoint returns true if μ = (0, 0, 0).
func (m Measure) IsFixpoint() bool {
	return m.OpenIssues == 0 && m.Unspecified == 0 && m.DriftScore == 0
}

// EvidenceChain is the formal certificate of issue completion (APP-INV-065).
type EvidenceChain struct {
	IssueNumber int                `json:"issue_number"`
	Entries     []EvidenceEntry    `json:"entries"`
	Complete    bool               `json:"complete"`
}

// EvidenceEntry records the witness and challenge for a single invariant.
type EvidenceEntry struct {
	InvariantID  string `json:"invariant_id"`
	WitnessID    int64  `json:"witness_id"`
	WitnessType  string `json:"witness_type"`
	ChallengeID  int64  `json:"challenge_id"`
	Verdict      string `json:"verdict"`
}

// Violation represents a missing element in the evidence chain.
type Violation struct {
	InvariantID string `json:"invariant_id"`
	Type        string `json:"type"` // missing_witness, stale_witness, missing_challenge, non_confirmed
	Detail      string `json:"detail"`
	Remedy      string `json:"remedy"`
}

// IssueInfo contains the derived state and metadata of an issue.
type IssueInfo struct {
	Number              int      `json:"number"`
	State               State    `json:"state"`
	ThreadID            string   `json:"thread_id,omitempty"`
	AffectedInvariants  []string `json:"affected_invariants,omitempty"`
	ValidTransitions    []string `json:"valid_transitions,omitempty"`
	NextAction          string   `json:"next_action,omitempty"`
}

// FitnessResult is the output of the Spec Fitness Function F(S) (APP-INV-069).
type FitnessResult struct {
	Score   float64         `json:"score"`
	Signals FitnessSignals  `json:"signals"`
}

// FitnessSignals are the 6 normalized quality signals.
type FitnessSignals struct {
	Validation    float64 `json:"validation"`     // V(S) = passed/total
	Coverage      float64 `json:"coverage"`        // C(S) = coverage_pct
	Drift         float64 `json:"drift"`           // D(S) = drift/max_drift
	ChallengeHP   float64 `json:"challenge_health"` // H(S) = confirmed/total
	Contradictions float64 `json:"contradictions"`   // K(S) = found/pairs
	IssueBacklog  float64 `json:"issue_backlog"`    // I(S) = open/total
}

// Deficiency represents a quality gap with estimated ΔF.
type Deficiency struct {
	Category    string  `json:"category"`     // validate, coverage, drift, challenge, contradict, issue
	Description string  `json:"description"`
	Action      string  `json:"action"`       // executable CLI command
	DeltaF      float64 `json:"delta_f"`      // estimated fitness improvement
}

// Protocol is the agent-executable JSON document (APP-INV-070).
type Protocol struct {
	Version     string              `json:"version"`
	SpecID      int64               `json:"spec_id"`
	Fitness     FitnessSection      `json:"fitness"`
	Measure     Measure             `json:"measure"`
	Issues      []IssueInfo         `json:"issues"`
	RankedWork  []Deficiency        `json:"ranked_work"`
	Convergence ConvergenceSection  `json:"convergence"`
}

// FitnessSection of the protocol.
type FitnessSection struct {
	Current    float64   `json:"current"`
	Target     float64   `json:"target"`
	Trajectory []float64 `json:"trajectory"`
	Lyapunov   float64   `json:"lyapunov"`
}

// ConvergenceSection of the protocol.
type ConvergenceSection struct {
	LyapunovDecreasing    bool `json:"lyapunov_decreasing"`
	MeasureDecreasing     bool `json:"measure_decreasing"`
	EstimatedStepsToFP    int  `json:"estimated_steps_to_fixpoint"`
}
