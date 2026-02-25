package autoprompt

import "encoding/json"

// ddis:maintains APP-INV-034 (state monad universality)
// ddis:implements APP-ADR-023 (LLMs as primary spec authors)

// CommandResult is the universal return type for auto-prompting commands.
// Every command returns (output, state, guidance) — the state monad triple.
type CommandResult struct {
	Output   string        `json:"output"`
	State    StateSnapshot `json:"state"`
	Guidance Guidance      `json:"guidance"`
}

// StateSnapshot summarizes the current state for the LLM interpreter.
type StateSnapshot struct {
	ActiveThread     string  `json:"active_thread"`
	Confidence       [5]int  `json:"confidence"`       // [coverage, depth, coherence, completeness, formality], each 0-10
	LimitingFactor   string  `json:"limiting_factor"`
	OpenQuestions    int     `json:"open_questions"`
	ArtifactsWritten int     `json:"artifacts_written"`
	SpecDrift        float64 `json:"spec_drift"`
	Iteration        int     `json:"iteration"`
	ModeObserved     string  `json:"mode_observed,omitempty"`
}

// Guidance provides light hints for the LLM's next move.
type Guidance struct {
	ObservedMode    string   `json:"observed_mode,omitempty"`
	DoFHint         string   `json:"dof_hint"`                     // {very_low, low, mid, high, very_high}
	SuggestedNext   []string `json:"suggested_next"`
	RelevantContext []string `json:"relevant_context,omitempty"`
	TranslationHint string   `json:"translation_hint,omitempty"`
	Attenuation     float64  `json:"attenuation"`
}

// ConfidenceIndex names for the Confidence array.
const (
	ConfCoverage     = 0
	ConfDepth        = 1
	ConfCoherence    = 2
	ConfCompleteness = 3
	ConfFormality    = 4
)

// DimensionNames maps confidence indices to human-readable names.
var DimensionNames = [5]string{"coverage", "depth", "coherence", "completeness", "formality"}

// DimensionPriority defines tie-breaking order for SelectFocusDimension.
// Lower index = higher priority.
var DimensionPriority = [5]int{ConfCompleteness, ConfCoherence, ConfDepth, ConfCoverage, ConfFormality}

// RenderJSON returns the CommandResult as pretty-printed JSON.
func (cr *CommandResult) RenderJSON() (string, error) {
	data, err := json.MarshalIndent(cr, "", "  ")
	if err != nil {
		return "", err
	}
	return string(data), nil
}
